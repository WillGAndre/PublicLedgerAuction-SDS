use super::kademlia::{KademliaInstance};
use super::blockchain::Block;
use super::pubsub::PubSubInstance;
use super::node::{Node};
use super::aux::get_ip;
use super::rpc::{full_rpc_proc, KademliaRequest, KademliaResponse, QueryValueResult};
use super::NODETIMEOUT;

use std::thread::{spawn, sleep};
use std::time::Duration;

use std::collections::HashSet;
use base64::decode;

#[derive(Clone)]
pub struct Bootstrap {
    pub nodes: Vec<AppNode>,
    pub bk_history: HashSet<Vec<u8>>
}

impl Bootstrap {
    pub fn new() -> Self {
        let mut res = Vec::new();
        res.push(AppNode::new(get_ip().unwrap(), 1330, None));
        res.push(AppNode::new(get_ip().unwrap(), 1331, None));
        res.push(AppNode::new(get_ip().unwrap(), 1332, None));
        res.push(AppNode::new(get_ip().unwrap(), 1333, None));

        let mut boot = Self {
            nodes: res,
            bk_history: HashSet::new()
        };

        boot = boot.init_sync();

        boot
    }

    fn init_sync(mut self) -> Bootstrap {
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
            let blockchain = self.nodes[i].kademlia.blockchain.lock()
                .expect("Error setting lock in local blockchain");
            self.bk_history.insert(blockchain.hash());
            drop(blockchain);
            i += 1;
        }
        self
    }

    // Note: Added node timeout
    pub fn full_bk_sync(mut boot: Bootstrap) {
        spawn(move || {
            loop {
                sleep(Duration::from_secs(NODETIMEOUT));
                let mut hit: usize = 0;
                let mut hashes: Vec<Vec<u8>> = Vec::new();
                for node in &boot.nodes {
                    let blockchain = node.kademlia.blockchain.lock()
                        .expect("Error setting lock in local blockchain");
                    let blockchain_hash = blockchain.hash();
                    drop(blockchain);
                    if !boot.bk_history.contains(&blockchain_hash) {
                        hit = 1
                    }
                    hashes.push(blockchain_hash);
                }

                if hit == 1 {
                    boot.bk_history.clear();
                    for hash in hashes.pop() {
                        boot.bk_history.insert(hash);
                    }
                    let mut i = 0;
                    while i < boot.nodes.len() {
                        let mut j = 0;
                            while j < boot.nodes.len() {
                                boot.nodes[j].choose_chain(boot.nodes[i].clone());
                                boot.nodes[i].choose_chain(boot.nodes[j].clone());
                                j += 1
                            }
                        i += 1
                    }
                }
            }
        });
    }

    /* TODO: trigger full_sync only if current chain and history are diff: use hash of bks for cmp
    pub fn full_sync(bootstrap: Bootstrap) {
        spawn(move || {
            loop {
                sleep(Duration::from_secs(5)); // Adjust timeout accordingly
                let mut i = 0;
                while i < bootstrap.nodes.len() {
                    let mut j = 0;
                    while j < bootstrap.nodes.len() {
                        if i != j {
                            bootstrap.nodes[j].choose_chain(bootstrap.nodes[i].clone());
                            bootstrap.nodes[i].choose_chain(bootstrap.nodes[j].clone());
                        }
                        j += 1;
                    }
                    i += 1;
                }
            }
        });
    }
    */
}

#[derive(Clone)]
pub struct AppNode {
    pub node: Node,
    pub kademlia: KademliaInstance,
    pub pubsub: PubSubInstance
}

// NOTE: blockchain should be queried before any action
impl AppNode {
    pub fn new(addr: String, port: u16, bootstrap: Option<Node>) -> Self {
        let node = Node::new(addr.clone(), port.clone());
        Self {
            node: node.clone(),
            kademlia: KademliaInstance::new(addr, port, bootstrap),
            pubsub: PubSubInstance::new(node.get_addr(), None, None)
        }
    }

    pub fn publish(&self, topic: String) {
        self.kademlia.insert(topic.clone(), self.pubsub.to_string());
        println!("\t[AN{}]: published topic: {}", self.node.port, topic)
        // TODO: Call pubsub msg loop
        // TODO: Maybe add block when publish is triggered
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

    // register method - arg: AppNode, Note: Added node timeout
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
                    let add_block = full_rpc_proc(&self.kademlia.rpc, KademliaRequest::AddBlock(block), bootnode.node.clone());
                    if let Some(KademliaResponse::Ping) = add_block {
                        sleep(Duration::from_secs(NODETIMEOUT));
                        return true
                    }
                }
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

    // Used to sync bootstrap nodes (AppNode's)
    // TODO: change appnode to reference
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

    /* Used to sync nodes closest to bootnode:
    fn sync_bk(&self, bootnode: AppNode) -> bool {
        let nodeswithdist = self.kademlia.find_node(&bootnode.node.id);
       
        // ---
        // println!("SYNC_BK NODES: {:?}", nodeswithdist);
        // ---

        let blockchain = self.kademlia.blockchain.lock()
            .expect("Error setting lock in blockchain");
        let local_blocks = blockchain.blocks.clone();
        drop(blockchain);
        for NodeWithDistance(node, _) in nodeswithdist {
            let sync_blockchain = full_rpc_proc(&self.kademlia.rpc, KademliaRequest::SyncLocalBlockChain(local_blocks.clone()), node);
            if let Some(KademliaResponse::Ping) = sync_blockchain {
                return true 
            }
        }
        false
    }*/
}

/* 
    App Instance:
    Prerequisites -> AppNode instance & Bootstrap node addr
    Keep BootAppNode reference, used for sync 
*/
pub struct App {
    pub appnode: AppNode,
    pub bootappnode: AppNode
}

impl App {
    pub fn new(addr: String, port: u16, bootappnode: AppNode) -> Self  {
        let bootnode = bootappnode.node.clone();
        let appnode = AppNode::new(addr, port, Some(bootnode));
        appnode.join_network(bootappnode.clone());
        
        Self {
            appnode: appnode,
            bootappnode: bootappnode
        }
    }
}