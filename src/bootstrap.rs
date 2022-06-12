use super::kademlia::{KademliaInstance};
use super::blockchain::Block;
use super::pubsub::PubSubInstance;
use super::node::{Node};
use super::aux::{get_ip, LockResultRes};
use super::rpc::{full_rpc_proc, KademliaRequest, KademliaResponse, QueryValueResult};
use super::NODETIMEOUT;

use std::sync::{Arc, Mutex};
use std::thread::{spawn, sleep};
use std::time::Duration;
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use base64::decode;
use log::{info, warn};

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

        info!("Synchronizing Bootstrap...");
        boot = boot.init_sync();
        info!("Bootstrap ready");

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
                            warn!("\t[BOOT]: FULL SYNC - Error synchronizing blockchain");
                            break; // sync next timeout
                        }
                    }
                    boot.bk_hash = global_hash.unwrap();
                    // Debug
                    boot.nodes[0].kademlia.log_blockchain();
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
            pubsub: PubSubInstance::new(None, node.get_addr(), None, None)
        }
    }

    pub fn publish(&self, topic: String, ttl: DateTime<Local>) {
        let mut pubsub = self.pubsub.clone();
        pubsub.set_ttl(ttl);
        self.kademlia.insert(topic.clone(), pubsub.to_string());
        //println!("\t[AN{}]: Published topic (in DHT): {}; Exp: {}", self.node.port, topic, ttl)
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
            //println!("\t[AN{}]: subscribed to topic: {}", self.node.port, topic);
            return true
        }
        
        false
    }

    // TODO:
    //  - verify node addr
    //  - verify msg (tuple -> (number to raise bid; sender addr))
    pub fn add_msg(&self, topic: String, msg: String) -> bool {
        let pubsub = self.kademlia.get(topic.clone());
        if pubsub == None {
            println!("\t[AN{}]: Error adding msg - couldn't find topic: {}", self.node.port, topic)
        } else {
            let pubsub_str = pubsub.unwrap();
            let pubsub_ins = self.get_pubsub_instance(pubsub_str).unwrap();
            if pubsub_ins.verify_addr(self.node.get_addr()) {
                let status = pubsub_ins.add_msg(msg);
                if status == 0 {
                    self.kademlia.insert(topic.clone(), pubsub_ins.to_string());
                    //println!("\t[AN{}]: added msg to topic: {}", self.node.port, topic);
                    return true
                }
            }
        }
        
        false
    }

    // register method - arg: AppNode, Note: Added node timeout
    pub fn join_network(&self, bootnode: Node) -> bool {
        let find_node = full_rpc_proc(&self.kademlia.rpc, KademliaRequest::NodeJoin(self.node.clone()), bootnode.clone());
        
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
                
                let query_blockchain = full_rpc_proc(&self.kademlia.rpc, KademliaRequest::QueryLocalBlockChain, bootnode.clone());
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

                    let add_block = full_rpc_proc(&self.kademlia.rpc, KademliaRequest::AddBlock(block), bootnode.clone());
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
            let id: Option<String> = Some(String::from(data_vec[0]));
            let publisher: String = String::from(data_vec[1]);
            let substack: Vec<String> = data_vec[2].trim_matches(pattern).split(' ').map(|s| String::from(s)).collect();
            let msgstack: Vec<String> = data_vec[3].trim_matches(pattern).split(' ').map(|s| String::from(s)).collect();
            let mut pubsub = PubSubInstance::new(id, publisher, Some(msgstack), Some(substack));
            let ttl_str: String = data_vec[4].to_string();
            if ttl_str != "NONE" {
                let ttl: DateTime<Local> = ttl_str.parse().unwrap(); // TODO: TEST
                pubsub.set_ttl(ttl);
            }
            return Some(pubsub);
        }
        None
    }

    pub fn get_pubsub_json(&self, topic: String) -> Value {
        let pubsub = self.kademlia.get(topic.clone());
        if pubsub == None { 
            println!("\t[AN{}]: Error getting PubSub - couldn't find topic: {}", self.node.port, topic)
        } else {
            let pubsub_str = pubsub.unwrap();
            let pubsub_ins: PubSubInstance = self.get_pubsub_instance(pubsub_str).unwrap();
            let json: Value = pubsub_ins.as_json();
            
            return json!(
                {
                    "name": topic,
                    "id": json["id"],
                    "num_subs": json["num_subs"],
                    "highest_bid": json["highest_bid"],
                    "highest_bidder": json["highest_bidder"],
                    "ttl": json["ttl"],
                    "subscribed": pubsub_ins.verify_addr(self.node.get_addr())
                }
            )
        }
        json!({})
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
    pub bootappnode: Node,
    pub topics: Arc<Mutex<Vec<(String, String, String)>>>,
}

