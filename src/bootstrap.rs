use super::kademlia::{KademliaInstance};
use super::blockchain::Block;
use super::pubsub::PubSubInstance;
use super::node::{Node};
use super::aux::get_ip;
use super::rpc::{full_rpc_proc, KademliaRequest, KademliaResponse, QueryValueResult};

use std::time::Duration;
use std::thread::{spawn, sleep};
use base64::decode;

/**
 *  - Add init function as in rpc.rs
 *      where:
 *              blockchain is broadcasted (within AuthNodes)
 *              routing table  || (|| RoutNodes)
 *              masterNode action
**/
// impl Bootstrap {
//     pub fn new() -> Self {
//         let mut mst = Vec::new();
//         mst.push(MasterNode::new(get_ip().unwrap(), 1330, None));
//         let mut aut = Vec::new();
//         aut.push(AuthorityNode::new(get_ip().unwrap(), 1331, None));
//         aut.push(AuthorityNode::new(get_ip().unwrap(), 1332, None));
//         aut.push(AuthorityNode::new(get_ip().unwrap(), 1333, None));
//         let mut rt = Vec::new();
//         rt.push(RoutingNode::new(get_ip().unwrap(), 1334, None));
//         rt.push(RoutingNode::new(get_ip().unwrap(), 1335, None));
//         let boot = Self {
//             mstrnodes: mst,
//             authnodes: aut,
//             routnodes: rt,
//         };

//         if !boot.sync_routing() {
//             eprintln!("Error syncing bootstrap routing table")
//         }

//         boot
//     }

//     /**
//      * init
//      *  current timeout at 20s
//      * 
//      * note: remember to add timeout in tests
//     **/
//     pub fn init(boot: Bootstrap) {
//         spawn(move || {
//             loop {
//                 sleep(Duration::from_secs(20));
//                 boot.broadcast_blockchain();
//                 boot.broadcast_routingtable();
//                 // master
//             }
//         });
//     }

//     fn sync_routing(&self) -> bool {
//         let mut res = false;
//         let mut size = self.authnodes.len();
//         for i in 0..size {
//             let mut j = i+1;
//             let a1 = &self.authnodes[i];
//             while j < size {
//                 let a2 = &self.authnodes[j];
//                 a1.kademlia.ping(a2.node.clone());
//                 a2.kademlia.ping(a1.node.clone());
//                 j += 1
//             }
//         }
//         size = self.routnodes.len();
//         for i in 0..size {
//             let mut j = i+1;
//             let r1 = &self.routnodes[i];
//             while j < size {
//                 let r2 = &self.routnodes[j];
//                 r1.kademlia.ping(r2.node.clone());
//                 r2.kademlia.ping(r1.node.clone());
//                 j += 1
//             }
//         }
//         for master in &self.mstrnodes {
//             for auth in &self.authnodes {
//                 res = master.kademlia.ping(auth.node.clone());
//             }
//             for route in &self.routnodes {
//                 res = master.kademlia.ping(route.node.clone());
//             }
//         }
//         for route in &self.routnodes {
//             for auth in &self.authnodes {
//                 res = route.kademlia.ping(auth.node.clone());
//             }
//         }

//         res
//     }

//     fn broadcast_blockchain(&self)  {
//         let size = self.authnodes.len();
//         for i in 0..size {
//             let mut j = i+1;
//             let n1 = &self.authnodes[i];
//             while j < size {
//                 let n2 = &self.authnodes[j];
//                 let n1remote = n1.kademlia.query_blockchain(n2.node.clone()).unwrap();
//                 let n2remote = n2.kademlia.query_blockchain(n1.node.clone()).unwrap();
//                 n1.verify_chain(n1remote);
//                 n2.verify_chain(n2remote);
//                 j += 1;
//             }
//         }
//     }

//     /*
//         Only syncs routingtable of
//         routing nodes. Thus, if Lightnode
//         is present in other bootstrap node
//         the "global" routing table wont be notified.
//     */
//     fn broadcast_routingtable(&self) {
//         let mut newnodes: Vec<Node> = Vec::new();
//         let size = self.routnodes.len();
//         for i in 0..size {
//             let mut j = i+1;
//             let rn1 = &self.routnodes[i];
//             while j < size {
//                 let rn2 = &self.routnodes[j];
//                 newnodes.append(&mut rn1.get_unknown_nodes(rn2));
//                 newnodes.append(&mut rn2.get_unknown_nodes(rn1));
//                 j = j+1;
//             }
//         }

