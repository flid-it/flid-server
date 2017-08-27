use rand::{thread_rng, Rng};
use std::sync::mpsc::{Sender, Receiver};

pub type PlayerId = usize;
pub type NodeId = usize;
pub type LinkId = usize;

#[derive(Clone, Debug)]
#[derive(Serialize)]
#[serde(tag = "type")]
pub enum Response {
    GameState(Game),
}

#[derive(Clone, Debug)]
pub enum Address {
    Player(PlayerId),
    SomePlayers(Vec<PlayerId>),
    All,
}

#[derive(Clone, Debug)]
pub struct AddressResponse {
    pub whom: Address,
    pub response: Response,
}

#[derive(Copy, Clone, Debug)]
#[derive(Deserialize)]
#[serde(tag = "type")]
pub enum Request {
    NewPlayer,
    GetState,
    Restart,
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

impl Link {
    fn has_id(&self, id: &NodeId) -> bool {
        self.n1 == *id || self.n2 == *id
    }

    fn between_ids(&self, n1: &NodeId, n2: &NodeId) -> bool {
        self.has_id(n1) && self.has_id(n2)
    }
}

#[derive(Serialize, Deserialize)]
#[derive(Clone, Debug)]
pub struct Game {
    pub nodes: Vec<Node>,
    pub links: Vec<Link>,
}

impl Point {
    fn dist(self, other: Point) -> f32 {
        (((self.x - other.x).pow(2) + (self.y - other.y).pow(2)) as f32).sqrt()
    }
}

impl Node {
}

impl Game {
    pub fn new() -> Game {
        let nodes = gen_nodes(100);
        let links = gen_links(&nodes);
        Game {nodes, links}
    }

    pub fn renew(&mut self) {
        self.nodes = gen_nodes(100);
        self.links = gen_links(&self.nodes);
    }

    pub fn main_loop(mut self,
                     incoming: Receiver<PersonalRequest>,
                     outgoing: Sender<AddressResponse>) {
        loop {
            let p_req = incoming.recv().unwrap();
            let id = p_req.player;
            debug!("Game request: {:?}", p_req);

            let resp = match p_req.request {
                Request::NewPlayer => {
                    AddressResponse {
                        whom: Address::Player(id),
                        response: Response::GameState(self.clone())
                    }
                }
                Request::GetState => {
                    AddressResponse {
                        whom: Address::Player(id),
                        response: Response::GameState(self.clone())
                    }
                }
                Request::Restart => {
                    self.renew();
                    AddressResponse{
                        whom: Address::All,
                        response: Response::GameState(self.clone())
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
    let mut nodes: Vec<Node> = vec!();
    let mut rng = thread_rng();

    while nodes.len() < n {
        let x = rng.gen_range(-1000, 1000);
        let y = rng.gen_range(-1000, 1000);
        let pos = Point{x, y};
        if get_nearest_nodes(&pos, &nodes, 1, 100f32).len() > 0 {
            continue;
        }

        let node = Node{id: nodes.len(), pos, size: rng.gen_range(0.5, 1.5)};
        nodes.push(node)
    }
    nodes
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