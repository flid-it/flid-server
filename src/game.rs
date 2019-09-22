use rand::{thread_rng, Rng};
use crossbeam_channel::{Sender, Receiver};
use time::precise_time_s;
use std::collections::HashMap;
use serde_derive::{Serialize, Deserialize};
use log::{debug};

pub type PlayerId = usize;
pub type NodeId = usize;
pub type LinkId = usize;

#[derive(Clone, Debug)]
#[derive(Serialize)]
#[serde(tag = "type")]
pub enum Response {
    GameState(Game),
    FlidState{flids: Vec<Flid>},
    FlidUpdate{flid: Flid},
    Hello{id: PlayerId},
    Nop,
}

#[derive(Clone, Debug)]
pub enum Address {
    None,
    Player(PlayerId),
    //SomePlayers(Vec<PlayerId>),
    All,
}

#[derive(Clone, Debug)]
pub struct AddressResponse {
    pub whom: Address,
    pub response: Response,
}

#[derive(Serialize, Deserialize)]
#[derive(Copy, Clone, Debug)]
pub enum ReqDir {
    To1,
    To2,
}

#[derive(Serialize, Deserialize)]
#[derive(Copy, Clone, Debug)]
#[serde(tag = "type")]
pub enum Request {
    NewPlayer,
    PlayerExit,
    GetState,
    Restart,
    Calc,
    Jump {link_id: LinkId},
}

#[derive(Copy, Clone, Debug)]
pub struct PersonalRequest {
    pub player: PlayerId,
    pub request: Request,
}

#[derive(Serialize, Deserialize)]
#[derive(Copy, Clone, Debug)]
struct Point {
    x: i64,
    y: i64,
}

#[derive(Serialize, Deserialize)]
#[derive(Copy, Clone, Debug)]
pub struct Node {
    id: NodeId,
    pos: Point,
    size: f32,
}

#[derive(Serialize, Deserialize)]
#[derive(Copy, Clone, Debug)]
pub struct Link {
    id: LinkId,
    quality: f32,
    n1: NodeId,
    n2: NodeId,
}

#[derive(Serialize, Deserialize)]
#[derive(Copy, Clone, Debug)]
pub enum Dir {
    To1,
    To2,
}

#[derive(Serialize, Deserialize)]
#[derive(Copy, Clone, Debug)]
pub struct Jump {
    id: LinkId,
    dir: Dir,
    start_at: f64,
    arrive_at: f64,
}

#[derive(Serialize, Deserialize)]
#[derive(Copy, Clone, Debug)]
enum Host {
    Link(Jump),
    Node(NodeId),
}

#[derive(Serialize, Deserialize)]
#[derive(Copy, Clone, Debug)]
pub struct Flid {
    id: PlayerId,
    host: Host,
}

#[derive(Serialize, Deserialize)]
#[derive(Clone, Debug)]
pub struct Game {
    pub nodes: Vec<Node>,
    pub links: Vec<Link>,
    pub flids: Vec<Flid>,
}

fn noop() -> AddressResponse {
    AddressResponse {
        whom: Address::None,
        response: Response::Nop
    }
}

impl Point {
    fn dist(self, other: Point) -> f32 {
        (((self.x - other.x).pow(2) + (self.y - other.y).pow(2)) as f32).sqrt()
    }
}

impl Node {
    fn dist_to(&self, other: &Node) -> f32 {
        self.pos.dist(other.pos)
    }

    fn time_to(&self, other: &Node) -> f64 {
        (self.dist_to(other) / 100.) as f64
    }
}

impl Link {
    fn has_id(&self, id: &NodeId) -> bool {
        self.n1 == *id || self.n2 == *id
    }

    fn between_ids(&self, n1: &NodeId, n2: &NodeId) -> bool {
        self.has_id(n1) && self.has_id(n2)
    }

    fn dir_from(&self, id: &NodeId) -> Option<Dir> {
        if self.n1 == *id {
            Some(Dir::To2)
        } else if self.n2 == *id {
            Some(Dir::To1)
        } else {
            None
        }
    }
}

impl Game {
    pub fn new() -> Game {
        let nodes = gen_nodes(100);
        let links = gen_links(&nodes);
        Game {nodes, links, flids: vec!()}
    }

    fn renew(&mut self) {
        self.nodes = gen_nodes(100);
        self.links = gen_links(&self.nodes);
        //todo respawn players
        self.flids = vec!();
    }

    fn calc(&mut self, _old_time: f64) -> f64 {
        let new_time = precise_time_s();
        //let dtime = new_time - old_time;

        let mut nodes = HashMap::new();
        let mut links = HashMap::new();
        for n in &self.nodes {
            nodes.insert(n.id, n);
        }
        for l in &self.links {
            links.insert(l.id, l);
        }

        for f in &mut self.flids {
            match f.host {
                Host::Link(jump) => {
                    let link = links[&jump.id];
                    let to = match jump.dir {
                        Dir::To1 => nodes[&link.n1],
                        Dir::To2 => nodes[&link.n2],
                    };

                    if jump.arrive_at <= new_time {
                        f.host = Host::Node(to.id);
                    }
                },
                Host::Node(_) => continue,
            }
        }
        new_time
    }