//         if !newnodes.is_empty() {
//             for rn in self.routnodes.iter() {
//                 rn.update_routing_table(newnodes.clone());
//             }
//         }
//     }
// }

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
pub struct Bootstrap {
    pub nodes: Vec<AppNode>
}

impl Bootstrap {
    pub fn new() -> Self {
        let mut res = Vec::new();
        res.push(AppNode::new(get_ip().unwrap(), 1330, None));
        res.push(AppNode::new(get_ip().unwrap(), 1331, None));
        res.push(AppNode::new(get_ip().unwrap(), 1332, None));
        res.push(AppNode::new(get_ip().unwrap(), 1333, None));

        let boot = Self {
            nodes: res
        };

        boot.sync();

        boot
    }

    fn sync(&self) {
        let mut i = 0;
        while i < self.nodes.len() {
            self.nodes[i].add_block(format!("REGISTER: {id}", id=self.nodes[i].node.get_addr()));
            let mut j = 0;
            while j < self.nodes.len() {
                if i != j {
                    self.nodes[i].kademlia.ping(self.nodes[j].node.clone());
                    self.nodes[j].kademlia.ping(self.nodes[i].node.clone());
                    self.nodes[j].choose_chain(self.nodes[i].clone());
                    self.nodes[i].choose_chain(self.nodes[j].clone());
                }
                j += 1;
            }
            i += 1;
        }
    }
}

#[derive(Clone)]
pub struct AppNode {
    pub node: Node,
    pub kademlia: KademliaInstance,
    pub pubsub: PubSubInstance,
    pub subs: Vec<PubSubInstance>
}

// NOTE: blockchain should be queried before any action
/*
    TODO: Change publish/subscribe/add_msg to blockchain
*/
impl AppNode {
    pub fn new(addr: String, port: u16, bootstrap: Option<Node>) -> Self {
        let node = Node::new(addr.clone(), port.clone());
        Self {
            node: node.clone(),
            kademlia: KademliaInstance::new(addr, port, bootstrap),
            pubsub: PubSubInstance::new(node.get_addr(), None, None),
            subs: Vec::new(),
        }
    }

    pub fn publish(&self, topic: String) {
        self.kademlia.insert(topic.clone(), self.pubsub.to_string());
        println!("\t[AN{}]: published topic: {}", self.node.port, topic)
        // TODO: Call pubsub msg loop
    }

    pub fn subscribe(&self, topic: String) -> bool {
        let pubsub = self.kademlia.get(topic.clone());
        if pubsub == None {
            println!("\t[AN{}]: Error subscribing - couldn't find topic: {}", self.node.port, topic)
        } else {
            let pubsub_str = pubsub.unwrap();
            let pubsub_ins = self.get_pubsub_instance(pubsub_str).unwrap();
            pubsub_ins.add_sub(self.node.get_addr());
            self.kademlia.insert(topic.clone(), pubsub_ins.to_string());
            println!("\t[AN{}]: subscribed to topic: {}", self.node.port, topic);
            return true
        }
        
        false
    }

    pub fn add_msg(&self, topic: String, msg: String) -> bool {
        let pubsub = self.kademlia.get(topic.clone());
        if pubsub == None {
            println!("\t[AN{}]: Error adding msg - couldn't find topic: {}", self.node.port, topic)
        } else {
            let pubsub_str = pubsub.unwrap();
            let pubsub_ins = self.get_pubsub_instance(pubsub_str).unwrap();
            if pubsub_ins.verify(self.node.get_addr()) {
                pubsub_ins.add_msg(msg);
                self.kademlia.insert(topic.clone(), pubsub_ins.to_string());
                println!("\t[AN{}]: added msg to topic: {}", self.node.port, topic);
                return true
            }
        }
        
        false
    }

