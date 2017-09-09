use rand::{thread_rng, Rng};
use std::sync::mpsc::{Sender, Receiver};
use time::precise_time_s;
use std::collections::HashMap;

pub type PlayerId = usize;
pub type NodeId = usize;
pub type LinkId = usize;
pub type FlowId = usize;

#[derive(Clone, Debug)]
#[derive(Serialize)]
#[serde(tag = "type")]
pub enum Response {
    GameState(Game),
    FlowState{flows: Vec<Flow>},
    FlowUpdate{flows: Vec<Flow>},
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
    GetState,
    Restart,
    Calc,
    ChangeFlow {flow_id: FlowId, dir: ReqDir}
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
    None,
    To1(PlayerId),
    To2(PlayerId),
}

#[derive(Serialize, Deserialize)]
#[derive(Copy, Clone, Debug)]
pub struct Blob {
    amount: f32,
    arrive_at: f64,
}

#[derive(Serialize, Deserialize)]
#[derive(Clone, Debug)]
enum FlowHost {
    Link{id: LinkId, dir: Dir, blobs: Vec<Blob>},
    Node(NodeId),
}

#[derive(Serialize, Deserialize)]
#[derive(Clone, Debug)]
pub struct Flow {
    id: FlowId,
    amount: f32,
    host: FlowHost,
}

#[derive(Serialize, Deserialize)]
#[derive(Clone, Debug)]
pub struct Game {
    pub nodes: Vec<Node>,
    pub links: Vec<Link>,
    pub flows: Vec<Flow>,
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
}

impl Game {
    pub fn new() -> Game {
        let nodes = gen_nodes(100);
        let links = gen_links(&nodes);
        let flows = gen_flows(&nodes, &links);
        Game {nodes, links, flows}
    }

    fn renew(&mut self) {
        self.nodes = gen_nodes(100);
        self.links = gen_links(&self.nodes);
        self.flows = gen_flows(&self.nodes, &self.links);
    }

    fn calc(&mut self, old_time: f64) -> f64 {
        let new_time = precise_time_s();
        let dtime = new_time - old_time;

        let mut nodes = HashMap::new();
        let mut links = HashMap::new();
        for n in &self.nodes {
            nodes.insert(n.id, n);
        }
        for l in &self.links {
            links.insert(l.id, l);
        }

        //сначала увеличиваем поток в нодах
        //тут мы полагаемся на то, что сначала обработаются линки, а потом ноды
        let mut inflows = HashMap::new();
        for f in &mut self.flows {
            match f.host {
                FlowHost::Link { id, dir, ref mut blobs } => {
                    let link = links[&id];
                    let to = match dir {
                        Dir::To1(_) => nodes[&link.n1],
                        Dir::To2(_) => nodes[&link.n2],
                        Dir::None => continue,
                    };

                    for &blob in blobs.iter() {
                        if blob.arrive_at <= new_time {
                            *inflows.entry(to.id).or_insert(0.) += blob.amount;
                        }
                    }
                    blobs.retain(|&b| b.arrive_at > new_time);
                }
                FlowHost::Node(id) => {
                    f.amount += nodes[&id].size * dtime as f32;
                    if inflows.contains_key(&id) {
                        f.amount += inflows[&id];
                    }
                }
            }
        }
        //затем откачиваем его
        //тут мы тоже полагаемся на то, что сначала обработаются линки, а потом ноды
        let mut outflows = HashMap::new();
        for f in &mut self.flows {
            match f.host {
                FlowHost::Link { id, dir, blobs: _} => {
                    let link = links[&id];
                    let from = match dir {
                        Dir::To1(_) => nodes[&link.n2],
                        Dir::To2(_) => nodes[&link.n1],
                        Dir::None => continue,
                    };
                    // TODO откачиваемый поток должен зависеть от текущего среднего потока игрока
                    // + чуть-чуть (чуть-чуть зависит от количества потоков)
                    // но пока все линки откачивают просто 3 потока в секунду
                    let amount = dtime as f32 * 3.;
                    if !outflows.contains_key(&from.id) {
                        outflows.insert(from.id, vec![amount]);
                    } else {
                        outflows.get_mut(&from.id).unwrap().push(amount);
                    }
                }
                FlowHost::Node(id) => {
                    if outflows.contains_key(&id) {
                        let outflow = outflows.get_mut(&id).unwrap();
                        let sum = outflow.iter().fold(0., |acc, &x| acc + x);
                        if sum < f.amount {
                            f.amount -= sum;
                        }
                        else {
                            let factor = f.amount / sum;
                            for a in outflow.iter_mut() {
                                *a *= factor;
                            }
                            f.amount = 0.;
                        }
                    }
                }
            }
        }
        for f in &mut self.flows {
            match f.host {
                FlowHost::Link { id, dir, ref mut blobs } => {
                    let link = links[&id];
                    let (from, to) = match dir {
                        Dir::To1(_) => (nodes[&link.n2], nodes[&link.n1]),
                        Dir::To2(_) => (nodes[&link.n1], nodes[&link.n2]),
                        Dir::None => continue,
                    };
                    let amount = outflows[&from.id][0];
                    outflows.get_mut(&from.id).unwrap().remove(0);
                    blobs.push(Blob {amount, arrive_at: new_time + from.time_to(to)});
                    f.amount = blobs.iter().fold(0., |acc, &b| acc + b.amount);
                }
                FlowHost::Node(_) => break,
            }
        }
        new_time
    }
    
    fn change_flow(&mut self, flow_id: FlowId, new_dir: Dir) -> Vec<Flow> {
        let flow = match self.flows.iter_mut().find(|f| f.id == flow_id) {
            Some(f) => f,
            None => return vec!(),
        };
        match flow.host {
            FlowHost::Node(_) => vec!(),
            FlowHost::Link {id, dir: _, blobs: _} => {
                //TODO тут надо смотреть, может ли игрок поменять направление
                flow.host = FlowHost::Link{id, dir: new_dir, blobs: vec!()};
                vec![flow.clone()]
            }
        }
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
                    t = self.calc(t);
                    AddressResponse {
                        whom: Address::Player(id),
                        response: Response::GameState(self.clone())
                    }
                }
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
                        AddressResponse {
                            whom: Address::None,
                            response: Response::Nop
                        }
                    } else {
                        t = self.calc(t);
                        AddressResponse {
                            whom: Address::All,
                            response: Response::FlowState { flows: self.flows.clone() }
                        }
                    }
                }
                Request::ChangeFlow {flow_id, dir} => {
                    let d = match dir {
                        ReqDir::To1 => Dir::To1(id),
                        ReqDir::To2 => Dir::To2(id),
                    };
                    let update = self.change_flow(flow_id, d);
                    AddressResponse {
                        whom: if update.len() > 0 {Address::All} else {Address::None},
                        response: Response::FlowUpdate{flows: update}
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

fn gen_flows(nodes: &[Node], links: &[Link]) -> Vec<Flow> {
    let mut res = vec!();
    //порядок генерации (сначала линки, потом ноды) важен!
    //обсчет тика полагается на то, что сначала идут линки!
    for &link in links {
        let id = res.len();
        res.push(Flow {
            id,
            amount: 0.,
            host: FlowHost::Link {
                id: link.id,
                dir: Dir::None,
                blobs: vec!(),
            }
        });
    }
    for &node in nodes {
        let id = res.len();
        res.push(Flow{id, amount: 0., host: FlowHost::Node(node.id)});
    }
    res
}