use sha2::{Digest, Sha256};
use serde::{Serialize, Deserialize};
use chrono::prelude::*;
use std::fmt::{Display, Formatter, Result};

/**
 * Hash (of data in block) must start with 00.
 *  \
 *   -> Network attribute: agreed upon between nodes
 *      based on a consensus algorithm.
**/
const DIFFICULTY_PREFIX: &str = "00";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Blockchain {
    pub blocks: Vec<Block>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Block {
    pub id: u64,
    pub hash: String,
    pub prev_hash: String,
    pub timestamp: i64,
    pub data: String,
    pub nonce: u64,
}

/**
 * Helper Function: 
 *  Binary representation of a given byte array 
 *  in the form of a String.
 **/
fn hash_to_binary(hash: &[u8]) -> String {
    let mut res: String = String::default();
    for c in hash {
        res.push_str(&format!("{:b}", c));
    }
    res
}

fn calc_hash(id: u64, timestamp: i64, prev_hash: &str, data: &str, nonce: u64) -> Vec<u8> {
    let data = serde_json::json!({
        "id": id,
        "prev_hash": prev_hash,
        "data": data,
        "timestamp": timestamp,
        "nonce": nonce
    });
    let mut hasher = Sha256::new();
    hasher.update(data.to_string().as_bytes());
    hasher.finalize().as_slice().to_owned()
}

/**
 * Mining:
 *  From block data and nonce generate hash
 *  that starts with our difficulty prefix.
**/
fn mine_block(id: u64, timestamp: i64, prev_hash: &str, data: &str) -> (u64, String) {
    //info!("mining block");
    let mut nonce = 0;

    loop {
        // if nonce % 100000 == 0 {
        //    println!("nonce: {}", nonce);
        // }
        let hash = calc_hash(id, timestamp, prev_hash, data, nonce);
        let bin_hash = hash_to_binary(&hash);
        if bin_hash.starts_with(DIFFICULTY_PREFIX) {
            let hex_hash = hex::encode(&hash);
            //println!("mined nonce: {}, hash: {}, bin hash: {}",
            //    nonce, hex_hash, bin_hash
            //);
            return (nonce, hex_hash);
        }
        nonce += 1;
    }
}

impl Display for Blockchain {
    fn fmt(&self, f: &mut Formatter) -> Result {
        write!(f, "{:?}", self.blocks)
    }
}

/**
 * Simple consensus criteria:
 *  Ask other nodes for their chains, if 
 *  there is any longer than ours, use theirs.
**/
impl Blockchain {
    pub fn new() -> Self {
        Self { blocks: vec![] }
    }

    pub fn genesis(&mut self) {
        let genesis_block = Block {
            id: 0,
            timestamp: Utc::now().timestamp(),
            prev_hash: String::from("none"),
            data: String::from("genesis_block"),
            nonce: 2836,
            hash: "0000f816a87f806bb0073dcf026a64fb40c946b5abee2573702828694d5b4c43".to_string(),
        };
        self.blocks.push(genesis_block);
    }

    pub fn add_block(&mut self, block: Block) -> bool {
        let prev_block = self.blocks.last().expect("At least one block");
        if self.is_block_valid(&block, prev_block) {
            self.blocks.push(block);
            return true
        } else {
            // error msg
            // eprint!("unable to add block - invalid")
        }
        false
    }

    pub fn remove_last_block(&mut self) {
        self.blocks.pop();
    }

    fn is_block_valid(&self, block: &Block, prev_block: &Block) -> bool {
        if block.prev_hash != prev_block.hash {
            return false;
        } else if !hash_to_binary(&hex::decode(&block.hash).expect("Decodable hex")).starts_with(DIFFICULTY_PREFIX) {
            return false;
        } else if block.id != prev_block.id + 1 {
            return false;
        } else if hex::encode(calc_hash(
            block.id,
            block.timestamp,
            &block.prev_hash,
            &block.data,
            block.nonce,
        )) != block.hash {
            return false;
        }
        true
    }

    fn is_chain_valid(&self, chain: &[Block]) -> bool {
        for i in 0..chain.len() {
            if i == 0 {
                continue;
            }
            let prev_block = chain.get(i - 1).expect("Block must exist");
            let block = chain.get(i).expect("Block must exist");
            if !self.is_block_valid(block, prev_block) {
                return false;
            }
        }
        true
    }

    pub fn choose_chain(&self, local: Vec<Block>, remote: Vec<Block>) -> Vec<Block> {
        let is_local_valid = self.is_chain_valid(&local);
        let is_remote_valid = self.is_chain_valid(&remote);

        if is_local_valid && is_remote_valid {
            if local.len() >= remote.len() {
                local
            } else {
                remote
            }
        } else if is_local_valid && !is_remote_valid {
            local
        } else if !is_local_valid && is_remote_valid {
            remote
        } else {
            panic!("Local and remote chains both invalid");
        }
    }

    pub fn get_diff_from_chains(&self, local: Vec<Block>, remote: Vec<Block>) -> Vec<Block> {
        let is_local_valid = self.is_chain_valid(&local);
        let is_remote_valid = self.is_chain_valid(&remote);
        let mut res: Vec<Block> = Vec::new();

        if is_local_valid && is_remote_valid {
            let local_len = local.len();
            let remote_len = remote.len();
            if local_len > remote_len {
                res = local.clone().drain((local_len - (local_len - remote_len) - 1)..).collect()
            } else if remote_len > local_len {
                res = remote.clone().drain((remote_len - (remote_len - local_len) - 1)..).collect()
            }
            // TODO: what if eq
        }  
        
        res
    }

    pub fn string(&self) -> String {
        self.to_string()
    }
    
    pub fn hash(&self) -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.update(self.to_string().as_bytes());
        hasher.finalize().as_slice().to_owned()
    }
}

impl Block {
    pub fn new(id: u64, prev_hash: String, data: String) -> Self {
        let now = Utc::now();
        let (nonce, hash) = mine_block(id, now.timestamp(), &prev_hash, &data);
        Self {
            id,
            hash,
            timestamp: now.timestamp(),
            prev_hash,
            data,
            nonce,
        }
    }
}