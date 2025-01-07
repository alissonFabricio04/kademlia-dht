extern crate kademlia_dht;
use kademlia_dht::node::Node;
use kademlia_dht::protocol::Protocol;
use kademlia_dht::utils;

const BIG_TEST: bool = true;

// be careful with the net size, for example my computer can't spawn too many threads
// messages may also exceed the buffer size used for streaming (see issue #1)
const NET_SIZE: usize = 10;

fn main() {
    // searching for nodes close to a key
    let node0 = Node::new(utils::get_local_ip().unwrap(), 1337);
    let node1 = Node::new(utils::get_local_ip().unwrap(), 1338);
    let node2 = Node::new(utils::get_local_ip().unwrap(), 1339);

    let interface0 = Protocol::new(node0.ip.clone(), node0.port.clone(), None);
    let _ = Protocol::new(node1.ip.clone(), node1.port.clone(), Some(node0.clone()));
    let interface2 = Protocol::new(node2.ip.clone(), node2.port.clone(), Some(node0.clone()));

    let key = "key-1";
    let value = "value-1";
    interface0.put(key.to_string(), value.to_string());

    let get_res = interface2.get("key-1".to_string());
    println!("Extracted: {:?}", get_res);

    let random_key = "key-";
    let dis = interface2.search_for_rapprochement(random_key.to_string());

    println!("{:?}", dis);
}
