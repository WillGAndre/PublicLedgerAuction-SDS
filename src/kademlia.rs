use super::rpc::{
    Rpc, RpcRequestWithMeta, RpcMessage, RpcPayload, 
    KademliaRequest, KademliaResponse, 
    QueryValueResult, 
    full_rpc_proc
};
use super::node::{Node, Key, Distance, NodeWithDistance};
use super::{K_PARAM, N_KBUCKETS, KEY_LEN, ALPHA, TREPLICATE};
use super::blockchain::{Blockchain, Block};

use crossbeam_channel;
use std::thread::{JoinHandle, spawn, sleep};
use std::sync::{Arc, Mutex};
use std::collections::{HashMap, BinaryHeap, HashSet};
use std::str;
use std::time::Duration;
use log::{info};

/**
 * KBucket Instance:
 *  Stores nodes where the distance (d) of the curr.
 *  node and other nodes is: 2^i <= d < 2^(i+1), where i
 *  is the index of the KBucket (there are at most N_BUCKETS per node).
*/
#[derive(Debug, Clone)]
pub struct Bucket {
    pub nodes: Vec<Node>,
}

/**
 * Node Routing Table:
 *  Node routing table struct used to hold KBuckets (all)
 *  and communication channel for sending/receiving messages.
**/
#[derive(Debug)]
pub struct RoutingTable {
    pub node: Node,
    pub kbuckets: Vec<Bucket>,
}

#[derive(Debug, Clone)]
pub struct KademliaInstance {
    pub rpc: Arc<Rpc>,
    pub routingtable: Arc<Mutex<RoutingTable>>,
    pub hashmap: Arc<Mutex<HashMap<String, String>>>,
    pub node: Node,
    pub blockchain: Arc<Mutex<Blockchain>>,
}

impl Bucket {
    pub fn new() -> Self {
        Self { nodes: Vec::with_capacity(K_PARAM) }
    }
}

impl RoutingTable {
    pub fn new(
        node: Node,
        bootstrap: Option<Node>,
    ) -> Self {
        let mut kbuckets: Vec<Bucket> = Vec::new();
        for _ in 0..N_KBUCKETS {
            kbuckets.push(Bucket::new());
        }

        let mut res = Self {
            node: node.clone(),
            kbuckets: kbuckets,
        };

        // populate rout table with itself
        res.update_routing_table(node);
        if let Some(bootstrap) = bootstrap {
            res.update_routing_table(bootstrap);
        }

        res
    }

    /*
     * Based on KademliaBriefOverview.pdf,
     * Other Options:
     *  1)  distance.log2().floor() holds kbucket index
     *          since,
     *               2^i <= dist(self.node.id, key) < 2^(i+1)
     *          if dist == 0 { break }
     *  
     *  2)
     *      for i in 0..KEY_LEN {
     *          for cmnpfx in (0..8).rev() {
     *              if (d.0[i] >> (7 - cmnpfx)) & 0x1 != 0 {
     *                  return 8 * i + cmnpfx;
     *              }
     *          }
     *      }
     *      8 * KEY_LEN - 1
     * 
     * (N_KBUCKETS - 1) - (8 * i) , offset 8 in 8 buckets (there are N_KBUCKETS --> multipl of 8)
     * (    || + shift), offset other buckets (8 bytes thus 8 possible shifts)
    */
    fn get_bucket_index(&self, key: &Key) -> usize {
        let d = Distance::new(&self.node.id, key);

        for i in 0..KEY_LEN {
            for shift in 0..8 {
                if (d.0[i] << shift) & 0b10000000 != 0 {
                    return (N_KBUCKETS - 1) - ((8 * i) + shift);
                } 
            }
        }

        0
    }

    // Get closest nodes according to key
    fn get_closest_nodes(&self, key: &Key) -> Vec<NodeWithDistance> {
        let mut res = Vec::new();
        let mut bucketindex = self.get_bucket_index(key);

        while bucketindex < self.kbuckets.len() - 1 {
            bucketindex += 1;

            for node in &self.kbuckets[bucketindex].nodes {
                res.push(
                    NodeWithDistance(node.clone(), Distance::new(&node.id, key))
                );
            }
        }

        
        res
    }