    // register method - arg: AppNode
    pub fn join_network(&self, bootnode: AppNode) -> bool {
        let find_node = full_rpc_proc(&self.kademlia.rpc, KademliaRequest::NodeJoin(self.node.clone()), bootnode.node.clone());
        
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
                
                let query_blockchain = full_rpc_proc(&self.kademlia.rpc, KademliaRequest::QueryLocalBlockChain, bootnode.node.clone());

                if let Some(KademliaResponse::QueryLocalBlockChain(blocks)) = query_blockchain {
                    let mut blockchain = self.kademlia.blockchain.lock()
                        .expect("Error setting lock in local blockchain");
                    blockchain.blocks = blockchain.choose_chain(blockchain.blocks.clone(), blocks);

                    let id = blockchain.blocks[blockchain.blocks.len() - 1].id + 1;
                    let prev_hash = blockchain.blocks[blockchain.blocks.len() - 1].hash.to_string();
                    let block = Block::new(id, prev_hash, format!("REGISTER: {id}", id=self.node.get_addr()));

                    blockchain.add_block(block.clone());
                    drop(blockchain);

                    // TODO
                    let add_block = full_rpc_proc(&self.kademlia.rpc, KademliaRequest::AddBlock(block), bootnode.node);
                    if let Some(KademliaResponse::Ping) = add_block {
                        return true
                    }
                }

                return true
            } else {
                println!("\t[AN{}]: Error joining network - No nearby nodes found", self.node.port)
            }
        } else {
            // TODO
            println!("\t[AN{}]: Error joining network", self.node.port)
        }
        false
    }

    pub fn add_block(&self, data: String) {
        let block = self.mine_block(data.clone());
        let mut blockchain = self.kademlia.blockchain.lock()
            .expect("Error setting lock in local blockchain");
        blockchain.add_block(block.clone());
        drop(blockchain);
        
        // ---
        println!("\t[AN{}]: Added Block info ({})", self.node.port, data)
    }

    fn mine_block(&self, data: String) -> Block {
        let blockchain = self.kademlia.blockchain.lock()
            .expect("Error setting lock in local blockchain");
        let id = blockchain.blocks[blockchain.blocks.len() - 1].id + 1;
        let prev_hash = blockchain.blocks[blockchain.blocks.len() - 1].hash.to_string();
        drop(blockchain);

        Block::new(id, prev_hash, data)
    }

    fn choose_chain(&self, appnode: AppNode) {
        let query_blockchain = full_rpc_proc(&self.kademlia.rpc, KademliaRequest::QueryLocalBlockChain, appnode.node.clone());
        if let Some(KademliaResponse::QueryLocalBlockChain(remoteblocks)) = query_blockchain {
            let mut blockchain = self.kademlia.blockchain.lock()
                .expect("Error setting lock in blockchain");
            blockchain.blocks = blockchain.choose_chain(blockchain.blocks.clone(), remoteblocks.clone());
            drop(blockchain);

            // ---
            println!("\t[AN{}]: Updated blockchain ({:?})", self.node.port, remoteblocks)
        }
    }

    fn get_pubsub_instance(&self, data: String) -> Option<PubSubInstance> {
        let decoded_data = String::from_utf8(decode(data.to_string()).expect("Error decoding data"))
            .expect("Error converting data to string");
        let pattern: &[_] = &['[', ']'];
        let data_vec: Vec<&str> = decoded_data.split(";").collect();
        if !data_vec.is_empty() {
            let publisher: String = String::from(data_vec[0]);
            let substack: Vec<String> = data_vec[1].trim_matches(pattern).split(' ').map(|s| String::from(s)).collect();
            let msgstack: Vec<String> = data_vec[2].trim_matches(pattern).split(' ').map(|s| String::from(s)).collect();
            return Some(PubSubInstance::new(publisher, Some(msgstack), Some(substack)));
        }
        None
    }

    // TESTING: subcribe from oth node
    pub fn subscribe_network(&self, topic: String, bootnode: AppNode) -> bool {
        let pubsub = self.kademlia.query_value(bootnode.node, topic.clone());

        if let Some(pubsub) = pubsub {
            if let QueryValueResult::Value(pubsub_str) = pubsub {
                let pubsub_ins = self.get_pubsub_instance(pubsub_str).unwrap();
                pubsub_ins.add_sub(self.node.get_addr());
                self.kademlia.insert(topic.clone(), pubsub_ins.to_string());
                println!("\t[AN{}]: subscribed to topic: {}", self.node.port, topic);
                return true
            }
            println!("\t[AN{}]: Error subscribing to topic: {}", self.node.port, topic)
        }
        println!("\t[AN{}]: Error subscribing to topic: {}", self.node.port, topic);
        false
    }
}

