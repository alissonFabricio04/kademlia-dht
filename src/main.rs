extern crate kademlia_dht;
use kademlia_dht::node::Node;
use kademlia_dht::protocol::Protocol;
use kademlia_dht::utils;

fn main() {
	let root = Node::new(utils::get_local_ip().unwrap(), 1337);
	let some_node = Node::new(utils::get_local_ip().unwrap(), 1338);
	let another_node = Node::new(utils::get_local_ip().unwrap(), 1339);

	let interface0 = Protocol::new(root.ip.clone(), root.port.clone(), None);
	let interface1 = Protocol::new(
		some_node.ip.clone(),
		some_node.port.clone(),
		Some(root.clone()),
	);
	let interface2 = Protocol::new(
		another_node.ip.clone(),
		another_node.port.clone(),
		Some(root.clone()),
	);

	interface0.ping(interface1.node.clone());
	interface2.ping(interface1.node.clone());

	interface0.store(
		interface1.node.clone(),
		"some_key".to_string(),
		"some_value".to_string(),
	);

	interface0.store(
		interface1.node.clone(),
		"another_key".to_string(),
		"another_value".to_string(),
	);

	let found = interface0.find_node(some_node.clone(), another_node.id.clone());
	println!("find_node: {:?}", found);

	utils::dump_interface_state(&interface0, "dumps/interface0.json");
	utils::dump_interface_state(&interface1, "dumps/interface1.json");
	utils::dump_interface_state(&interface2, "dumps/interface2.json");
}