/*
    TODO:
        - set sub/add_msg timeout on fail
*/
// NOTE: Pubsub GET (DHT) before any action
/*
    - Refresh --> Get PubSub
    - Send_json --> Package PubSub as json {id: <>, name: <topic-name>, num_subs: <>, highest_bid: <>, highest_bidder: <>, TTL: <>, subscribed: <bool>}
*/
impl App {
    pub fn new(addr: String, port: u16, bootappnode: Node) -> Self  {
        //let bootnode = bootappnode.node.clone();
        let node = bootappnode.clone();
        let appnode = AppNode::new(addr, port, Some(node));

        let app = Self {
            appnode: appnode,
            bootappnode: bootappnode.clone(),
            topics: Arc::new(Mutex::new(Vec::new())),
        };

        let mut register = app.join_network(bootappnode.clone());
        let mut iter = 2;
        while !register {
            if iter == 5 { break; }
            register = app.join_network(bootappnode.clone());
            iter += 1;
        }

        App::pull_bk(app.clone());
        App::teardow_pubsub(app.clone());

        app
    }

    // register method - arg: AppNode, Note: Added node timeout
    /*
        Based on the AppNode join_network function but with small
        nuance. After joining the network previous work saved in BK
        is analyzed in order to propogate information back to the CLI.
        Notice that this procedure is based on the slides provided by 
        the teacher in the practical class (KademliaBriefOverview.pdf),
        blockchain is queried then new block is added.
    */
    fn join_network(&self, bootnode: Node) -> bool {
        let find_node = full_rpc_proc(&self.appnode.kademlia.rpc, KademliaRequest::NodeJoin(self.appnode.node.clone()), bootnode.clone());
        
        if let Some(KademliaResponse::NodeJoin(nodes)) = find_node {
            if !nodes.is_empty() {
                for node in nodes {
                    if node.id != self.appnode.node.id {
                        full_rpc_proc(&self.appnode.kademlia.rpc, KademliaRequest::NodeJoin(self.appnode.node.clone()), node.clone());
                        let mut routingtable = self.appnode.kademlia.routingtable.lock().get_guard();
                        routingtable.update_routing_table(node);
                        drop(routingtable)
                    }
                }
                
                let query_blockchain = full_rpc_proc(&self.appnode.kademlia.rpc, KademliaRequest::QueryLocalBlockChain, bootnode.clone());
                if let Some(KademliaResponse::QueryLocalBlockChain(blocks)) = query_blockchain {
                    let mut blockchain = self.appnode.kademlia.blockchain.lock().get_guard();
                    let new_blocks = blockchain.get_diff_from_chains(blockchain.blocks.clone(), blocks.clone());
                    blockchain.blocks = blockchain.choose_chain(blockchain.blocks.clone(), blocks);

                    let id = blockchain.blocks[blockchain.blocks.len() - 1].id + 1;
                    let prev_hash = blockchain.blocks[blockchain.blocks.len() - 1].hash.to_string();
                    let data = Data::new(
                        format!("REGISTER: {id}", id=self.appnode.node.get_addr()), 
                        0,
                        None
                    );
                    let block = Block::new(id, prev_hash, data.to_json());

                    blockchain.add_block(block.clone());
                    drop(blockchain);

                    let add_block = full_rpc_proc(&self.appnode.kademlia.rpc, KademliaRequest::AddBlock(block), bootnode.clone());
                    if let Some(KademliaResponse::Ping) = add_block {
                        println!("\t[AN{}]: Added Block info ({})", self.appnode.node.port, data.to_json());

                        for new_block in new_blocks.clone() {
                            if new_block.id != 0 {
                                let data: Data = serde_json::from_str(&new_block.data).
                                    expect("Error converting data to json");
                                match data.msg_type {
                                    0 => {},
                                    1 => {
                                        let ttl: DateTime<Local> = data.exp_time.unwrap().parse().unwrap();
                                        let ttl_str = format!("{}", ttl);
                                        let time = Local::now();
                                        let diff = (ttl - time).num_minutes();
                                        if diff > 0 {
                                            let mut topics = self.topics.lock().get_guard();
                                            let msg_split: Vec<&str> = data.msg.split('|').collect();
                                            let topic_split: Vec<&str> = msg_split[0].split(' ').collect();
                                            let publisher_split: Vec<&str> = msg_split[1].split(' ').collect();
                                            let topic_entry = (topic_split[1].to_string(), ttl_str, publisher_split[1].to_string());
                                            if !topics.contains(&topic_entry) {
                                                topics.push(topic_entry);
                                            }
                                            drop(topics);
                                        }
                                        // App::teardow_pubsub(app.clone(), 1 * 60);
                                    },
                                    2 => {
                                        let mut topics = self.topics.lock().get_guard();
                                        let data_split: Vec<&str> = data.msg.split('|').collect();
                                        let topic: Vec<&str> = data_split[0].split(' ').collect();
                                        let index = topics.iter().position(|(x, _, _)| *x == topic[1]);
                                        if index != None {
                                            topics.remove(index.unwrap());
                                        }
                                        drop(topics);
                                        // bid -> data_split[1] | bidder -> data_split[2]
                                    },
                                    _ => {},
                                };
                            }
                        }

                        sleep(Duration::from_secs(NODETIMEOUT));
                        return true
                    } else if let Some(KademliaResponse::PingUnableProcReq) = add_block {
                        let mut blockchain = self.appnode.kademlia.blockchain.lock().get_guard();
                        blockchain.remove_last_block();
                        drop(blockchain);
                        println!("\t[AN{}]: Unable to add block info ({})", self.appnode.node.port, data.to_json());
                        return false
                    }
                }
            } else {
                println!("\t[AN{}]: Error joining network - No nearby nodes found", self.appnode.node.port)
            }
        } else {
            // TODO
            println!("\t[AN{}]: Error joining network", self.appnode.node.port)
        }
        false   
    }

