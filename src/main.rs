extern crate ws;

use std::env;
use ws::{listen, Handler, Sender, Result, Message, CloseCode};

struct Server {
    out: Sender,
}

impl Handler for Server {
    fn on_open(&mut self, _: ws::Handshake) -> Result<()> {
        println!("Connection opened!");
        Ok(())
    }

    fn on_message(&mut self, msg: Message) -> Result<()> {
        println!("Message received: {}", msg);
        self.out.send(msg)
    }

    fn on_close(&mut self, code: CloseCode, reason: &str) {
        match code {
            CloseCode::Normal => println!("The client is done with the connection."),
            CloseCode::Away   => println!("The client is leaving the site."),
            _ => println!("The client encountered an error: {}", reason),
        }
    }
}

fn main() {
    let args: Vec<_> = env::args().collect();
    if args.len() < 3 {
        panic!("Needed IP and port!");
    }
    let addr = format!("{}:{}", args[1], args[2]);
    println!("Server started at {}", addr);
    listen(addr, |out| Server { out: out }).unwrap()
} 