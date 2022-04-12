use super::kademlia::KademliaInstance;
use super::node::{Node};

/*
    TODO:
        MasterNode:
            Functionalities - Validate global blockchain (local AuthNode blockchain) with local /
                              Update global blockchain (given error, etc) /
                              Save hard copy of authNode blockchain in hashmap / 
                              Verify block transactions
        
        AuthorityNode:
            Functionalities - Receive mining requests (process block addition to blockchain) /
                              Distribute blockchain information (as broadcast)

        RoutingNode:
            Functionalities - Distribute routing information /
                              Maintain bootstrap nodes in routing table at all costs /
                              Handle new nodes and their addition to the network /
                              Relay AuthorityNode messages 

        LightNode:
            Functionalities - Simple Network Entity /
                              Mining capabilities /
                              General node functionalities (routing table / hashmap) /
    --- LINKS:
     https://nodes.com/
     https://www.sofi.com/learn/content/what-are-nodes/
    ---
*/

pub struct MasterNode {
    pub node: Node,
    pub kademlia: KademliaInstance
}

pub struct AuthorityNode { // G
    pub node: Node,
    pub kademlia: KademliaInstance
}

pub struct RoutingNode { // G
    pub node: Node,
    pub kademlia: KademliaInstance
}

pub struct LightNode {
    pub node: Node,
    pub kademlia: KademliaInstance
}

//  ***
//  ---
//  ***