    fn pull_bk(app: App) {
        spawn(move || {
            loop {
                sleep(Duration::from_secs(NODETIMEOUT * 2));
                let query_blockchain = full_rpc_proc(&app.appnode.kademlia.rpc, KademliaRequest::QueryLocalBlockChain, app.bootappnode.clone());
                if let Some(KademliaResponse::QueryLocalBlockChain(blocks)) = query_blockchain {
                    let mut blockchain = match app.appnode.kademlia.blockchain.lock() {
                        Ok(blockchain) => blockchain,
                        Err(_) => continue
                    };
                    let new_blocks = blockchain.get_diff_from_chains(blockchain.blocks.clone(), blocks.clone());
                    blockchain.blocks = blockchain.choose_chain(blockchain.blocks.clone(), blocks);
                    drop(blockchain);

                    for block in new_blocks.clone() {
                        let data: Data = serde_json::from_str(&block.data).
                            expect("Error converting data to json");
                        match data.msg_type {
                            0 => {},
                            1 => {
                                let ttl: DateTime<Local> = data.exp_time.unwrap().parse().unwrap();
                                let ttl_str = format!("{}", ttl);
                                let time = Local::now();
                                let diff = (ttl - time).num_seconds();
                                if diff > 0 {
                                    let mut topics = match app.topics.lock() {
                                        Ok(topics) => topics,
                                        Err(_) => continue
                                    };
                                    let msg_split: Vec<&str> = data.msg.split('|').collect();
                                    let topic_split: Vec<&str> = msg_split[0].split(' ').collect();
                                    let publisher_split: Vec<&str> = msg_split[1].split(' ').collect();
                                    let topic_entry = (topic_split[1].to_string(), ttl_str, publisher_split[1].to_string());
                                    if !topics.contains(&topic_entry) {
                                        topics.push(topic_entry);
                                    }
                                    drop(topics)
                                }
                            },
                            2 => {
                                let mut topics = match app.topics.lock() {
                                    Ok(topics) => topics,
                                    Err(_) => continue
                                };
                                let data_split: Vec<&str> = data.msg.split('|').collect();
                                let topic: Vec<&str> = data_split[0].split(' ').collect();
                                let index = topics.iter().position(|(x, _, _)| *x == topic[1]);
                                if index != None {
                                    topics.remove(index.unwrap());
                                }
                                drop(topics)
                                // bid -> data_split[1] | bidder -> data_split[2]
                            },
                            _ => {},
                        };
                    }
                }

            }
        });
    }

