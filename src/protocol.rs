use super::key;
use super::network;
use super::node::Node;
use super::routing::RoutingTable;

use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct Protocol {
    pub routes: Arc<Mutex<RoutingTable>>,
    pub store: Arc<Mutex<HashMap<String, String>>>,
    pub rpc: Arc<network::Rpc>,
    pub node: Node,
}

impl Protocol {
    // TODO: missing bootstrap node
    pub fn new(ip: String, port: u16) -> Self {
        let node = Node::new(ip, port);
        println!("[VERBOSE] Protocol::new --> Node created");

        let routes = RoutingTable::new(node.clone());
        println!("[VERBOSE] Protocol::new --> Routes created");

        let (sender, receiver) = mpsc::channel();
        let rpc = network::Rpc::new(node.clone());
        network::Rpc::open(rpc.clone(), sender);
        println!("[VERBOSE] Protocol::new --> RPC created");

        let node = Self {
            routes: Arc::new(Mutex::new(routes)),
            store: Arc::new(Mutex::new(HashMap::new())),
            rpc: Arc::new(rpc),
            node,
        };

        node.clone().requests_handler(receiver);

        node
    }

    fn requests_handler(self, receiver: mpsc::Receiver<network::Request>) {
        // TODO: return response

        println!(
            "[*] Protocol::requests_handler --> Starting Requests Handler for receiver: {} [*]",
            self.node.get_addr()
        );
        std::thread::spawn(move || {
            for req in receiver.iter() {
                let node = self.clone();

                println!(
                    "[VERBOSE] Protocol::requests_handler --> Spawning thread to handle {:?}",
                    &req
                );
                std::thread::spawn(move || {
                    node.parse_req(req);
                });
            }
        });
    }

    fn parse_req(&self, req: network::Request) {
        println!(
            "\t[VERBOSE] Protocol::requests_handler --> Parsing: {:?}",
            req
        );
    }

    pub fn ping(&self, dst: Node) {
        let msg = network::RpcMessage {
            token: key::Key::new(String::from("Hello node1")),
            src: self.node.get_addr(),
            dst: dst.get_addr(),
            msg: network::Message::Request(network::Request::Ping),
        };
        self.rpc.send_msg(&msg, &dst.get_addr());
        println!(
            "[+] Protocol::ping --> Ping from {} to {} with token {:?} was sent!",
            msg.src, msg.dst, msg.token
        );
    }
}