//  ***
//  ---
//  ***

// impl AuthorityNode {
//     pub fn new(addr: String, port: u16, bootstrap: Option<Node>) -> Self {
//         Self {
//             node: Node::new(addr.clone(), port.clone()),
//             kademlia: KademliaInstance::new(addr, port, bootstrap)
//         }
//     }

//     pub fn add_block(&self, block: Block) {
//         let mut blockchain = self.kademlia.blockchain.lock()
//             .expect("Error setting lock in local blockchain");
//         blockchain.add_block(block.clone());
//     }

//     pub fn verify_chain(&self, remote: Vec<Block>) {
//         let mut blockchain = self.kademlia.blockchain.lock()
//             .expect("Error setting lock in blockchain");
//         let chain = blockchain.choose_chain(blockchain.blocks.clone(), remote);
//         blockchain.blocks = chain;
//     }
// }

// impl RoutingNode {
//     pub fn new(addr: String, port: u16, bootstrap: Option<Node>) -> Self {
//         Self {
//             node: Node::new(addr.clone(), port.clone()),
//             kademlia: KademliaInstance::new(addr, port, bootstrap)
//         }
//     }

//     fn update_routing_table(&self, nodes: Vec<Node>) {
//         let mut routingtable = self.kademlia.routingtable.lock()
//             .expect("Error setting lock in routing table");

//         for node in nodes {
//             routingtable.update_routing_table(node);
//         }
//     } 

//     fn get_unknown_nodes(&self, remotenode: &RoutingNode) -> Vec<Node> {
//         let localroutingtable = self.kademlia.routingtable.lock()
//             .expect("Error setting lock in routing table");
//         let remoteroutingtable = remotenode.kademlia.routingtable.lock()
//             .expect("Error setting lock in routing table");

//         let mut nodes = Vec::new();
//         for bucket in remoteroutingtable.kbuckets.iter() {
//             for node in bucket.nodes.iter() {
//                 if !localroutingtable.contains_node(&node.id) {
//                     nodes.push(node.clone())
//                 }
//             }
//         }
//         drop(localroutingtable);
//         drop(remoteroutingtable);

//         nodes
//     }
// }

// impl LightNode {
//     pub fn new(addr: String, port: u16, bootstrap: Option<Node>) -> Self {
//         Self {
//             node: Node::new(addr.clone(), port.clone()),
//             kademlia: KademliaInstance::new(addr, port, bootstrap)
//         }
        
//         // range addr / range ports
//         //  1355: 1340-1370 / 192.178.0.1: 192.178.0.1
//         // ping
//         // join_network
//     }

//     /*
//         Join Network:
//          In order for the LightNode to join,
//          a bootstrap node must be known.
//     */
//     pub fn join_network(&self, bootnode: Node) -> bool {
//         let find_node = full_rpc_proc(&self.kademlia.rpc, KademliaRequest::NodeJoin(self.node.clone()), bootnode);
        
//         if let Some(KademliaResponse::NodeJoin(nodes)) = find_node {
//             if !nodes.is_empty() {
//                 for node in nodes {
//                     if node.id != self.node.id {
//                         full_rpc_proc(&self.kademlia.rpc, KademliaRequest::NodeJoin(self.node.clone()), node.clone());
//                         let mut routingtable = self.kademlia.routingtable.lock()
//                             .expect("Error setting lock in routing table");
//                         routingtable.update_routing_table(node);
//                         drop(routingtable)
//                     }
//                 }
//                 return true
//             } else {
//                 println!("\t[LT{}]: Error joining network - No nearby nodes found", self.node.port)
//             }
//         } else {
//             // TODO
//             println!("\t[LT{}]: Error joining network", self.node.port)
//         }
//         false
//     }
// }

//  ***
//  ---
//  ***