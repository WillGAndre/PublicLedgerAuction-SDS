use std::sync::{Arc, Mutex};
use std::fmt::{Display, Formatter, Result};
use base64::{encode};
use chrono::{DateTime, Local};
use sha2::{Sha256, Digest};
use serde_json::{json, Value};

#[derive(Debug, Clone)]
pub struct PubSubInstance {
    pub id: String,
    pub msgstack: Arc<Mutex<Vec<String>>>,
    pub substack: Arc<Mutex<Vec<String>>>,
    pub publisher: String,
    pub ttl: Option<DateTime<Local>>,
}

/*
    Improvement:
     - Use compression function to compress string (pubsub instance),
       before adding to hashmap (https://crates.io/crates/compressed_string).

     - Improve print function(s).
*/

/*
    PubSubInstance: 
    Represented as the following string in kademlia DHT:
        <id> : <publisher_addr> : <msgstack> : <substack> : <TTL>
    Used to save state of our bidding sessions.
*/
impl PubSubInstance {
    pub fn new(id: Option<String>, publisher: String, msgstack: Option<Vec<String>>, substack: Option<Vec<String>>) -> Self {
        if msgstack != None && substack != None {
            return
                Self {
                    id: id.unwrap(),
                    msgstack: Arc::new(Mutex::new(msgstack.unwrap())),
                    substack: Arc::new(Mutex::new(substack.unwrap())),
                    publisher: publisher,
                    ttl: None
                }
        }
        let mut hasher = Sha256::new();
        hasher.update(format!("{}",Local::now()).as_bytes());
        Self {
            id: hex::encode(hasher.finalize()),
            msgstack: Arc::new(Mutex::new(Vec::new())),
            substack: Arc::new(Mutex::new(Vec::new())),
            publisher: publisher,
            ttl: None
        }
    }
    
    pub fn set_ttl(&mut self, ttl: DateTime<Local>) {
        self.ttl = Some(ttl);
    }

    // NOTE: Msgs should be of type "<raise_num>;<addr>"
    pub fn add_msg(&self, msg: String) -> usize {
        let id: &str = &self.id.clone()[..4];
        if self.verify_pubsub() {
            if self.verify_msg(msg.clone()) {
                let mut msgstack = self.msgstack.lock()
                    .expect("Error setting lock in msg stack");
                msgstack.push(msg);
                drop(msgstack);
                
                0
            } else {
                println!("\t[PubSub({})]: Invalid msg", id);

                1            
            }
        } else {
            println!("\t[PubSub({})]: PubSub TTL expired or not set", id);

            2
        }
    }

    // TODO: handle concurrent subs
    pub fn add_sub(&self, sub: String) {
        if self.verify_pubsub() && !self.verify_addr(sub.clone()) {
            let mut substack = self.substack.lock()
                .expect("Error setting lock in msg stack");
            substack.push(sub);
            drop(substack)
        }
    }

    //  - verify msg (tuple -> (number to raise bid; sender addr))
    // NOTE: Msgs should be of type "<raise_num>;<addr>"
    pub fn verify_msg(&self, msg: String) -> bool {
        let msgstack = self.msgstack.lock()
            .expect("Error setting lock in msg stack");
        
        if msgstack.is_empty() || (msgstack.last().unwrap() == "" && msgstack.len() == 1){
            drop(msgstack);
            return true
        } else {
            let stack = msgstack.clone();
            drop(msgstack);
            let last_val: Value = serde_json::from_str(stack.last().unwrap()).unwrap();
            let new_val: Value = serde_json::from_str(&msg).unwrap();

            let last_raise: usize = last_val["data"].to_string().parse::<usize>().unwrap();
            let new_raise: usize = new_val["data"].to_string().parse::<usize>().unwrap();

            if new_raise > last_raise {
                return true
            }
        }
        false
    }

    pub fn verify_addr(&self, addr: String) -> bool {
        if self.publisher == addr {
            return true
        }
        let substack = self.substack.lock()
            .expect("Error setting lock in substack");

        if substack.contains(&addr) {
            drop(substack);
            return true;
        }
        drop(substack);
        false
    }

    pub fn verify_pubsub(&self) -> bool { // TODO: diff calc
        if self.ttl == None {
            return false
        }

        let time = Local::now();
        let diff = (self.ttl.unwrap() - time).num_seconds(); // num_minutes
        if diff > 0 {
            return true
        }
        
        false
    }

    // ---

    // PubSub info sent to App -> used at cli
    pub fn as_json(&self) -> Value {
        let id: &str = &self.id.clone()[..4];
        let msgstack = &self.msgstack.lock()
            .expect("Error setting lock in msgstack");
        let stack = msgstack;
        drop(msgstack);
        let last_entry: &String = stack.last().unwrap();
        let mut last_entry_json: Value = json!({"data":0,"sender_addr":"unknown"});
        if last_entry != ""
        {
            last_entry_json = serde_json::from_str(last_entry).unwrap();
        }
        let substack = &self.substack.lock()
            .expect("Error setting lock in substack");
        let num_subs = substack.len();
        drop(substack);
        json!(
            {
                "id": id,
                "num_subs": num_subs,
                "highest_bid": last_entry_json["data"],
                "highest_bidder": last_entry_json["sender_addr"],
                "ttl": format!("{}", self.ttl.unwrap()),
            }
        )
    } 

    fn print_msgstack(&self) -> String {
        let mut msgstack_str = String::new();
        let msgstack = self.msgstack.lock()
            .expect("Error setting lock in msg stack");
        let msgstack_clone = msgstack.clone();
        drop(msgstack);
        let msgstack_len = msgstack_clone.len();
        let mut iter = 0;
        for msg in msgstack_clone {
            let mut full = String::new();
            full.push_str(&msg);
            if iter < msgstack_len-1 {
                full.push_str(" ");
            }
            msgstack_str.push_str(&full);
            iter += 1;
        }
        format!("{}", msgstack_str)
    }

    pub fn print_substack(&self) -> String {
        let mut substack_str = String::new();
        let substack = self.substack.lock()
            .expect("Error setting lock in msg stack");
        let substack_clone = substack.clone();
        drop(substack);

        let substack_len = substack_clone.len();
        let mut iter = 0;
        for sub in substack_clone {
            let mut full = String::new();
            full.push_str(&sub);
            if iter < substack_len-1 {
                full.push_str(" ");
            }
            substack_str.push_str(&full);
            iter += 1
        }

        format!("{}", substack_str)
    }

    fn encode_instance(&self) -> String {
        let mut str_to_encode = String::new();
        str_to_encode.push_str(&self.id);
        str_to_encode.push_str(";");
        str_to_encode.push_str(&self.publisher);
        str_to_encode.push_str(";");
        str_to_encode.push_str(&self.print_substack());
        str_to_encode.push_str(";");
        str_to_encode.push_str(&self.print_msgstack());
        str_to_encode.push_str(";");
        if self.ttl != None {
            str_to_encode.push_str(&format!("{}", (self.ttl.unwrap())))
        } else {
            str_to_encode.push_str("NONE")
        }
        // ---
        // println!("PUBSUB BEFORE ENCODE: {}", str_to_encode);
        // ---
        encode(str_to_encode)
    }
}

impl Display for PubSubInstance {
    fn fmt(&self, f: &mut Formatter) -> Result {
        write!(f, "{}", self.encode_instance())
    }
}