    // Last resort for find_value
    fn get_all_nodes(&self, key: &Key) -> Vec<NodeWithDistance> {
        let mut res = Vec::new();

        for index in 1..N_KBUCKETS {
            for node in &self.kbuckets[index].nodes {
                if res.len() == K_PARAM {
                    break;
                }
                res.push(
                    NodeWithDistance(node.clone(), Distance::new(&node.id, key))
                );
            }
            if res.len() == K_PARAM {
                break;
            }
        }

        res.sort_by(|a, b| a.1.cmp(&b.1));
        res
    }

    // Routing table update function: Updates routing table with new node
    pub fn update_routing_table(&mut self, node: Node) {
        let bucketindex = self.get_bucket_index(&node.id);

        if self.kbuckets[bucketindex].nodes.len() < K_PARAM {
            let nodeindex = self.kbuckets[bucketindex].nodes.iter().position(|n| n.id == node.id);
            match nodeindex {
                Some(i) => {
                    self.kbuckets[bucketindex].nodes.remove(i);
                    self.kbuckets[bucketindex].nodes.push(node);
                },
                None => {
                    //println!("{} routing table update: new node ( {} )", self.node.get_addr(), &node.get_node());
                    self.kbuckets[bucketindex].nodes.push(node);
                },
            }
        }
    }

    /*
     * Returns distance of node(s) in the bucket
     * with the key supplied as argument.
     *  Assume that both kad instances hold node1
     *  and node2, Example:
     *   kad1.query_node(node1, node2.id)
     *      \
     *       rt1.get_bucket_nodes(node2.id)
     *        \
     *         From the bucket and bucket index 
     *         we get node2 and calculate the distance
     *         between node2.id and the original id
     *         we are searching for, which is node2.id.
     *         Thus the distance between both ids will be 0.
    */
    pub fn get_bucket_nodes(&self, key: &Key) -> Vec<NodeWithDistance> {
        let mut res = Vec::new();
        let bucketindex = self.get_bucket_index(key);

        for node in &self.kbuckets[bucketindex].nodes {
            res.push(
                NodeWithDistance(node.clone(), Distance::new(&node.id, key))
            );
        }

        res.sort_by(|a, b| a.1.cmp(&b.1));
        res.truncate(K_PARAM);

        res
    }

    // Check if node contains key in routing table
    pub fn contains_node(&self, key: &Key) -> bool {
        let bucket = self.get_bucket_nodes(key);

        if bucket.iter().position(|nwd| nwd.0.id == key.clone()) == None {
            return false
        }

        true
    }
}

impl KademliaInstance {
    pub fn new(ip: String, port: u16, bootstrap: Option<Node>) -> Self {
        let node = Node::new(ip, port);
        let routingtable = RoutingTable::new(node.clone(), bootstrap);
        let mut blockchain = Blockchain::new();
        blockchain.genesis();

        // RPC channels
        let (rpc_sender, rpc_receiver) = crossbeam_channel::unbounded();
        let rpc = Rpc::new(node.clone());
        Rpc::init(rpc.clone(), rpc_sender);

        let kad = Self {
            rpc: Arc::new(rpc),
            routingtable: Arc::new(Mutex::new(routingtable)),
            hashmap: Arc::new(Mutex::new(HashMap::new())),
            node: node.clone(),
            blockchain: Arc::new(Mutex::new(blockchain))
        };

        kad.clone().requests_handler(rpc_receiver);
        
        // Populate routing table with our instance
        kad.find_node(&node.id);

        // republish every <key,value> every timeout
        let kadclone = kad.clone();
        spawn(move || {
            sleep(Duration::from_secs(TREPLICATE));
            kadclone.republish();
        });

        kad
    }

    pub fn republish(&self) {
        let hashmap = self.hashmap.lock()
            .expect("Error setting lock in hashmap");
        for (key, value) in &*hashmap {
            self.insert(key.to_string(), value.to_string());
        }
        drop(hashmap)
    }

    /**
     * HASHMAP FUNCTIONS 
    **/

