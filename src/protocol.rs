use super::network;
use super::node::Node;
use super::routing;
use super::utils::ChannelPayload;

use crossbeam_channel;
use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct Protocol {
    pub routes: Arc<Mutex<routing::RoutingTable>>,
    pub store: Arc<Mutex<HashMap<String, String>>>,
    pub rpc: Arc<network::Rpc>,
    pub node: Node,
}

impl Protocol {
    pub fn new(ip: String, port: u16, bootstrap: Option<Node>) -> Self {
        let node = Node::new(ip, port);
        println!("[VERBOSE] Protocol::new --> Node created");

        let (rt_channel_sender, rt_channel_receiver) = crossbeam_channel::unbounded();

        let routes = routing::RoutingTable::new(
            node.clone(),
            bootstrap,
            rt_channel_sender.clone(),
            rt_channel_receiver.clone(),
        );
        println!("[VERBOSE] Protocol::new --> Routes created");

        let (rpc_channel_sender, rpc_channel_receiver) = mpsc::channel();

        let rpc = network::Rpc::new(node.clone());
        network::Rpc::open(rpc.clone(), rpc_channel_sender);
        println!("[VERBOSE] Protocol::new --> RPC created");

        let protocol = Self {
            routes: Arc::new(Mutex::new(routes)),
            store: Arc::new(Mutex::new(HashMap::new())),
            rpc: Arc::new(rpc),
            node,
        };

        protocol.clone().requests_handler(rpc_channel_receiver);
        protocol
            .clone()
            .rt_forwarder(rt_channel_sender, rt_channel_receiver);

        // TODO: perform lookup on ourselves

        protocol
    }

    fn rt_forwarder(
        self,
        sender: crossbeam_channel::Sender<ChannelPayload>,
        receiver: crossbeam_channel::Receiver<ChannelPayload>,
    ) {
        std::thread::spawn(move || {
            for req in receiver.iter() {
                let protocol = self.clone();
                let sender_clone = sender.clone();

                println!(
                    "[VERBOSE] Protocol::rt_forwarder --> Spawning thread to forward {:?}",
                    &req
                );
                std::thread::spawn(move || match req {
                    ChannelPayload::Request(payload) => match payload.0 {
                        network::Request::Ping => {
                            let success = protocol.ping(payload.1);
                            if success {
                                if let Err(_) = sender_clone
                                    .send(ChannelPayload::Response(network::Response::Ping))
                                {
                                    println!("[FAILED] Protocol::rt_forwared --> Receiver is dead, closing channel");
                                }
                            } else {
                                if let Err(_) = sender_clone.send(ChannelPayload::NoData) {
                                    println!("[FAILED] Protocol::rt_forwared --> Receiver is dead, closing channel");
                                }
                            }
                        }
                        _ => {
                            unimplemented!();
                        }
                    },
                    ChannelPayload::Response(_) => {
                        println!("[FAILED] Protocol::rt_forwarder --> Received a Response instead of a Request")
                    }
                    ChannelPayload::NoData => {
                        println!("[FAILED] Protocol::rt_forwarder --> Received a NoData instead of a Request")
                    }
                });
            }
        });
    }

    fn requests_handler(self, receiver: mpsc::Receiver<network::ReqWrapper>) {
        println!(
            "[*] Protocol::requests_handler --> Starting Requests Handler for receiver: {} [*]",
            &self.node.get_addr()
        );
        std::thread::spawn(move || {
            for req in receiver.iter() {
                let protocol = self.clone();

                println!(
                    "[VERBOSE] Protocol::requests_handler --> Spawning thread to handle {:?}",
                    &req
                );
                std::thread::spawn(move || {
                    let res = protocol.craft_res(req);
                    protocol.reply(res);
                });
            }
        });
    }