    fn teardow_pubsub(app: App) {
        spawn(move || {
            loop {
                let topics = match app.topics.lock() {
                    Ok(topics) => topics,
                    Err(_) => continue
                };
                let topics_state = topics.clone();
                drop(topics);
                if topics_state.len() == 0 {
                    sleep(Duration::from_secs(NODETIMEOUT * 50));
                } else {
                    let mut topic_to_delete: String = String::from("");
                    for (topic, ttl_str, publisher_addr) in topics_state.clone() {
                        let ttl: DateTime<Local> = ttl_str.parse().unwrap();
                        let diff: i64 = (ttl - Local::now()).num_seconds();
                        if diff <= 0 && publisher_addr == app.appnode.node.get_addr() {
                            topic_to_delete = topic.clone();
                            let json = app.get_json(topic.clone());
                            app.pull_bk_add_block(
                                Data::new(
                                    format!("END_TOPIC: {}|BID: {}|BIDDER: {}", topic, json["highest_bid"], json["highest_bidder"]), 
                                    2, 
                                    None
                                )
                            );
                            break
                        }
                        if topic_to_delete != "" {
                            break
                        }
                    }
                    if topic_to_delete != "" {
                        let mut topics = match app.topics.lock() {
                            Ok(topics) => topics,
                            Err(_) => continue
                        };
                        let index = topics.iter().position(|(x, _, _)| *x == topic_to_delete).unwrap();
                        topics.remove(index);
                        drop(topics)
                    }
                }
            }
        });
    }

    pub fn publish(&self, topic: String) -> bool {
        let timeout_mins: i64 = 2; // 15
        let ttl = Local::now() + chrono::Duration::minutes(timeout_mins);
        let ttl_str = format!("{}", ttl);
        let data = Data::new(
            format!("PUB_TOPIC: {}|PUBLISHER: {}", topic, self.appnode.node.get_addr()), 
            1,
            Some(format!("{}", ttl)),
        );
        if self.pull_bk_add_block(data.clone()) {
            // TODO: error handeling
            self.appnode.publish(topic.clone(), ttl);

            let topic_entry = (topic, ttl_str, self.appnode.node.get_addr());
            let mut topics = self.topics.lock().expect("Error settig lock in topics");
            if !topics.contains(&topic_entry) {
                topics.push(topic_entry);
            }
            drop(topics);
            sleep(Duration::from_secs(NODETIMEOUT));
            return true
        }
        false
    }

    // TODO: - Retry mech
    // Maybe add timeout before return (?)
    pub fn subscribe(&self, topic: String) -> bool {
        let topics = self.topics.lock().get_guard();
        let topics_vec: Vec<(String,String,String)> = topics.clone().into_iter().collect();
        drop(topics);

        let mut sub = false;
        for topic_state in topics_vec {
            if topic == topic_state.0 {
                let ttl: DateTime<Local> = topic_state.1.parse().unwrap();
                if (ttl - Local::now()).num_seconds() > 0 {
                    sub = self.appnode.subscribe(topic);
                    sleep(Duration::from_secs(NODETIMEOUT));
                    break
                }
            }
        }
        
        sub
    }

    // TODO: - Retry mech
    pub fn add_msg(&self, topic: String, msg: String) -> bool {
        // bid X (only)
        let msg_split: Vec<&str> = msg.split(' ').collect();
        let raise: usize = msg_split[1].parse::<usize>().unwrap();
        let res_msg = json!({"data": raise, "sender_addr": self.appnode.node.get_addr()});
        let status = self.appnode.add_msg(topic, res_msg.to_string());

        sleep(Duration::from_secs(NODETIMEOUT));
        status
    }

    pub fn get_topics(&self) -> Vec<(String, String, String)> {
        let topics = self.topics.lock().get_guard();
        let res = topics.clone();
        drop(topics);
        
        res
    }

    pub fn get_json(&self, topic: String) -> Value {
        self.appnode.get_pubsub_json(topic).clone()
    }

    fn pull_bk_add_block(&self, data: Data) -> bool {
        let query_blockchain = full_rpc_proc(&self.appnode.kademlia.rpc, KademliaRequest::QueryLocalBlockChain, self.bootappnode.clone());
        if let Some(KademliaResponse::QueryLocalBlockChain(blocks)) = query_blockchain {
            let mut blockchain = self.appnode.kademlia.blockchain.lock()
                .expect("Error setting lock in local blockchain");
            blockchain.blocks = blockchain.choose_chain(blockchain.blocks.clone(), blocks);

            let id = blockchain.blocks[blockchain.blocks.len() - 1].id + 1;
            let prev_hash = blockchain.blocks[blockchain.blocks.len() - 1].hash.to_string();
            let block = Block::new(id, prev_hash, data.to_json());

            blockchain.add_block(block.clone());
            drop(blockchain);

            let add_block = full_rpc_proc(&self.appnode.kademlia.rpc, KademliaRequest::AddBlock(block), self.bootappnode.clone());
            if let Some(KademliaResponse::Ping) = add_block {
                // println!("\t[AN{}]: Added Block info ({})", self.appnode.node.port, data.to_json());
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