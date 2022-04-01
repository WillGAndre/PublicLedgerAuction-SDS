use super::rpc::{Rpc, RpcRequestWithMeta, RpcMessage, RpcPayload, KademliaRequest, KademliaResponse, full_rpc_proc};
use super::node::{Node, Key, Distance, NodeWithDistance};
use super::{K_PARAM, N_KBUCKETS, KEY_LEN, ALPHA};

use crossbeam_channel;
use std::thread::{JoinHandle, spawn};
use std::sync::{Arc, Mutex};
use std::collections::{HashMap, BinaryHeap, HashSet};
use std::str;

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
     *       rt1.get_closest_nodes(node2.id)
     *        \
     *         From the bucket and bucket index 
     *         we get node2 and calculate the distance
     *         between node2.id and the original id
     *         we are searching for, which is node2.id.
     *         Thus the distance between both ids will be 0.
    */
    pub fn get_closest_nodes(&self, key: &Key) -> Vec<NodeWithDistance> {
        let mut res = Vec::new();
        let bucketindex = self.get_bucket_index(key);

        for node in &self.kbuckets[bucketindex].nodes {
            res.push(
                NodeWithDistance(node.clone(), Distance::new(&node.id, key))
            );
        }

        // if res.len() < K_PARAM {
        //     while bucketindex < self.kbuckets.len() - 1 {
        //         bucketindex += 1;

        //         for node in &self.kbuckets[bucketindex].nodes {
        //             res.push(
        //                 NodeWithDistance(node.clone(), Distance::new(&node.id, key))
        //             );
        //         }
        //     }
        // }

        res.sort_by(|a, b| a.1.cmp(&b.1));
        res.truncate(K_PARAM);

        res
    }
}

impl KademliaInstance {
    pub fn new(ip: String, port: u16, bootstrap: Option<Node>) -> Self {
        let node = Node::new(ip, port);
        let routingtable = RoutingTable::new(node.clone(), bootstrap);

        // RPC channels
        let (rpc_sender, rpc_receiver) = crossbeam_channel::unbounded();
        let rpc = Rpc::new(node.clone());
        Rpc::init(rpc.clone(), rpc_sender);

        let kad = Self {
            rpc: Arc::new(rpc),
            routingtable: Arc::new(Mutex::new(routingtable)),
            hashmap: Arc::new(Mutex::new(HashMap::new())),
            node: node.clone(),
        };

        kad.clone().requests_handler(rpc_receiver);
        kad.find_node(&node.id);

        // republish every <key,value> every timeout

        kad
    }

    pub fn ping(&self, node: Node) -> bool {
        let res = full_rpc_proc(&self.rpc, KademliaRequest::Ping, node.clone());

        let mut routingtable = self.routingtable.lock()
            .expect("Error setting lock in routing table");

        if let Some(KademliaResponse::Ping) = res {
            routingtable.update_routing_table(node);

            true
        } else {
            eprintln!("NO RESPONSE TO PING");
            // remove contact from routing table

            false
        }
    }

    /*
        TODO: docs
        qynode -> query node
        id -> id to search
    */
    pub fn query_node(&self, qynode: Node, id: Key) -> Option<Vec<NodeWithDistance>> {
        let res = full_rpc_proc(&self.rpc, KademliaRequest::FindNode(id), qynode.clone());

        let mut routingtable = self.routingtable.lock()
            .expect("Error setting lock in routing table");

        if let Some(KademliaResponse::FindNode(nodeswithdist)) = res {
            routingtable.update_routing_table(qynode);
            Some(nodeswithdist)
        } else {
            // Remove node from routing table
            None
        }

    } 

    /*
        Find node:
            Uses multiple threads for lookup,
            each node in our routing table closest to 'id'
            is visited and used to query for the node using the
            specified id. From there each thread holds the history
            of the search in a vector of NodeWithDistance. So each 
            node that was queried is added to the result. Result 
            vector is sorted by distance before being returned.
    */
    pub fn find_node(&self, id: &Key) -> Vec<NodeWithDistance> {
        let mut res: Vec<NodeWithDistance> = Vec::new();

        let routingtable = self.routingtable.lock()
            .expect("Error setting lock in routing table");

        let mut history = HashSet::new();
        let mut nodes = BinaryHeap::from(routingtable.get_closest_nodes(id));
        drop(routingtable);

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
            KademliaRequest::Store(_, _) => (KademliaResponse::Ping, request),
            KademliaRequest::FindNode(ref id) => {
                let routingtable = self.routingtable.lock()
                    .expect("Error setting lock in routing table");

                let result = routingtable.get_closest_nodes(id);

                (KademliaResponse::FindNode(result), request)
            },
            KademliaRequest::FindValue(_) => (KademliaResponse::Ping, request),
        }
    }
    
    /**
     * TESTING FUNCTIONS 
    **/

    pub fn print_routing_table(&self) {
        let routingtable = self.routingtable.lock()
            .expect("Error setting lock in routing table");
        println!("{:?}", routingtable);
    } 
}