    /**
     * Both functions have a nuance
     * where if no node or value is
     * found, then local hashmap is
     * used to perform the action.
    **/

    /**
     * Function based on KAD paper
     * ACTUAL FUNCTION USED TO INSERT VALUE
     * NOT TO BE CONFUSED WITH STORE 
    **/
    pub fn insert(&self, keystr: String, value: String) {
        let nodes = self.find_node(&Key::new(keystr.clone()));

        if nodes.is_empty() {
            let mut hashmap = self.hashmap.lock()
                .expect("");
            hashmap.insert(keystr, value);
            drop(hashmap)

            // println!("\t[AN{}]: Added to self DHT", self.node.port)
        } else {
            // let mut nodes_list: Vec<(Node, String)> = Vec::new();
            // for NodeWithDistance(node, _) in nodes.clone() {
            //     let kad = self.clone();
            //     let keystr = keystr.clone();
            //     let get_value = kad.query_value(node.clone(), keystr);

            //     if let Some(query_value) = get_value {
            //         if let QueryValueResult::Value(value_str) = query_value {
            //             nodes_list.push((node, value_str))
            //         }
            //     }
            // }

            // if !nodes_list.is_empty() {
            //     nodes_list.sort_by(|nv1, nv2| nv1.1.len().cmp(&nv2.1.len()));

            //     for (node, _) in nodes_list {
            //         let kad = self.clone();
            //         let keystr = keystr.clone();
            //         let value = value.clone();
            //         kad.store_value(node, keystr, value);
            //     }
            // } else {
            //     for NodeWithDistance(node, _) in nodes {
            //         let kad = self.clone();
            //         let keystr = keystr.clone();
            //         let value = value.clone();
            //         kad.store_value(node, keystr, value);
            //     }
            // }

            for NodeWithDistance(node, _) in nodes {
                let kad = self.clone();
                let keystr = keystr.clone();
                let value = value.clone();
                kad.store_value(node, keystr, value);
            }

            // println!("\t[AN{}]: Added to other DHT", self.node.port)
        }
    }

    pub fn get(&self, key: String) -> Option<String> {
        let (value, mut nodes) = self.find_value(key.clone());

        if value == None {
            let hashmap = self.hashmap.lock()
                .expect("Error setting lock in hashmap");
            let hashmap_clone = hashmap.clone();
            let value = hashmap_clone.get(&key);
            drop(hashmap);
            if value != None {
                return Some(value.unwrap().to_string())
            }
            None
        } else {
            value.map(|val| {
                if let Some(NodeWithDistance(node, _)) = nodes.pop() {
                    self.store_value(node, key, val.clone());
                }
                val
            })
        }
    }

    /**
     *  FIND NODE/VALUE FUNCTIONS
    **/

    /*
        Find node:
            Uses multiple threads for lookup,
            each node in our routing table closest to 'id'
            is visited and used to query for the node using the
            specified id.

            If no nodes are present in the bucket, then either
            the node isn't present in the routing table or no 
            other node is stored in that bucket (ex: find_node_test).
            If this occurs then, the closest nodes to the id 
            are queried.
    */
    pub fn find_node(&self, id: &Key) -> Vec<NodeWithDistance> {
        let mut res: Vec<NodeWithDistance> = Vec::new();

        let routingtable = self.routingtable.lock()
            .expect("Error setting lock in routing table");

        let mut history = HashSet::new();
        
        let mut nodes = self.build_heap(id, routingtable); // # nodes >= ALPHA

        for entry in &nodes {
            history.insert(entry.clone());
        }

        while !nodes.is_empty() {
            let mut threads: Vec<JoinHandle<Option<Vec<NodeWithDistance>>>> = Vec::new();
            let mut qynodes: Vec<NodeWithDistance> = Vec::new();
            let mut results: Vec<Option<Vec<NodeWithDistance>>> = Vec::new();

            // ALPHA parallelism
            for _ in 0..ALPHA {
                match nodes.pop() {
                    Some(node) => { qynodes.push(node); },
                    None => { break; },
                }
            }

            // ref --> reference
            for NodeWithDistance(ref node, _) in &qynodes {
                let kad = self.clone();
                let node = node.clone();
                let id = id.clone();
                threads.push(spawn(move || {
                    kad.query_node(node, id)
                }));
            }

            for thread in threads {
                results.push(thread.join()
                    .expect("Error joining threads with results")
                );
            }

            for (result, qynode) in results.into_iter().zip(qynodes) {
                if let Some(entries) = result {
                    // add intermediate query node to result
                    res.push(qynode);

                    // if result node(s) (closest nodes to qynode)
                    // haven't been searched add them to heap
                    for entry in entries {
                        if history.insert(entry.clone()) {
                            nodes.push(entry);
                        }
                    }
                }
            }
        }
        res.truncate(K_PARAM);

        res
    }