    fn craft_res(&self, req: network::ReqWrapper) -> (network::Response, network::ReqWrapper) {
        println!("\t[VERBOSE] Protocol::craft_res --> Parsing: {:?}", &req);

        let mut routes = self
            .routes
            .lock()
            .expect("Failed to acquire mutex on 'Routes' struct");

        // must craft node object because ReqWrapper contains only the src string addr
        let split = req.src.split(":");
        let parsed: Vec<&str> = split.collect();

        let src_node = Node::new(
            parsed[0].to_string(),
            parsed[1]
                .parse::<u16>()
                .expect("[FAILED] Failed to parse Node port from address"),
        );
        routes.update(src_node);
        drop(routes);

        match req.payload {
            network::Request::Ping => (network::Response::Ping, req),
            network::Request::Store(ref k, ref v) => {
                // ref is used to borrow k and v, which are the contents of req

                let mut store = self
                    .store
                    .lock()
                    .expect("[FAILED] Protocol::craft_res --> Failed to acquire mutex on store");
                store.insert(k.to_string(), v.to_string());

                (network::Response::Ping, req)
            }
            network::Request::FindNode(ref id) => {
                let routes = self.routes.lock().expect(
                    "[FAILED] Protocol::craft_res --> Failed to acquire mutex on 'Routes' struct",
                );

                let result = routes.get_closest_nodes(id.clone(), super::ALPHA);

                (network::Response::FindNode(result), req)
            }
            network::Request::FindValue(ref k) => {
                let key = super::key::Key::new(k.to_string());
                let store = self.store.lock().expect(
                    "[FAILED] Protocol::craft_res --> Failed to acquire mutex on 'Store' struct",
                );

                let val = store.get(k);

                match val {
                    Some(v) => (
                        network::Response::FindValue(routing::FindValueResult::Value(
                            v.to_string(),
                        )),
                        req,
                    ),
                    None => {
                        let routes = self.routes.lock().expect("[FAILED] Protocol::craft_res --> Failed to acquire mutex on 'Routes' struct");
                        (
                            network::Response::FindValue(routing::FindValueResult::Nodes(
                                routes.get_closest_nodes(key, super::ALPHA),
                            )),
                            req,
                        )
                    }
                }
            }
        }
    }

    fn reply(&self, packet_details: (network::Response, network::ReqWrapper)) {
        println!(
            "\t[VERBOSE] Replying with {:?} to {}",
            &packet_details.0, &packet_details.1.src
        );

        let msg = network::RpcMessage {
            token: packet_details.1.token,
            src: self.node.get_addr(),
            dst: packet_details.1.src,
            msg: network::Message::Response(packet_details.0),
        };

        self.rpc.send_msg(&msg);
    }

    pub fn ping(&self, dst: Node) -> bool {
        println!("[STATUS] Protocol::ping --> Pinging...");
        let res = self
            .rpc
            .make_request(network::Request::Ping, dst.clone())
            .recv()
            .expect("Failed to receive data from channel while awaiting Ping response");

        let mut routes = self
            .routes
            .lock()
            .expect("Failed to acquire lock on routes");

        if let Some(network::Response::Ping) = res {
            println!("[STATUS] Protocol::Ping --> Got Pong");
            routes.update(dst);
            true
        } else {
            println!(
                "[FAILED] Protocol::Ping --> No response, removing contact from routing tablr"
            );
            routes.remove(&dst);
            false
        }
    }

    pub fn store(&self, dst: Node, key: String, val: String) -> bool {
        /*
            For both to store and to find a <key,value>-pair, a node lookup must performed. If a <key,value>-
            pair shall be stored in the network, a node lookup for the key is conducted. Thereafter, STORE-
            RPCs are sent to all of the k nodes the node lookup has returned. A STORE-RPC instructs a
            node to store the <key,value>-pair contained in the message locally.
        */
        let res = self
            .rpc
            .make_request(network::Request::Store(key, val), dst.clone())
            .recv()
            .expect("[FAILED] Protocol::store --> Failed to receive response through channel");

        // since we get a ping, update our routing table
        let mut routes = self
            .routes
            .lock()
            .expect("[FAILED] Protocol::store --> Failed to acquire mutex on 'Routes' struct");
        if let Some(network::Response::Ping) = res {
            routes.update(dst);
            true
        } else {
            routes.remove(&dst);
            false
        }
    }

    pub fn find_node(
        &self,
        dst: Node,
        id: super::key::Key,
    ) -> Option<Vec<routing::NodeAndDistance>> {
        let res = self
            .rpc
            .make_request(network::Request::FindNode(id), dst.clone())
            .recv()
            .expect("[FAILED] Protocol::find_node --> Failed to receive response through channel");

        let mut routes = self
            .routes
            .lock()
            .expect("[FAILED] Protocol::find_node --> Failed to acquire mutex on 'Routes' struct");
        if let Some(network::Response::FindNode(entries)) = res {
            routes.update(dst);
            Some(entries)
        } else {
            routes.remove(&dst);
            None
        }
    }

    pub fn find_value(&self, dst: Node, k: String) -> Option<routing::FindValueResult> {
        let res = self
            .rpc
            .make_request(network::Request::FindValue(k), dst.clone())
            .recv()
            .expect("[FAILED] Protocol::find_value --> Failed to receive response through channel");

        let mut routes = self
            .routes
            .lock()
            .expect("[FAILED] Protocol::find_value --> Failed to acquire mutex on 'Routes' struct");

        if let Some(network::Response::FindValue(val)) = res {
            routes.update(dst);
            Some(val)
        } else {
            routes.remove(&dst);
            None
        }
    }
}
