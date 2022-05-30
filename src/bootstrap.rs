use super::kademlia::{KademliaInstance};
use super::blockchain::Block;
use super::pubsub::PubSubInstance;
use super::node::{Node};
use super::aux::get_ip;
use super::rpc::{full_rpc_proc, KademliaRequest, KademliaResponse, QueryValueResult};
use super::NODETIMEOUT;

use std::sync::{Arc, Mutex};
use std::thread::{spawn, sleep};
use std::time::Duration;
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use serde_json::Result;
use std::collections::{HashMap};

use base64::decode;

#[derive(Clone)]
pub struct Bootstrap {
    pub nodes: Vec<AppNode>,
    pub bk_hash: Vec<u8>,
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
            bk_hash: Vec::new()
        };

        boot = boot.init_sync();

        boot
    }

    fn init_sync(mut self) -> Bootstrap {
        let mut global_hash: Option<Vec<u8>> = None;
        let mut i = 0;
        while i < self.nodes.len() {
            self.nodes[i].add_block(Data::new(format!("REGISTER: {id}", id=self.nodes[i].node.get_addr()), 0, None).to_json());
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
            global_hash = Some(blockchain.hash());
            drop(blockchain);
            i += 1;
        }
        self.bk_hash = global_hash.unwrap();
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
                    if blockchain_hash != boot.bk_hash {
                        hit = 1
                    }
                    hashes.push(blockchain_hash);
                }

                if hit == 1 {
                    hashes.clear();
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
                    let mut global_hash: Option<Vec<u8>> = None;
                    for node in &boot.nodes {
                        let blockchain = node.kademlia.blockchain.lock()
                            .expect("Error setting lock in local blockchain");
                        let blockchain_hash = blockchain.hash();
                        drop(blockchain);
                        if global_hash == None {
                            global_hash = Some(blockchain_hash)
                        } else if global_hash != Some(blockchain_hash) {
                            println!("\t[BOOT]: FULL SYNC - Error synchronizing blockchain");
                            break; // sync next timeout
                        }
                    }
                    boot.bk_hash = global_hash.unwrap();
                }
            }
        });
    }
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

    pub fn publish(&self, topic: String, ttl: DateTime<Local>) {
        let mut pubsub = self.pubsub.clone();
        pubsub.set_ttl(ttl);
        self.kademlia.insert(topic.clone(), pubsub.to_string());
        println!("\t[AN{}]: Published topic (in DHT): {}; Exp: {}", self.node.port, topic, ttl)
        // TODO: Call pubsub msg loop
        // TODO: Maybe add block when publish is triggered, set ttl for pubsub instance 
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
            if pubsub_ins.verify_addr(self.node.get_addr()) {
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
                    let data = Data::new(
                        format!("REGISTER: {id}", id=self.node.get_addr()), 
                        0,
                        None
                    );
                    let block = Block::new(id, prev_hash, data.to_json());

                    blockchain.add_block(block.clone());
                    drop(blockchain);

                    let add_block = full_rpc_proc(&self.kademlia.rpc, KademliaRequest::AddBlock(block), bootnode.node.clone());
                    if let Some(KademliaResponse::Ping) = add_block {
                        println!("\t[AN{}]: Added Block info ({})", self.node.port, data.to_json());
                        sleep(Duration::from_secs(NODETIMEOUT));
                        return true
                    } else if let Some(KademliaResponse::PingUnableProcReq) = add_block {
                        let mut blockchain = self.kademlia.blockchain.lock()
                            .expect("Error setting lock in local blockchain");
                        blockchain.remove_last_block();
                        drop(blockchain);
                        println!("\t[AN{}]: Unable to add block info ({})", self.node.port, data.to_json());
                        return false
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
        let res = blockchain.add_block(block.clone());
        drop(blockchain);
        
        // ---
        if res {
            println!("\t[AN{}]: Added Block info ({})", self.node.port, data)
        }
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
            // println!("\t[AN{}]: Updated blockchain ({:?})", self.node.port, remoteblocks)
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
            let ttl: DateTime<Local> = data_vec[3].parse().unwrap();
            let mut pubsub = PubSubInstance::new(publisher, Some(msgstack), Some(substack));
            pubsub.set_ttl(ttl);
            return Some(pubsub);
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

#[derive(Serialize, Deserialize, Clone)]
pub struct Data {
    msg: String,
    msg_type: usize,
    exp_time: Option<String>
}

impl Data {
    pub fn new(msg: String, msg_type: usize, exp_time: Option<String>) -> Self {
        Self {
            msg: msg,
            msg_type: msg_type,
            exp_time: exp_time,
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(&self).unwrap()
    }
}

/* 
    App Instance:
    Prerequisites -> AppNode instance & Bootstrap node addr
    Keep BootAppNode reference, used for sync 
*/
#[derive(Clone)]
pub struct App {
    pub appnode: AppNode,
    pub bootappnode: AppNode,
    pub topics: Arc<Mutex<Vec<String>>>,
}

/*
    topics: 
        <topic;{obj: "", highest_bid: "", highest_bidder: "", publisher: "", ttl: ""}>
*/
#[derive(Serialize, Deserialize, Clone)]
pub struct BidSession {
    pub topic: String,
    pub obj: String,
    pub highest_bid: String,
    pub highest_bidder: String,
    pub publisher: String,
    pub ttl: String,
}

impl BidSession {
    pub fn new(topic: String, obj: String, 
        highest_bid: String, highest_bidder: String, 
        publisher: String, ttl: String) -> Self {
            Self {
                topic: topic,
                obj: obj,
                highest_bid: highest_bid,
                highest_bidder: highest_bidder,
                publisher: publisher,
                ttl: ttl
            }    
        }
    pub fn to_json(&self) -> String {
        serde_json::to_string(&self).unwrap()
    }
}

/*
    BK MSG:
        NETWORK:
        - REGISTER

        PUBSUB:
        - NEW TOPIC
        - DELETE TOPIC
        - FINISH TOPIC
            \
             \
              -> TRANSACTIONS: <IP:PORT> -> <IP:PORT> ; $$$
*/

/*
    APP MSG:
        - SUBSCRIBE TOPIC
            {msg: BID <TOPIC> X, sender:}
            {msg: LEAVE <TOPIC>, sender:}
        
        - PUBLISH TOPIC
            PUBLISH:
            "
                > topic session has started
                > starting bid at: 100$$

                (new bid > bid (100))
                > NEW BID: X$$
            "
            BID <TOPIC> X -> pubsub struct ( split(str, ' ') -> [1] -> regex([0-9]+) )
            LEAVE <TOPIC>
*/

/*
    TODO:
        - periodically request blockchain
        - pubsub topics/msgs alerts
*/
impl App {
    pub fn new(addr: String, port: u16, bootappnode: AppNode) -> Self  {
        let bootnode = bootappnode.node.clone();
        let appnode = AppNode::new(addr, port, Some(bootnode));
        let mut register = appnode.join_network(bootappnode.clone());
        let mut iter = 2;
        while !register {
            if iter == 5 { break; }
            sleep(Duration::from_secs(NODETIMEOUT));
            register = appnode.join_network(bootappnode.clone());
            iter += 1;
        }
        
        let app = Self {
            appnode: appnode,
            bootappnode: bootappnode,
            topics: Arc::new(Mutex::new(Vec::new())),
        };

        App::pull_bk(app.clone());

        app
    }

    fn pull_bk(app: App) {
        spawn(move || {
            loop {
                sleep(Duration::from_secs(NODETIMEOUT));
                let query_blockchain = full_rpc_proc(&app.appnode.kademlia.rpc, KademliaRequest::QueryLocalBlockChain, app.bootappnode.node.clone());
                if let Some(KademliaResponse::QueryLocalBlockChain(blocks)) = query_blockchain {
                    let mut blockchain = app.appnode.kademlia.blockchain.lock()
                        .expect("Error setting lock in local blockchain");
                    let new_blocks = blockchain.get_diff_from_chains(blockchain.blocks.clone(), blocks.clone());
                    blockchain.blocks = blockchain.choose_chain(blockchain.blocks.clone(), blocks);
                    
                    for block in new_blocks.clone() {
                        let data: Data = serde_json::from_str(&block.data).
                            expect("Error converting data to json");
                        match data.msg_type {
                            0 => {},
                            1 => {
                                let ttl: DateTime<Local> = data.exp_time.unwrap().parse().unwrap();
                                let ttl_str = format!("{}", ttl);
                                let time = Local::now();
                                let diff = (ttl - time).num_minutes();
                                if diff > 0 {
                                    let mut topics = app.topics.lock()
                                        .expect("Error setting lock in topics vec!");
                                    let topic = format!("{};{}", data.msg, ttl_str);
                                    if !topics.contains(&topic) {
                                        topics.push(topic);
                                    }
                                    drop(topics);
                                }
                            },
                            _ => {},
                        };
                    }
                }

            }
        });
    }

    pub fn publish(&self, topic: String) -> bool {
        let ttl = Local::now() + chrono::Duration::minutes(15);
        let ttl_str = format!("{}", ttl);
        let data = Data::new(
            format!("PUB_TOPIC: {}", topic), 
            1,
            Some(format!("{}", ttl)),
        );
        if self.pull_bk_add_block(data.clone()) {
            // TODO: error handeling
            self.appnode.publish(topic, ttl);

            let topic = format!("{};{}", data.msg, ttl_str);
            let mut topics = self.topics.lock()
                .expect("Error setting lock in topics vec!");
            if !topics.contains(&topic) {
                topics.push(topic);
            }
            drop(topics);


            sleep(Duration::from_secs(NODETIMEOUT));
            return true
        }

        // Call pubsub teardown loop:
        //  if addr == publisher addr and on pubsub exp_time hit: Send BK Block (end topic + transactions) 

        false
    }

    pub fn subscribe(&self, topic: String) -> bool {
        let topics = self.topics.lock()
            .expect("Error setting lock in topics vec!");
        let topics_vec: Vec<String> = topics.clone().into_iter().collect();
        drop(topics);

        let mut sub = false;
        if !topics_vec.is_empty() {
            for topic_state in topics_vec {
                let topic_str: Vec<&str> = topic_state.split(";").collect();
                if topic_str[0] == topic.clone() {
                    let ttl: DateTime<Local> = topic_str[1].parse().unwrap();
                    let time = Local::now();
                    let diff = (ttl - time).num_minutes();
                    if diff > 0 {
                        sub = self.appnode.subscribe(topic.clone());
                        break
                    }
                }
            }
        } else {
            sub = self.appnode.subscribe(topic.clone());
        }

        if sub {
            // TODO: call loop to receive msgs
        }
        
        sub
    }

    fn pull_bk_add_block(&self, data: Data) -> bool {
        let query_blockchain = full_rpc_proc(&self.appnode.kademlia.rpc, KademliaRequest::QueryLocalBlockChain, self.bootappnode.node.clone());
        if let Some(KademliaResponse::QueryLocalBlockChain(blocks)) = query_blockchain {
            let mut blockchain = self.appnode.kademlia.blockchain.lock()
                .expect("Error setting lock in local blockchain");
            blockchain.blocks = blockchain.choose_chain(blockchain.blocks.clone(), blocks);

            let id = blockchain.blocks[blockchain.blocks.len() - 1].id + 1;
            let prev_hash = blockchain.blocks[blockchain.blocks.len() - 1].hash.to_string();
            let block = Block::new(id, prev_hash, data.to_json());

            blockchain.add_block(block.clone());
            drop(blockchain);

            let add_block = full_rpc_proc(&self.appnode.kademlia.rpc, KademliaRequest::AddBlock(block), self.bootappnode.node.clone());
            if let Some(KademliaResponse::Ping) = add_block {
                println!("\t[AN{}]: Added Block info ({})", self.appnode.node.port, data.to_json());
            } else if let Some(KademliaResponse::PingUnableProcReq) = add_block {
                let mut blockchain = self.appnode.kademlia.blockchain.lock()
                    .expect("Error setting lock in local blockchain");
                blockchain.remove_last_block();
                drop(blockchain);
                println!("\t[AN{}]: Unable to add block info ({})", self.appnode.node.port, data.to_json());
                return false
            }
        }
        true
    }
}