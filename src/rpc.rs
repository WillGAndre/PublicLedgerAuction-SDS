use serde::{Serialize, Deserialize};
use serde_json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crossbeam_channel;
use std::str;
use std::net::UdpSocket;
use std::thread;
use std::time::{SystemTime, Duration};

use super::node::{Key, NodeWithDistance, Node};
use super::blockchain::{Block};
use super::TREPLICATE;

// ENUM -> define types
// STRUCTS -> define obj

/**
 *  KADEMLIA
**/

#[derive(Serialize, Deserialize, Debug)]
pub enum KademliaRequest {
    Ping,
    Store(String, String),
    QueryNode(Key),
    QueryValue(String),

    // BLOCKCHAIN REQUESTS ----
    QueryLocalBlockChain,
    AddBlock(Block),
    // ----

    NodeJoin(Node)
}

#[derive(Serialize, Deserialize, Debug)]
pub enum KademliaResponse {
    Ping,
    QueryNode(Vec<NodeWithDistance>),
    QueryValue(QueryValueResult),

    // BLOCKCHAIN RESPONSES ----
    QueryLocalBlockChain(Vec<Block>),
    // ----

    NodeJoin(Vec<Node>)
}

#[derive(Serialize, Deserialize, Debug)]
pub enum QueryValueResult {
    Nodes(Vec<NodeWithDistance>),
    Value(String),
}

/**
 *  RPC
**/

#[derive(Serialize, Deserialize, Debug)]
pub enum RpcPayload {
    End,
    Request(KademliaRequest),
    Response(KademliaResponse),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RpcMessage {
    pub id: Key,
    pub src: String,
    pub dst: String,
    pub payload: RpcPayload,
}

#[derive(Debug)]
pub struct RpcRequest {
    pub id: Key,
    pub src: String,
    pub payload: KademliaRequest,
}

#[derive(Debug)]
pub struct RpcRequestWithMeta {
    pub id: Key,
    pub src: String,
    pub payload: KademliaRequest,
}

/**
 * RPC:
 *  - Socket: Arc UdpSocket, here we use Arc to share 
 *      memory from the socket among threads.
 *  - MsgsMap: Arc Mutex of HashMap with Key and Sender channel 
 *      (to send KademliaResponse's). Mutex, mutual exclusion 
 *      primitive used to protect data between threads (using locks).
 *      HashMap uses custom ids to save Sender channel to send custom
 *      messages (ABS: Message Stack).
 *  - Node: Current node.
 *  
 *  LINKS:
 *   > https://doc.rust-lang.org/std/sync/struct.Mutex.html
 *   > https://www.youtube.com/watch?v=_4fSLuvPMf8
**/
#[derive(Debug, Clone)]
pub struct Rpc {
    pub socket: Arc<UdpSocket>,
    pub msgsmap: Arc<Mutex<HashMap<Key, crossbeam_channel::Sender<Option<KademliaResponse>>>>>,
    pub node: Node,
}

impl Rpc {
    pub fn new(node: Node) -> Self {
        let socket = UdpSocket::bind(node.get_addr()).expect("Error in UDP Socket bind");
        
        Self { socket: Arc::new(socket), msgsmap: Arc::new(Mutex::new(HashMap::new())), node: node }
    }

    pub fn init(rpc: Rpc, sender_ch: crossbeam_channel::Sender<RpcRequestWithMeta>) {
        thread::spawn(move || {
            let mut buf = [0u8; 8192]; // TODO: Set buf size as parameter

            loop {
                let (len, src_addr) = rpc.socket.recv_from(&mut buf)
                    .expect("Error receiving data from node");
                
                let payload = String::from(str::from_utf8(&buf[..len])
                    .expect("Error parsing bytes as string"));

                let mut content: RpcMessage = serde_json::from_str(&payload)
                    .expect("Error decoding content from payload");

                content.src = src_addr.to_string();

                if content.dst != rpc.node.get_addr() {
                    continue
                }

                // RPC prints
                // println!("From {:?} to {:?}: {:?}", &content.src, &content.dst, &content.payload);

                match content.payload {
                    RpcPayload::Request(kadrequest) => {
                        let meta = RpcRequestWithMeta {
                            id: content.id,
                            src: content.src,
                            payload: kadrequest,
                        };

                        if let Err(_) = sender_ch.send(meta) {
                            break;
                        }
                    },
                    RpcPayload::Response(kadresponse) => {
                        rpc.clone().handle_response(content.id, kadresponse)
                    }
                    RpcPayload::End => {
                        break;
                    }
                }
            }
        });
    }

    /* 
        Handle incoming response from message hashmap (thus msgsmap id is needed)
    */
    pub fn handle_response(self, id: Key, response: KademliaResponse) {
        thread::spawn(move || {
            let mut msgsmap = self.msgsmap.lock()
                .expect("Error setting lock while handeling response");
            // Data unlocked from this point forward
            let state = match msgsmap.get(&id) {
                Some(sender_ch) => sender_ch.send(Some(response)),
                None => {eprintln!("Error getting sender channel for id: {:?}", id); return}
            };
            if let Ok(_) = state {
                msgsmap.remove(&id);
            }
        });
    }

    // Handle request to a destination node which will be caught on init() and sent to Kademlia instance
    pub fn handle_request(&self, request: KademliaRequest, dst_node: Node) 
        -> crossbeam_channel::Receiver<Option<KademliaResponse>> {
            let (sender_ch, receiver_ch) = crossbeam_channel::unbounded();
            let mut msgsmap = self.msgsmap.lock()
                .expect("Error setting lock from message hashmap while handeling request");
            let id = Key::new(format!("{}:{:?}", dst_node.get_addr(), SystemTime::now()));
            msgsmap.insert(id.clone(), sender_ch.clone());
            let rpcmsg = RpcMessage {
                id: id.clone(),
                src: self.node.get_addr(),
                dst: dst_node.get_addr(),
                payload: RpcPayload::Request(request)
            };

            self.send_msg(&rpcmsg);

            let rpcinstance = self.clone();
            thread::spawn(move || {
                thread::sleep(Duration::from_secs(TREPLICATE));
                if let Ok(_) = sender_ch.send(None) {
                    let mut msgsmap = rpcinstance.msgsmap.lock()
                        .expect("Error setting lock from message hashmap while handeling request");
                    msgsmap.remove(&id);
                }
            });          
            
            receiver_ch
    }

    pub fn send_msg(&self, rpcmsg: &RpcMessage) {
        let encodedmsg = serde_json::to_string(&rpcmsg)
            .expect("Error serializing RpcMessage while handeling request");
            
        self.socket.send_to(&encodedmsg.as_bytes(), &rpcmsg.dst)
            .expect("Error sending RpcMessage while handeling request");
    }
}

pub fn full_rpc_proc(rpc: &Rpc, request: KademliaRequest, node: Node) -> Option<KademliaResponse> {
    rpc.handle_request(request, node).recv()
        .expect("Error receiving response from rpc")
}

