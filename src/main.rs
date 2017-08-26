extern crate ws;
extern crate rand;
extern crate serde;
#[macro_use]
extern crate serde_json;
#[macro_use]
extern crate serde_derive;

mod game;

use std::env;
use std::thread;
use std::collections::HashMap;
use std::sync::mpsc::{channel, Sender, Receiver};
use ws::{WebSocket, Handler, Factory, CloseCode};
use serde_json::{Value, to_string, from_str, to_value};

use game::{Game, Response, Request, PlayerId};

struct Server {
    to_game: Sender<Request>
}

struct PlayerHandler {
    ws: ws::Sender,
    to_game: Sender<Request>,
    from_game: Receiver<Response>,
    to_me: Sender<Response>,
}


impl Factory for Server {
    type Handler = PlayerHandler;

    fn connection_made(&mut self, ws: ws::Sender) -> PlayerHandler {
        let (to_me, from_game) = channel();
        PlayerHandler {ws: ws, to_game: self.to_game.clone(), from_game, to_me}
    }
}

impl Handler for PlayerHandler {
    fn on_open(&mut self, _: ws::Handshake) -> ws::Result<()> {
        println!("Connection opened!");
        self.to_game.send(Request::NewPlayer(self.ws.clone())).unwrap();
        Ok(())
    }

    fn on_message(&mut self, msg: ws::Message) -> ws::Result<()> {
        println!("Message received: {}", msg);

        let r: Request = from_str(msg.as_text().unwrap()).unwrap();
        self.on_game_request(r);
        return Ok(())
    }

    fn on_close(&mut self, code: CloseCode, reason: &str) {
        match code {
            CloseCode::Normal => println!("The client is done with the connection."),
            CloseCode::Away   => println!("The client is leaving the site."),
            _ => println!("The client encountered an error: {}", reason),
        }
    }
}

impl PlayerHandler {
    fn on_game_request(&self, req: Request) {
        //println!("Request: {:?}", req);
        self.to_game.send(req).unwrap();
    }

    fn on_game_response(&self, resp: Response) {
        //println!("Response: {:?}", resp);
        self.ws.send(ws::Message::from(to_string(&resp).unwrap())).unwrap();
    }
}

fn main() {
    let args: Vec<_> = env::args().collect();
    let addr;
    if args.len() < 3 {
        let port = match env::var("PORT") {
            Ok(val) => val,
            Err(_) => "3003".to_string(),
        };
        println!("Needed IP and port, so use default");
        addr = format!("0.0.0.0:{}", port);
    } else {
        addr = format!("{}:{}", args[1], args[2]);
    }

    let (to_game, from_players) = channel();

    let mut g = Game::new();

    thread::spawn(move|| {
        let mut players: HashMap<PlayerId, ws::Sender> = HashMap::new();
        let mut last_id = 1;
        loop {
            let req = from_players.recv().unwrap();
            //println!("Game proc request: {:?}", req);

            let (id, resp) = match req {
                Request::NewPlayer(ref sender) => {
                    let id = last_id;
                    last_id += 1;
                    players.insert(id, sender.clone());

                    (id, Response::SetPlayer {id})
                }
                Request::GetState{id} => {
                    (id, Response::GameState {nodes: g.nodes.clone()})
                }
                Request::Restart => {
                    g.renew();
                    (0, Response::GameState {nodes: g.nodes.clone()})
                }
            };
            //println!("Send response to {}: {:?}", id, resp);

            let mut to_drop = vec!();
            {
                let mut addrs = vec!();
                if id > 0 {
                    addrs.push((&id, &players[&id]));
                } else {
                    println!("Broadcast to {} clients", players.len());
                    addrs.extend(players.iter());
                }
                for &(&id, ref player) in &addrs {
                    if !send_to(&player, &resp) {
                        println!("Cannot send to {}, off it", id);
                        to_drop.push(id);
                    }
                }
            }

            for id in to_drop {
                players.remove(&id);
            }
        }
    });

    let server = Server { to_game };
    println!("Server started at {}", addr);
    WebSocket::new(server).unwrap().listen(addr).unwrap();
}

fn send_to(ws: &ws::Sender, resp: &Response) -> bool {
    match ws.send(ws::Message::from(to_string(resp).unwrap())) {
        Err(_) => false,
        Ok(_) => true,
    }
}