use std::sync::{Arc, Mutex};
use std::fmt::{Display, Formatter, Result};

#[derive(Debug, Clone)]
pub struct PubSubInstance {
    pub msgstack: Arc<Mutex<Vec<String>>>,
    pub substack: Arc<Mutex<Vec<String>>>,
    pub publisher: String
}

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

    pub fn add_msg(&self, msg: String) {
        let mut msgstack = self.msgstack.lock()
            .expect("Error setting lock in msg stack");
        msgstack.push(msg);
        drop(msgstack)
    }

    pub fn add_sub(&self, sub: String) {
        let mut substack = self.substack.lock()
            .expect("Error setting lock in msg stack");
        substack.push(sub);
        drop(substack)
    }

    // ---

    fn print_msgstack(&self) -> String {
        let mut msgstack_str = String::new();
        let msgstack = self.msgstack.lock()
            .expect("Error setting lock in msg stack");
        let msgstack_clone = msgstack.clone();
        drop(msgstack);
        for msg in msgstack_clone {
            let mut full = String::new();
            full.push_str(&msg);
            full.push_str(" ");
            msgstack_str.push_str(&full)
        }
        format!("{}", msgstack_str)
    }

    fn print_substack(&self) -> String {
        let mut substack_str = String::new();
        let substack = self.substack.lock()
            .expect("Error setting lock in msg stack");
        let substack_clone = substack.clone();
        drop(substack);
        for sub in substack_clone {
            let mut full = String::new();
            full.push_str(&sub);
            full.push_str(" ");
            substack_str.push_str(&full)
        }
        format!("{}", substack_str)
    }
}

impl Display for PubSubInstance {
    fn fmt(&self, f: &mut Formatter) -> Result {
        write!(f, "{};{};{}", self.publisher, self.print_substack(), self.print_msgstack())
    }
}