    // Same as function above but for given key (string)
    pub fn find_value(&self, keystr: String) -> (Option<String>, Vec<NodeWithDistance>) {
        let mut res: Vec<NodeWithDistance> = Vec::new();
        let key: Key = Key::new(keystr.clone());
        let routingtable = self.routingtable.lock()
            .expect("Error setting lock in routing table");
        let mut history = HashSet::new();

        let mut nodes = self.build_heap(&key, routingtable); // # nodes >= ALPHA

        for entry in &nodes {
            history.insert(entry.clone());
        }

        while !nodes.is_empty() {
            let mut threads: Vec<JoinHandle<Option<QueryValueResult>>> = Vec::new();
            let mut qynodes: Vec<NodeWithDistance> = Vec::new();
            let mut results: Vec<Option<QueryValueResult>> = Vec::new();

            // ALPHA parallelism
            for _ in 0..ALPHA {
                match nodes.pop() {
                    Some(node) => { qynodes.push(node); },
                    None => { break; },
                }
            }

            // ref --> reference
            for NodeWithDistance(ref node, _) in &qynodes {
                let kad = self.clone();
                let node = node.clone();
                let keystr = keystr.clone();
                threads.push(spawn(move || {
                    kad.query_value(node, keystr)
                }));
            }
            
            for thread in threads {
                results.push(thread.join()
                    .expect("Error joining threads with results")
                );
            }

            let mut value_res = String::from("");
            for (result, qynode) in results.into_iter().zip(qynodes) {
                if let Some(value) = result {
                    match value {
                        QueryValueResult::Nodes(entries) => {
                            // add intermediate query node to result
                            // since search didn't find the value
                            res.push(qynode);

                            for entry in entries {
                                if history.insert(entry.clone()) {
                                    nodes.push(entry);
                                }
                            }
                        },
                        QueryValueResult::Value(value) => {
                            if value.len() > value_res.len() {
                                value_res = value
                            }
                            // ---
                            // res.sort_by(|a,b| a.1.cmp(&b.1));
                            // res.truncate(K_PARAM);

                            // return (Some(value), res)
                            // ---
                        }
                    }
                }
            }
            // ---
            if value_res != "" {
                res.sort_by(|a,b| a.1.cmp(&b.1));
                res.truncate(K_PARAM);

                return (Some(value_res), res)
            }
            // ---
        }

        res.truncate(K_PARAM);
        (None, res)
    }


    // NOTICE/TODO: ATM HEAP MAY INCLUDE MULTIPLE COPIES OF NODES
    // FUNCTION SHOULD BE TWEAKED IN CASE NOT ENOUGH NODES AREN'T
    // BEING RETURNED FROM find_node/find_value (inconsistencies 
    // in pubsub hashmap insert).
    fn build_heap(&self, key: &Key, routingtable: std::sync::MutexGuard<RoutingTable>) -> BinaryHeap<NodeWithDistance> {
        let mut nodes = BinaryHeap::from(routingtable.get_bucket_nodes(key));

        let mut cycle = 0;
        while nodes.len() < ALPHA {
            let mut candidate_nodes: Vec<NodeWithDistance> = Vec::new();
            if cycle == 0 {
                candidate_nodes = routingtable.get_closest_nodes(key);
                cycle = 1;
            } else if cycle == 1 {
                candidate_nodes = routingtable.get_all_nodes(key);
                cycle = 2;
            } else {
                nodes.extend(candidate_nodes); // TODO: REMOVE
                break
            }

            let candidate_nodes_len = candidate_nodes.len();
            let range = ALPHA - nodes.len();
            let res: Vec<NodeWithDistance>;

            if range <= candidate_nodes_len {
                res = candidate_nodes.drain(0..range).collect();
            } else {
                res = candidate_nodes.drain(0..candidate_nodes_len).collect();
            }

            nodes.extend(res)
        }

        drop(routingtable);
        nodes
    }