    fn jump(&self, host: &Host, link: &Link, time: f64) -> Option<Jump> {
        match host {
            Host::Link(_) => None,
            Host::Node(node_id) => {
                if let Some(dir) = link.dir_from(&node_id) {
                    Some(Jump {
                        id: link.id,
                        dir,
                        start_at: time,
                        arrive_at: time + self.time(link),
                    })
                } else {
                    None
                }
            },
        }
    }

    fn time(&self, link: &Link) -> f64 {
        let n1 = self.nodes.iter().find(|f| f.id == link.n1).unwrap();
        let n2 = self.nodes.iter().find(|f| f.id == link.n2).unwrap();
        n1.time_to(n2)
    }

    pub fn main_loop(mut self,
                     incoming: Receiver<PersonalRequest>,
                     outgoing: Sender<AddressResponse>) {
        let mut t = precise_time_s();
        loop {
            let p_req = incoming.recv().unwrap();
            let id = p_req.player;
            debug!("Game request: {:?}", p_req);

            let resp = match p_req.request {
                Request::NewPlayer => {
                    let node = self.nodes[thread_rng().gen_range(0, self.nodes.len())];
                    let flid = Flid {
                        id,
                        host: Host::Node(node.id),
                    };
                    self.flids.push(flid);

                    outgoing.send(AddressResponse {
                        whom: Address::Player(id),
                        response: Response::Hello{id},
                    }).unwrap();

                    AddressResponse {
                        whom: Address::All,
                        response: Response::GameState(self.clone())
                    }
                }
                Request::PlayerExit => {
                    self.flids.retain(|f| f.id != id);

                    AddressResponse {
                        whom: Address::All,
                        response: Response::GameState(self.clone())
                    }
                },
                Request::GetState => {
                    t = self.calc(t);
                    AddressResponse {
                        whom: Address::Player(id),
                        response: Response::GameState(self.clone())
                    }
                }
                Request::Restart => {
                    self.renew();
                    t = precise_time_s();
                    AddressResponse {
                        whom: Address::All,
                        response: Response::GameState(self.clone())
                    }
                }
                Request::Calc => {
                    if precise_time_s() - t < 0.2 {
                        noop()
                    } else {
                        t = self.calc(t);
                        AddressResponse {
                            whom: Address::All,
                            response: Response::FlidState { flids: self.flids.clone() }
                        }
                    }
                }
                Request::Jump {link_id} => {
                    let flid = self.flids.iter().find(|f| f.id == id).unwrap();
                    let link = self.links.iter().find(|l| l.id == link_id);
                    match link {
                        None => noop(),
                        Some(l) => match self.jump(&flid.host, l, t) {
                            None => noop(),
                            Some(jump) => {
                                let flid = self.flids.iter_mut().find(|f| f.id == id).unwrap();
                                flid.host = Host::Link(jump);
                                AddressResponse {
                                    whom: Address::All,
                                    response: Response::FlidUpdate{flid: flid.clone()},
                                }
                            },
                        },
                    }
                }
            };
            debug!("Game response: {:?}", resp);
            outgoing.send(resp).unwrap();
        }
    }
}

fn get_nearest_nodes(pos: &Point, nodes: &[Node], n: usize, dist: f32) -> Vec<Node> {
    let mut n = n;

    if n == 0 {
        n = nodes.len()
    }

    let mut source = nodes.to_vec();
    source.sort_by(|a, b| pos.dist(a.pos).partial_cmp(&pos.dist(b.pos)).unwrap());

    let mut res = vec!();
    for node in &source {
        if dist == 0. || pos.dist(node.pos) < dist {
            res.push(node.clone())
        }
        if res.len() >= n {
            break
        }
    }
    res
}

fn gen_nodes(n: usize) -> Vec<Node> {
    let mut res = vec!();
    let mut rng = thread_rng();

    while res.len() < n {
        let x = rng.gen_range(-1000, 1000);
        let y = rng.gen_range(-1000, 1000);
        let pos = Point{x, y};
        if get_nearest_nodes(&pos, &res, 1, 100f32).len() > 0 {
            continue;
        }

        let node = Node{id: res.len(), pos, size: rng.gen_range(0.5, 1.5)};
        res.push(node)
    }
    res
}

fn gen_links(nodes: &[Node]) -> Vec<Link> {
    let mut rng = thread_rng();
    let mut res: Vec<Link> = vec!();
    for &node in nodes {
        let links_count = rng.gen_range(2, 5) + 1;
        let nearest = get_nearest_nodes(&node.pos, nodes, links_count, 0.)[1..].to_vec();
        for n in &nearest {
            if let None = res.iter().find(|l| l.between_ids(&node.id, &n.id)) {
                let id = res.len();
                res.push(Link{id, quality: rng.gen_range(0.01, 0.99), n1: node.id, n2: n.id});
            }
        }
    }
    res
}
