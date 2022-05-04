use std::sync::{Arc, Mutex};
use std::collections::{HashMap};
use serde::{Serialize, Deserialize};
use std::fmt::{Display, Formatter, Result};

use super::node::{Key};

/*
    TODO:
        Extend rpc to add PubSub Requests/Responses
        Add PubSub instance
*/

#[derive(Debug, Clone)]
pub struct PubSubInstance {
    pub msgstack: Arc<Mutex<Vec<String>>>,
    pub substack: Arc<Mutex<Vec<Key>>>,
    pub publisher: Key
}

impl PubSubInstance {
    pub fn new(publisherid: Key) -> Self {
        Self {
            msgstack: Arc::new(Mutex::new(Vec::new())),
            substack: Arc::new(Mutex::new(Vec::new())),
            publisher: publisherid
        }
    }

    pub fn add_msg(&self, msg: String) {
        let mut msgstack = self.msgstack.lock()
            .expect("Error setting lock in msg stack");
        msgstack.push(msg);
    }

    pub fn add_sub(&self, sub: Key) {
        let mut substack = self.substack.lock()
            .expect("Error setting lock in msg stack");
        substack.push(sub);
    }

    // ---

    fn print_msgstack(&self) -> String {
        let msgstack = self.msgstack.lock()
            .expect("Error setting lock in msg stack");
        let msgstack_clone = msgstack.clone();
        drop(msgstack);
        format!("{:?}", msgstack_clone)
    }

    fn print_substack(&self) -> String {
        let substack = self.substack.lock()
            .expect("Error setting lock in msg stack");
        let substack_clone = substack.clone();
        drop(substack);
        format!("{:?}", substack_clone)
    }
}

impl Display for PubSubInstance {
    fn fmt(&self, f: &mut Formatter) -> Result {
        write!(f, "{:?}:{}:{}", self.publisher, self.print_substack(), self.print_msgstack())
    }
}