    /**
     * RPC CALLS
    **/

    // Send ping to node
    pub fn ping(&self, node: Node) -> bool {
        let res = full_rpc_proc(&self.rpc, KademliaRequest::Ping, node.clone());

        if let Some(KademliaResponse::Ping) = res {
            let mut routingtable = self.routingtable.lock()
                .expect("Error setting lock in routing table");
            routingtable.update_routing_table(node);
            drop(routingtable);

            true
        } else {
            eprintln!("NO RESPONSE TO PING");
            // remove contact from routing table

            false
        }
    }

    // Query node for given id (routing table)
    pub fn query_node(&self, qynode: Node, id: Key) -> Option<Vec<NodeWithDistance>> {
        let res = full_rpc_proc(&self.rpc, KademliaRequest::QueryNode(id), qynode.clone());

        if let Some(KademliaResponse::QueryNode(nodeswithdist)) = res {
            let mut routingtable = self.routingtable.lock()
                .expect("Error setting lock in routing table");
            routingtable.update_routing_table(qynode);
            drop(routingtable);

            Some(nodeswithdist)
        } else {
            // Remove node from routing table
            None
        }

    } 

    // Query node for given key (hashmap)
    pub fn query_value(&self, qynode: Node, key: String) -> Option<QueryValueResult> {
        let res = full_rpc_proc(&self.rpc, KademliaRequest::QueryValue(key), qynode.clone());
        
        if let Some(KademliaResponse::QueryValue(value)) = res {
            let mut routingtable = self.routingtable.lock()
                .expect("Error setting lock in routing table");
            routingtable.update_routing_table(qynode);
            drop(routingtable);
            Some(value)
        } else {
            None
        }
    }

    // Store <key,value> in node
    pub fn store_value(&self, qynode: Node, key: String, value: String) {
        let res = full_rpc_proc(&self.rpc, KademliaRequest::Store(key.clone(), value.clone()), qynode.clone());

        if let Some(KademliaResponse::Ping) = res {
            let mut routingtable = self.routingtable.lock()
                .expect("Error setting lock in routingtable");
            routingtable.update_routing_table(qynode);
            drop(routingtable);
        } else {
            // TODO: error logs
            println!("ERROR")
        }
    }

    // Query node for local blockchain
    pub fn query_blockchain(&self, qynode: Node) -> Option<Vec<Block>> {
        let res = full_rpc_proc(&self.rpc, KademliaRequest::QueryLocalBlockChain, qynode.clone());

        if let Some(KademliaResponse::QueryLocalBlockChain(blockchain)) = res {
            Some(blockchain)
        } else {
            None
        }
    }

    /** 
     * Requests handler & Response constructor
    */

    fn requests_handler(self, receiver: crossbeam_channel::Receiver<RpcRequestWithMeta>) {
        std::thread::spawn(move || {
            for request in receiver.iter() {
                let kadinstance = self.clone();

                std::thread::spawn(move || {
                    let response = kadinstance.make_response(request);
                    let payload = RpcPayload::Response(response.0);
                    kadinstance.rpc.send_msg(
                        &RpcMessage {
                            id: response.1.id,
                            src: kadinstance.node.get_addr(),
                            dst: response.1.src,
                            payload: payload,
                        }
                    );
                });
            }
        });
    }

