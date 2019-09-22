mod game;

use std::env;
use std::thread;
use std::collections::HashMap;
use crossbeam_channel::{select, unbounded, Sender, Receiver};
use ws::{listen, Handler, CloseCode};
use serde_json::{to_string, from_str};
use log::{debug};

use crate::game::{Game, Response, Address, AddressResponse, Request, PersonalRequest, PlayerId};

enum ServerEvent {
    NewPlayer {id: PlayerId, ws: ws::Sender},
    PlayerExit {id: PlayerId},
}

struct PlayerHandler {
    id: PlayerId,
    ws: ws::Sender,
    to_game: Sender<PersonalRequest>,
    to_dispatcher: Sender<ServerEvent>,
}

impl Handler for PlayerHandler {
    fn on_open(&mut self, _: ws::Handshake) -> ws::Result<()> {
        debug!("Player {} open connection", self.id);
        self.to_dispatcher.send(ServerEvent::NewPlayer {id: self.id, ws: self.ws.clone()}).unwrap();
        Ok(())
    }

    fn on_message(&mut self, msg: ws::Message) -> ws::Result<()> {
        debug!("Player {} send message: {}", self.id, msg);

        if msg.is_text() {
            match from_str(msg.as_text().unwrap()) {
                Ok(r) => {
                    self.to_game.send(personal(self.id, r)).unwrap();
                }
                Err(_) => {
                    debug!("Player {} sent wrong message!", self.id)
                }
            }
        } else {
            debug!("Player {} sent non-text message!", self.id)
        }

        return Ok(())
    }

    fn on_close(&mut self, code: CloseCode, reason: &str) {
        self.to_dispatcher.send(ServerEvent::PlayerExit {id: self.id}).unwrap();
        match code {
            CloseCode::Normal => debug!("Player {} is done", self.id),
            CloseCode::Away   => debug!("Player {} is leaving the site", self.id),
            _ => debug!("Player {} encountered an error: {}", self.id, reason),
        }
    }
}

fn send_to(ws: Option<&ws::Sender>, response: &Response) -> bool {
    match ws {
        None => false,
        Some(ws) => match ws.send(ws::Message::from(to_string(response).unwrap())) {
            Err(_) => false,
            Ok(_) => true,
        }
    }

}

fn send(list: &HashMap<PlayerId, ws::Sender>, addr: &Address, response: &Response) {
    debug!("Send response to {:?}: {:?}", addr, response);
    match *addr {
        Address::None => debug!("Some answer to no one"),
        Address::Player(ref id) => {
            send_to(list.get(id), response);
        }
        /*Address::SomePlayers(ref ids) => {
            for id in ids {
                send_to(list.get(id), response);
            }
        }*/
        Address::All => {
            for ref ws in list.values() {
                send_to(Some(ws), response);
            }
        }
    };
}

fn personal(id: PlayerId, r: Request) -> PersonalRequest {
    PersonalRequest{player: id, request: r}
}

fn dispatch(
    from_server: Receiver<ServerEvent>,
    from_game: Receiver<AddressResponse>,
    to_game: Sender<PersonalRequest>)
{
    let mut to_players: HashMap<PlayerId, ws::Sender> = HashMap::new();
    loop {
        select! {
            recv(from_server) -> server_event => {
                let event = server_event.unwrap();
                match event {
                    ServerEvent::NewPlayer{id, ws} => {
                        debug!("Added player {} to dispatcher", id);
                        to_players.insert(id, ws);
                        to_game.send(personal(id, Request::NewPlayer)).unwrap();
                    }
                    ServerEvent::PlayerExit{id} => {
                        debug!("Remove player {} from dispatcher", id);
                        to_players.remove(&id);
                    }
                }
            },
            recv(from_game) -> game_response => {
                let response = game_response.unwrap();
                send(&to_players, &response.whom, &response.response);
            }
        }
    }
}


fn main() {
    env_logger::init();

    let args: Vec<_> = env::args().collect();
    let addr;
    let default_port = 3003;
    let port = if args.len() < 2 {
        match env::var("PORT") {
            Ok(val) => {
                debug!("Needed port, so use $PORT ({})", val);
                val
            },
            Err(_) => {
                debug!("Needed port, so use default ({})", default_port);
                default_port.to_string()
            },
        }
    } else {
        args[1].to_string()
    };
    addr = format!("0.0.0.0:{}", port);

    let (to_game, from_players) = unbounded();
    let (to_dispatcher, from_server) = unbounded();
    let (to_dispatcher_game, from_game) = unbounded();

    let to_game2 = to_game.clone();

    let g = Game::new();

    thread::spawn(|| {
        dispatch(from_server, from_game, to_game);
    });

    thread::spawn(|| {
        g.main_loop(from_players, to_dispatcher_game);
    });

    let mut last_id = 0;

    debug!("Server started at {}", addr);
    listen(addr, |ws| {
        last_id += 1;
        PlayerHandler {
            id: last_id,
            ws,
            to_game: to_game2.clone(),
            to_dispatcher: to_dispatcher.clone()
        }
    }).unwrap();
}
