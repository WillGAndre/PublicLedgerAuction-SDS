use std::sync::{Arc, Mutex};
use std::fmt::{Display, Formatter, Result};
use base64::{encode};

#[derive(Debug, Clone)]
pub struct PubSubInstance {
    pub msgstack: Arc<Mutex<Vec<String>>>,
    pub substack: Arc<Mutex<Vec<String>>>,
    pub publisher: String
}

/*
    Improvement:
     - Use compression function to compress string (pubsub instance),
       before adding to hashmap (https://crates.io/crates/compressed_string).

     - Improve print function(s).
*/

impl PubSubInstance {
    pub fn new(publisher: String, msgstack: Option<Vec<String>>, substack: Option<Vec<String>>) -> Self {
        if msgstack != None && substack != None {
            return
                Self {
                    msgstack: Arc::new(Mutex::new(msgstack.unwrap())),
                    substack: Arc::new(Mutex::new(substack.unwrap())),
                    publisher: publisher
                }
        }
        Self {
            msgstack: Arc::new(Mutex::new(Vec::new())),
            substack: Arc::new(Mutex::new(Vec::new())),
            publisher: publisher
        }
    }

    // TODO
    // pub fn relay_msgstack(pubsub: PubSubInstance) {
        
    // }

    // TODO: loop -> Relay msgs using substack (called when publish is performed)
    /*
        2 threads:
            - Sub msg (receive msgs from subs)
            - Sender (send msg only)
        
            (maintain concurrent list (state))
    */

    pub fn add_msg(&self, msg: String) {
        let mut msgstack = self.msgstack.lock()
            .expect("Error setting lock in msg stack");
        msgstack.push(msg);
        drop(msgstack)
    }

    pub fn add_sub(&self, sub: String) {
        if !self.verify(sub.clone()) {
            let mut substack = self.substack.lock()
                .expect("Error setting lock in msg stack");
            substack.push(sub);
            drop(substack)
        }
    }

    pub fn verify(&self, addr: String) -> bool {
        if self.publisher == addr {
            return true
        }
        self.verify_subs(addr)
    }

    fn verify_subs(&self, addr: String) -> bool {
        let substack = self.substack.lock()
            .expect("Error setting lock in substack");

        if substack.contains(&addr) {
            drop(substack);
            return true;
        }
        drop(substack);
        false
    }

    // ---

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

    fn print_substack(&self) -> String {
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
        str_to_encode.push_str(&self.publisher);
        str_to_encode.push_str(";");
        str_to_encode.push_str(&self.print_substack());
        str_to_encode.push_str(";");
        str_to_encode.push_str(&self.print_msgstack());
        encode(str_to_encode)
    }
}

impl Display for PubSubInstance {
    fn fmt(&self, f: &mut Formatter) -> Result {
        write!(f, "{}", self.encode_instance())
    }
}

