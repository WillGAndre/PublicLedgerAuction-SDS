use super::kademlia::KademliaInstance;
use super::node::{Node};
use super::aux::get_ip;

pub struct Bootstrap {
    pub mstrnodes: Vec<MasterNode>,
    pub authnodes: Vec<AuthorityNode>,
    pub routnodes: Vec<RoutingNode>,
}

impl Bootstrap {
    pub fn new() -> Self {
        let mut mst = Vec::new();
        mst.push(MasterNode::new(get_ip().unwrap(), 1330, None));
        let mut aut = Vec::new();
        aut.push(AuthorityNode::new(get_ip().unwrap(), 1331, None));
        aut.push(AuthorityNode::new(get_ip().unwrap(), 1332, None));
        aut.push(AuthorityNode::new(get_ip().unwrap(), 1333, None));
        let mut rt = Vec::new();
        rt.push(RoutingNode::new(get_ip().unwrap(), 1334, None));
        rt.push(RoutingNode::new(get_ip().unwrap(), 1335, None));
        Self {
            mstrnodes: mst,
            authnodes: aut,
            routnodes: rt,
        }
    }
}

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

impl MasterNode { // T
    pub fn new(addr: String, port: u16, bootstrap: Option<Node>) -> Self {
        Self {
            node: Node::new(addr.clone(), port.clone()),
            kademlia: KademliaInstance::new(addr, port, bootstrap)
        }
    }
}

pub struct AuthorityNode { // G
    pub node: Node,
    pub kademlia: KademliaInstance
}

impl AuthorityNode {
    pub fn new(addr: String, port: u16, bootstrap: Option<Node>) -> Self {
        Self {
            node: Node::new(addr.clone(), port.clone()),
            kademlia: KademliaInstance::new(addr, port, bootstrap)
        }
    }
}

pub struct RoutingNode { // G
    pub node: Node,
    pub kademlia: KademliaInstance
}

impl RoutingNode {
    pub fn new(addr: String, port: u16, bootstrap: Option<Node>) -> Self {
        Self {
            node: Node::new(addr.clone(), port.clone()),
            kademlia: KademliaInstance::new(addr, port, bootstrap)
        }


    }
}

pub struct LightNode { // T
    pub node: Node,
    pub kademlia: KademliaInstance
}

impl LightNode {
    pub fn new(addr: String, port: u16, bootstrap: Option<Node>) -> Self {
        Self {
            node: Node::new(addr.clone(), port.clone()),
            kademlia: KademliaInstance::new(addr, port, bootstrap)
        }
    }
}

//  ***
//  ---
//  ***