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
    pub lightnodes: Vec<LightNode>,
}

/**
 * TODO:
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
            lightnodes: Vec::new(),
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
                // routing 
                // master
            }
        });
    }

    fn sync_routing(&self) -> bool {
        let mut res = false;
        let mut size = self.authnodes.len();
        for i in 0..size {
            let mut j = i+1;
            while j < size {
                let a1 = &self.authnodes[i];
                let a2 = &self.authnodes[j];
                a1.kademlia.ping(a2.node.clone());
                a2.kademlia.ping(a1.node.clone());
                j += 1
            }
        }
        size = self.routnodes.len();
        for i in 0..size {
            let mut j = i+1;
            while j < size {
                let r1 = &self.routnodes[i];
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
    }

        /*
         Update global routing table:
            - Always keep bootstrap nodes in routing table,
              even if bucket is full.
            - Look for inconsistencies in RoutingNodes
              routing table.
                \
                 -> If new node was added:
                     ping node to verify that its online,
                     if so update routingtable in RoutingNodes.
                \
                 -> Add new nodes to Bootstrap history of LightNodes,
                    and update this history (remove old nodes).
        */

    // fn broadcast_routingtable(&self) {
    //     let size = self.routnodes.len();
    //     for i in 0..size {

    //     }
    // }
}

/*
    TODO:
        MasterNode:
            Functionalities - Validate global blockchain (local AuthNode blockchain) with local /
                              Update global blockchain (given error, etc) /
                              Save hard copy of authNode blockchain in hashmap / 
                              Verify block transactions
        
        AuthorityNode:
            Functionalities - Receive mining requests (process block addition to blockchain) /
                              Distribute blockchain information (as broadcast)

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

    // TODO: Add function to update routing table 
    // with nodes not present in current routing table
    // but present in routing table of a node belonging
    // to bootstrap and sync routing tables of nodes. 
    // fn contains_node(&self, node: Node) -> bool {
    //     let routingtable = self.kademlia.routingtable.lock()
    //         .expect("Error setting lock in routing table");
    //     let res = routingtable.get_bucket_nodes(&node.id);
    //     drop(routingtable);

    //     if res.is_empty() {
    //         return false
    //     }
    //     match res.iter().position(|n| n.0.id == node.id) {
    //         Some(_) => {
    //             return true
    //         },
    //         None => {
    //             return false
    //         }
    //     }
    // }

    // TODO:
    // fn get_unknown_nodes(&self, remotenode: RoutingNode) {
    //     let localroutingtable = self.kademlia.routingtable.lock()
    //         .expect("Error setting lock in routing table");

    //     let remoteroutingtable = remotenode.kademlia.routingtable.lock()
    //         .expect("Error setting lock in routing table");

    //     for (localbucket, remotebucket) in localroutingtable.kbuckets.iter()
    //         .zip(remoteroutingtable.kbuckets.iter()) {
    //             let localnodes = localbucket.nodes.clone();
    //             let remotenodes = remotebucket.nodes.clone();
    //     }
    // }
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