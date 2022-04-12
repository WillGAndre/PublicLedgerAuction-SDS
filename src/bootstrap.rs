use super::kademlia::KademliaInstance;
use super::blockchain::Block;
use super::node::{Node};
use super::aux::get_ip;


use std::time::Duration;
use std::thread::{spawn, sleep};

#[derive(Clone)]
pub struct Bootstrap {
    pub mstrnodes: Vec<MasterNode>,
    pub authnodes: Vec<AuthorityNode>,
    pub routnodes: Vec<RoutingNode>,
}

impl Bootstrap {
    pub fn new() -> Self {
        let mut mst = Vec::new();
        mst.push(MasterNode::new(get_ip().unwrap(), 1330, None));
        let mut aut = Vec::new();
        aut.push(AuthorityNode::new(get_ip().unwrap(), 1331, None));
        aut.push(AuthorityNode::new(get_ip().unwrap(), 1332, None));
        aut.push(AuthorityNode::new(get_ip().unwrap(), 1333, None));
        let mut rt = Vec::new();
        rt.push(RoutingNode::new(get_ip().unwrap(), 1334, None));
        rt.push(RoutingNode::new(get_ip().unwrap(), 1335, None));
        let boot = Self {
            mstrnodes: mst,
            authnodes: aut,
            routnodes: rt,
        };

        if !boot.sync_routing() {
            eprintln!("Error syncing bootstrap routing table")
        }

        let bootclone = boot.clone();
        spawn(move || {
            // TODO: ADD TIMEOUT
            bootclone.broadcast_blockchain();
        });

        boot
    }

    fn sync_routing(&self) -> bool {
        let mut res = false;
        for master in &self.mstrnodes {
            for auth in &self.authnodes {
                res = master.kademlia.ping(auth.node.clone());
            }
            for route in &self.routnodes {
                res = master.kademlia.ping(route.node.clone());
            }
        }
        for route in &self.routnodes {
            for auth in &self.authnodes {
                res = route.kademlia.ping(auth.node.clone());
            }
        }

        res
    }

    pub fn broadcast_blockchain(&self)  { // TODO
        //loop {
            //sleep(Duration::from_secs(20));
            let size = self.authnodes.len();
            for i in 0..size {
                let mut j = i+1;
                while j < size {
                    let n1 = &self.authnodes[i];
                    let n2 = &self.authnodes[j];
                    let n1remote = n1.kademlia.query_blockchain(n2.node.clone()).unwrap();
                    let n2remote = n2.kademlia.query_blockchain(n1.node.clone()).unwrap();
                    n1.verify_chain(n1remote);
                    n2.verify_chain(n2remote);
                    j += 1;
                }
            }
        //}
    }
}

/*
    TODO:
        MasterNode:
            Functionalities - Validate global blockchain (local AuthNode blockchain) with local /
                              Update global blockchain (given error, etc) /
                              Save hard copy of authNode blockchain in hashmap / 
                              Verify block transactions
        
        AuthorityNode: (G)
            Functionalities - Receive mining requests (process block addition to blockchain) /
                              Distribute blockchain information (as broadcast)

        RoutingNode:   (G)
            Functionalities - Distribute routing information /
                              Maintain bootstrap nodes in routing table at all costs /
                              Handle new nodes and their addition to the network /
                              Relay AuthorityNode messages 

        LightNode:
            Functionalities - Simple Network Entity /
                              Mining capabilities /
                              General node functionalities (routing table / hashmap) /
    --- LINKS:
     https://nodes.com/
     https://www.sofi.com/learn/content/what-are-nodes/
    ---
*/

#[derive(Clone)]
pub struct MasterNode {
    pub node: Node,
    pub kademlia: KademliaInstance
}

impl MasterNode { // T
    pub fn new(addr: String, port: u16, bootstrap: Option<Node>) -> Self {
        Self {
            node: Node::new(addr.clone(), port.clone()),
            kademlia: KademliaInstance::new(addr, port, bootstrap)
        }
    }
}

#[derive(Clone)]
pub struct AuthorityNode { // G
    pub node: Node,
    pub kademlia: KademliaInstance
}

impl AuthorityNode {
    pub fn new(addr: String, port: u16, bootstrap: Option<Node>) -> Self {
        Self {
            node: Node::new(addr.clone(), port.clone()),
            kademlia: KademliaInstance::new(addr, port, bootstrap)
        }
    }

    pub fn add_block(&self, block: Block) {
        let mut blockchain = self.kademlia.blockchain.lock()
            .expect("Error setting lock in local blockchain");
        blockchain.add_block(block.clone());
    }


    pub fn verify_chain(&self, remote: Vec<Block>) {
        let mut blockchain = self.kademlia.blockchain.lock()
            .expect("Error setting lock in blockchain");
        let chain = blockchain.choose_chain(blockchain.blocks.clone(), remote);
        blockchain.blocks = chain;
    }
}

#[derive(Clone)]
pub struct RoutingNode { // G
    pub node: Node,
    pub kademlia: KademliaInstance
}

impl RoutingNode {
    pub fn new(addr: String, port: u16, bootstrap: Option<Node>) -> Self {
        Self {
            node: Node::new(addr.clone(), port.clone()),
            kademlia: KademliaInstance::new(addr, port, bootstrap)
        }
    }
}

pub struct LightNode { // T
    pub node: Node,
    pub kademlia: KademliaInstance
}

impl LightNode {
    pub fn new(addr: String, port: u16, bootstrap: Option<Node>) -> Self {
        Self {
            node: Node::new(addr.clone(), port.clone()),
            kademlia: KademliaInstance::new(addr, port, bootstrap)
        }
    }
}

//  ***
//  ---
//  ***