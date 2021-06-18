use super::key::{Distance, Key};
use super::network;
use super::node::Node;
use super::utils::ChannelPayload;
use super::K_PARAM;
use super::N_BUCKETS;

use crossbeam_channel;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct NodeAndDistance(pub Node, pub Distance);

#[derive(Debug, Serialize, Deserialize)]
pub struct FindValueResult(Option<Vec<NodeAndDistance>>, Option<String>);

#[derive(Debug)]
pub struct KBucket {
    pub nodes: Vec<Node>,
    pub size: usize,
}

#[derive(Debug)]
pub struct RoutingTable {
    pub node: Node,
    pub kbuckets: Vec<KBucket>,
    pub sender: crossbeam_channel::Sender<ChannelPayload>,
    pub receiver: crossbeam_channel::Receiver<ChannelPayload>,
}

impl PartialEq for NodeAndDistance {
    fn eq(&self, other: &NodeAndDistance) -> bool {
        let mut equal = true;
        let mut i = 0;
        while equal && i < 32 {
            if self.1 .0[i] != other.1 .0[i] {
                equal = false;
            }

            i += 1;
        }

        equal
    }
}

// A k-bucket with index i stores contacts whose ids
// have a distance between 2^i and 2^i+1 to the own id
impl KBucket {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            size: K_PARAM,
        }
    }
}

impl RoutingTable {
    pub fn new(
        node: Node,
        bootstrap: Option<Node>,
        sender: crossbeam_channel::Sender<ChannelPayload>,
        receiver: crossbeam_channel::Receiver<ChannelPayload>,
    ) -> Self {
        let mut kbuckets: Vec<KBucket> = Vec::new();
        for _ in 0..N_BUCKETS {
            kbuckets.push(KBucket::new());
        }

        let mut ret = Self {
            node: node.clone(),
            kbuckets,
            sender,
            receiver,
        };

        ret.update(node);

        if let Some(bootstrap) = bootstrap {
            ret.update(bootstrap);
        }

        ret
    }

    fn get_lookup_bucket_index(&self, key: &Key) -> usize {
        // https://stackoverflow.com/questions/2656642/easiest-way-to-find-the-correct-kademlia-bucket

        // given a bucket j, we are guaranteed that
        //  2^j <= distance(node, contact) < 2^(j+1)
        // a node with distance d will be put in the k-bucket with index i=⌊logd⌋

        let d = Distance::new(&self.node.id, key);
        for i in 0..super::KEY_LEN {
            for j in (0..8).rev() {
                if (d.0[i] >> (7 - j)) & 0x1 != 0 {
                    return i * 8 + j;
                }
            }
        }

        super::KEY_LEN * 8 - 1
    }

    fn contact_via_rpc(&self, dst: String) -> bool {
        if let Err(_) = self
            .sender
            .send(ChannelPayload::Request((network::Request::Ping, dst)))
        {
            println!("RoutingTable::contact_via_rpc --> Receiver is dead, closing channel");
            return false;
        }

        true
    }

    pub fn update(&mut self, node: Node) {
        /*
            TODO: Adding a node:
                If the corresponding k-bucket stores less than k contacts
                and the new node is not already contained, the new node is added at the tail of the list.
                If the k-bucket contains the contact already, it is moved to the tail of the list.
                Should the appropriate k-bucket be full, then the contact at the head of the list is pinged.
                If it replies, then it is moved to the tail of the list and the new contact is not added.
                If it does not, the old contact is discarded and the new contact is added at the tail.
        */

        let bucket_idx = self.get_lookup_bucket_index(&node.id);

        if self.kbuckets[bucket_idx].nodes.len() < K_PARAM {
            let node_idx = self.kbuckets[bucket_idx]
                .nodes
                .iter()
                .position(|x| x.id == node.id);
            match node_idx {
                Some(i) => {
                    println!("[VERBOSE] Routing::update --> Node was already in the kbucket, moving it to the tail of the list");
                    self.kbuckets[bucket_idx].nodes.remove(i);
                    self.kbuckets[bucket_idx].nodes.push(node);
                }
                None => {
                    println!("[VERBOSE] Routing::update --> First time we see this contact, pushing to the tail of the list");
                    self.kbuckets[bucket_idx].nodes.push(node);
                    println!("[DEBUG] Routing::update --> pushed");
                }
            }
        } else {
            let success = self.contact_via_rpc(self.kbuckets[bucket_idx].nodes[0].get_addr());

            // TODO: wait for response, then proceed (we need a mpms channel)
        }
    }

    pub fn remove(&mut self, node: &Node) {
        let bucket_idx = self.get_lookup_bucket_index(&node.id);

        if let Some(i) = self.kbuckets[bucket_idx]
            .nodes
            .iter()
            .position(|x| x.id == node.id)
        {
            self.kbuckets[bucket_idx].nodes.remove(i);
            println!(
                "[VERBOSE] Routing::remove --> removed contact with index: {}",
                i
            );
        } else {
            println!("[WARN] Tried to remove non-existing entry");
        }
    }
}