    fn make_response(&self, request: RpcRequestWithMeta) -> (KademliaResponse, RpcRequestWithMeta) {
        let mut routingtable = self.routingtable.lock()
            .expect("Error setting lock in routing table");

        let addr: Vec<&str> = request.src.split(":").collect();
        routingtable.update_routing_table(
            Node::new(
                addr[0].to_string(), 
                addr[1].parse::<u16>()
                    .expect("Error parsing port to u16"),
            )
        );
        drop(routingtable);

        match request.payload {
            KademliaRequest::Ping => (KademliaResponse::Ping, request),
            KademliaRequest::Store(ref key, ref value) => {
                let mut hashmap = self.hashmap.lock()
                    .expect("");
                hashmap.insert(key.to_string(), value.to_string());
                drop(hashmap);
                (KademliaResponse::Ping, request)
            },
            KademliaRequest::QueryNode(ref id) => {
                let routingtable = self.routingtable.lock()
                    .expect("Error setting lock in routing table");
                let result = routingtable.get_bucket_nodes(id);
                drop(routingtable);

                (KademliaResponse::QueryNode(result), request)
            },
            KademliaRequest::QueryValue(ref keystr) => {
                let key = Key::new(keystr.to_string());
                let hashmap = self.hashmap.lock()
                    .expect("Error setting lock in hashmap");
                let hashmap_clone = hashmap.clone();
                let value = hashmap_clone.get(keystr);
                drop(hashmap);

                match value {
                    Some(val) => (
                        (KademliaResponse::QueryValue(QueryValueResult::Value(val.to_string())), request)
                    ),
                    None => {
                        let routingtable = self.routingtable.lock()
                            .expect("Error setting lock in routing table");
                        let bucket_nodes = routingtable.get_bucket_nodes(&key);
                        drop(routingtable);
                        (KademliaResponse::QueryValue(QueryValueResult::Nodes(bucket_nodes)), request)
                    }
                }
            },

            KademliaRequest::AddBlock(ref block) => {
                let mut blockchain = self.blockchain.lock()
                    .expect("Error setting lock in local blockchain");
                let res = blockchain.add_block(block.clone());
                drop(blockchain);
                if res {
                    return (KademliaResponse::Ping, request)
                }
                (KademliaResponse::PingUnableProcReq, request)
            },
            KademliaRequest::QueryLocalBlockChain => {
                let blockchain = self.blockchain.lock()
                    .expect("Error setting lock in local blockchain");
                let blocks = blockchain.blocks.clone();
                drop(blockchain);
                (KademliaResponse::QueryLocalBlockChain(blocks), request)
            },

            KademliaRequest::NodeJoin(ref node) => {
                let nodes: Vec<Node> = self.find_node(&node.id).iter().map(|nwd| nwd.0.clone()).collect();
                let mut routingtable = self.routingtable.lock()
                    .expect("Error setting lock in routing table");
                routingtable.update_routing_table(node.clone());
                drop(routingtable);
                (KademliaResponse::NodeJoin(nodes), request)
            },
        }
    }
    
    /**
     * TESTING FUNCTIONS 
    **/

    pub fn same_bucket(&self, id1: &Key, id2: &Key) -> bool {
        let routingtable = self.routingtable.lock()
            .expect("Error setting lock in routing table");
        
        if routingtable.get_bucket_index(id1) == routingtable.get_bucket_index(id2) {
            drop(routingtable);
            return true
        }

        drop(routingtable);
        false
    }

    pub fn print_routing_table(&self) {
        let routingtable = self.routingtable.lock()
            .expect("Error setting lock in routing table");
        println!("{:?}", routingtable);
        drop(routingtable);
    }

    pub fn print_hashmap(&self) -> String {
        let hashmap = self.hashmap.lock()
            .expect("Error setting lock in hasmap");
        let hashmap_clone = hashmap.clone();
        drop(hashmap);
        format!("{:?}", hashmap_clone)
    }

    pub fn print_blockchain(&self) {
        let blockchain = self.blockchain.lock()
            .expect("Error setting lock in local blockchain");
        let blocks = blockchain.blocks.clone();
        drop(blockchain);
        println!("{:?}", blocks)
    }

    /**
     * DEBUG
    **/

    pub fn log_blockchain(&self) {
        let blockchain = self.blockchain.lock()
            .expect("Error setting lock in local blockchain");
        let blocks: Vec<Block> = blockchain.blocks.clone();
        drop(blockchain);
        info!("\nBN[{}] New BK block: {:?}", self.node.port, blocks.last())
    } 
}
