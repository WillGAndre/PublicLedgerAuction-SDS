use super::KEY_LEN;

use log::{info};
use sha2::{Sha256, Digest};
use std::fmt::{Debug, Formatter, Error, Binary};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Debug)]
pub struct Node {
    pub id: Key,
    pub addr: String,
    pub port: u16,
}

/*
    Node:
    ID field serves as key, that is, array of bytes
    with KEY_LEN size (128).
*/
impl Node {
    pub fn new(addr: String, port: u16) -> Self {
        let full = format!("{}:{}", addr, port);
        let id = Key::new(full);
        
        Self {id , addr, port}
    }

    pub fn get_node(&self) -> String {
        format!("{:?} {}:{}", self.id, self.addr, self.port)
    }

    pub fn get_addr(&self) -> String {
        format!("{}:{}", self.addr, self.port)
    }
}

#[derive(Clone, Serialize, Deserialize, Ord, PartialOrd, Eq, PartialEq, Copy, Hash)]
pub struct Distance(pub [u8; KEY_LEN]);

impl Distance {
    pub fn new(k1: &Key, k2: &Key) -> Self {
        let mut res = [0u8; KEY_LEN];
        for i in 0..KEY_LEN {
            res[i] = k1.0[i] ^ k2.0[i];
        }
        Self(res)
    }
}

impl Debug for Distance {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        for x in &self.0 {
            match write!(f, "{}", x) {
                Ok(_) => {}
                Err(e) => {
                    info!("Error debuging distance: {}", e)
                }
            }
        }
        Ok(())
    }
}
impl Binary for Distance {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        for x in &self.0 {
            match write!(f, "{:08b}", x) {
                Ok(_) => {}
                Err(e) => {
                    info!("Error writing distance as binary: {}", e)
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Serialize, Deserialize, Eq, Hash, Debug)]
pub struct NodeWithDistance(pub Node, pub Distance);

// ONLY DISTANCE IS COMPARED
impl Ord for NodeWithDistance {
    fn cmp(&self, other: &NodeWithDistance) -> Ordering {
        self.1.cmp(&other.1)
    }
}
impl PartialOrd for NodeWithDistance {
    fn partial_cmp(&self, other: &NodeWithDistance) -> Option<Ordering> {
        Some(self.1.cmp(&other.1))
    }
}
impl PartialEq for NodeWithDistance {
    fn eq(&self, other: &NodeWithDistance) -> bool {
        for i in 0..KEY_LEN {
            if self.1.0[i] != other.1.0[i] {
                return false
            }
        }
        true
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Key(pub [u8; KEY_LEN]);

impl Key {
    pub fn new(input: String) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(input.as_bytes());

        let hash = hasher.finalize();
        let mut res = [0; KEY_LEN];

        for i in 0..KEY_LEN {
            res[i] = hash[i];
        }

        Self(res)
    }
}

impl Debug for Key {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        for x in &self.0 {
            match write!(f, "{}", x) {
                Ok(_) => {}
                Err(e) => {
                    info!("Error debuging key: {}", e)
                }
            }
        }
        Ok(())
    }
}
impl Binary for Key {
    fn fmt(&self, f: &mut Formatter) -> Result<(), Error> {
        for x in &self.0 {
            match write!(f, "{:08b}", x) {
                Ok(_) => {}
                Err(e) => {
                    info!("Error writing key as binary: {}", e)
                }
            }
        }
        Ok(())
    }
}