use super::kademlia::{KademliaInstance};
use super::blockchain::Block;
use super::node::{Node};
use super::aux::get_ip;
use super::rpc::{full_rpc_proc, KademliaRequest, KademliaResponse};

use std::time::Duration;
use std::thread::{spawn, sleep};

#[derive(Clone)]
pub struct Bootstrap {
    pub mstrnodes: Vec<MasterNode>,
    pub authnodes: Vec<AuthorityNode>,
    pub routnodes: Vec<RoutingNode>,
}

/**
 *  - Add init function as in rpc.rs
 *      where:
 *              blockchain is broadcasted (within AuthNodes)
 *              routing table  || (|| RoutNodes)
 *              masterNode action
**/
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

        boot
    }

    /**
     * init
     *  current timeout at 20s
     * 
     * note: remember to add timeout in tests
    **/
    pub fn init(boot: Bootstrap) {
        spawn(move || {
            loop {
                sleep(Duration::from_secs(20));
                boot.broadcast_blockchain();
                boot.broadcast_routingtable();
                // master
            }
        });
    }

    fn sync_routing(&self) -> bool {
        let mut res = false;
        let mut size = self.authnodes.len();
        for i in 0..size {
            let mut j = i+1;
            let a1 = &self.authnodes[i];
            while j < size {
                let a2 = &self.authnodes[j];
                a1.kademlia.ping(a2.node.clone());
                a2.kademlia.ping(a1.node.clone());
                j += 1
            }
        }
        size = self.routnodes.len();
        for i in 0..size {
            let mut j = i+1;
            let r1 = &self.routnodes[i];
            while j < size {
                let r2 = &self.routnodes[j];
                r1.kademlia.ping(r2.node.clone());
                r2.kademlia.ping(r1.node.clone());
                j += 1
            }
        }
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

    fn broadcast_blockchain(&self)  {
        let size = self.authnodes.len();
        for i in 0..size {
            let mut j = i+1;
            let n1 = &self.authnodes[i];
            while j < size {
                let n2 = &self.authnodes[j];
                let n1remote = n1.kademlia.query_blockchain(n2.node.clone()).unwrap();
                let n2remote = n2.kademlia.query_blockchain(n1.node.clone()).unwrap();
                n1.verify_chain(n1remote);
                n2.verify_chain(n2remote);
                j += 1;
            }
        }
    }

    /*
        Only syncs routingtable of
        routing nodes. Thus, if Lightnode
        is present in other bootstrap node
        the "global" routing table wont be notified.
    */
    fn broadcast_routingtable(&self) {
        let mut newnodes: Vec<Node> = Vec::new();
        let size = self.routnodes.len();
        for i in 0..size {
            let mut j = i+1;
            let rn1 = &self.routnodes[i];
            while j < size {
                let rn2 = &self.routnodes[j];
                newnodes.append(&mut rn1.get_unknown_nodes(rn2));
                newnodes.append(&mut rn2.get_unknown_nodes(rn1));
                j = j+1;
            }
        }

        if !newnodes.is_empty() {
            for rn in self.routnodes.iter() {
                rn.update_routing_table(newnodes.clone());
            }
        }
    }
}

//  ***
//  ---
//  ***


/*
    Upon creation, AppNode needs to:
        - join kad network
        - get global blockchain and register (mine/add_block) block
        in blockchain
*/

#[derive(Clone)]
pub struct AppNode {
    pub node: Node,
    pub kademlia: KademliaInstance
}

//  ***
//  ---
//  ***

/*
        MasterNode:
            Functionalities - Validate global blockchain (local AuthNode blockchain) with local /
                              Update global blockchain (given error, etc) /
                              Save hard copy of authNode blockchain in hashmap / 
                              Verify block transactions
        
        AuthorityNode:
            Functionalities - Receive mining requests (process block addition to blockchain) /
                              Distribute blockchain information (as broadcast) /
                              Keep persistent up to date copy of blockchain in hashmap (?)
                                \
                                 --> Continue bootstrap instance from previous blockchain
                                     stored persistently

        RoutingNode:
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

    fn update_routing_table(&self, nodes: Vec<Node>) {
        let mut routingtable = self.kademlia.routingtable.lock()
            .expect("Error setting lock in routing table");

        for node in nodes {
            routingtable.update_routing_table(node);
        }
    } 

    fn get_unknown_nodes(&self, remotenode: &RoutingNode) -> Vec<Node> {
        let localroutingtable = self.kademlia.routingtable.lock()
            .expect("Error setting lock in routing table");
        let remoteroutingtable = remotenode.kademlia.routingtable.lock()
            .expect("Error setting lock in routing table");

        let mut nodes = Vec::new();
        for bucket in remoteroutingtable.kbuckets.iter() {
            for node in bucket.nodes.iter() {
                if !localroutingtable.contains_node(&node.id) {
                    nodes.push(node.clone())
                }
            }
        }
        drop(localroutingtable);
        drop(remoteroutingtable);

        nodes
    }
}

#[derive(Clone)]
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
        
        // range addr / range ports
        //  1355: 1340-1370 / 192.178.0.1: 192.178.0.1
        // ping
        // join_network
    }

    /*
        Join Network:
         In order for the LightNode to join,
         a bootstrap node must be known.
    */
    pub fn join_network(&self, bootnode: Node) -> bool {
        let find_node = full_rpc_proc(&self.kademlia.rpc, KademliaRequest::NodeJoin(self.node.clone()), bootnode);
        
        if let Some(KademliaResponse::NodeJoin(nodes)) = find_node {
            if !nodes.is_empty() {
                for node in nodes {
                    if node.id != self.node.id {
                        full_rpc_proc(&self.kademlia.rpc, KademliaRequest::NodeJoin(self.node.clone()), node.clone());
                        let mut routingtable = self.kademlia.routingtable.lock()
                            .expect("Error setting lock in routing table");
                        routingtable.update_routing_table(node);
                        drop(routingtable)
                    }
                }
                return true
            } else {
                println!("\t[LT{}]: Error joining network - No nearby nodes found", self.node.port)
            }
        } else {
            // TODO
            println!("\t[LT{}]: Error joining network", self.node.port)
        }
        false
    }
}

//  ***
//  ---
//  ***