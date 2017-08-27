#![feature(mpsc_select)]

extern crate ws;
extern crate rand;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;

mod game;

use std::env;
use std::thread;
use std::collections::HashMap;
use std::sync::mpsc::{channel, Sender, Receiver};
use ws::{listen, Handler, CloseCode};
use serde_json::{to_string, from_str};

use game::{Game, Response, Address, AddressResponse, Request, PersonalRequest, PlayerId};

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
        println!("Connection opened!");
        self.to_dispatcher.send(ServerEvent::NewPlayer {id: self.id, ws: self.ws.clone()}).unwrap();
        self.to_game.send(self.personal(Request::NewPlayer)).unwrap();
        Ok(())
    }

    fn on_message(&mut self, msg: ws::Message) -> ws::Result<()> {
        println!("Message received: {}", msg);

        let r = from_str(msg.as_text().unwrap()).unwrap();
        self.to_game.send(self.personal(r)).unwrap();

        return Ok(())
    }

    fn on_close(&mut self, code: CloseCode, reason: &str) {
        self.to_dispatcher.send(ServerEvent::PlayerExit {id: self.id}).unwrap();
        match code {
            CloseCode::Normal => println!("The client is done with the connection."),
            CloseCode::Away   => println!("The client is leaving the site."),
            _ => println!("The client encountered an error: {}", reason),
        }
    }
}

impl PlayerHandler {
    fn personal(&self, r: Request) -> PersonalRequest {
        PersonalRequest{player: self.id, request: r}
    }
}

fn send_to(ws: &ws::Sender, response: &Response) -> bool {
    match ws.send(ws::Message::from(to_string(response).unwrap())) {
        Err(_) => false,
        Ok(_) => true,
    }
}

fn send(list: &HashMap<PlayerId, ws::Sender>, addr: &Address, response: &Response) {
    match *addr {
        Address::Player(ref id) => {
            send_to(&list[id], response);
        }
        Address::SomePlayers(ref ids) => {
            for id in ids {
                send_to(&list[&id], response);
            }
        }
        Address::All => {
            for ref ws in list.values() {
                send_to(&ws, response);
            }
        }
    }
}

fn dispatch(from_server: Receiver<ServerEvent>, from_game: Receiver<AddressResponse>) {
    let mut to_players: HashMap<PlayerId, ws::Sender> = HashMap::new();
    loop {
        select! {
            server_event = from_server.recv() => {
                let event = server_event.unwrap();
                match event {
                    ServerEvent::NewPlayer{id, ws} => {
                        to_players.insert(id, ws);
                    }
                    ServerEvent::PlayerExit{id} => {
                        to_players.remove(&id);
                    }
                }
            },
            game_response = from_game.recv() => {
                let response = game_response.unwrap();
                send(&to_players, &response.whom, &response.response);
            }
        }
    }
}


fn main() {
    let args: Vec<_> = env::args().collect();
    let addr;
    let default_port = 3003;
    let port = if args.len() < 2 {
        print!("Needed port, so use ");
        match env::var("PORT") {
            Ok(val) => {
                println!("$PORT ({})", val);
                val
            },
            Err(_) => {
                println!("default ({})", default_port);
                default_port.to_string()
            },
        }
    } else {
        args[1].to_string()
    };
    addr = format!("0.0.0.0:{}", port);

    let (to_game, from_players) = channel();
    let (to_dispatcher, from_server) = channel();
    let (to_dispatcher_game, from_game) = channel();

    let g = Game::new();

    thread::spawn(move|| {
        dispatch(from_server, from_game);
    });

    thread::spawn(move|| {
        g.main_loop(from_players, to_dispatcher_game);
    });

    let mut last_id = 0;

    println!("Server started at {}", addr);
    listen(addr, |ws| {
        last_id += 1;
        PlayerHandler {
            id: last_id,
            ws,
            to_game: to_game.clone(),
            to_dispatcher: to_dispatcher.clone()
        }
    }).unwrap();
}
