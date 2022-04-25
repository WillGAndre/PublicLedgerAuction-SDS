pub mod node;
pub mod aux;
pub mod rpc;
pub mod kademlia;
pub mod blockchain;
pub mod bootstrap;

/**
 * BASED ON: KademliaBriefOverview.pdf 
**/

// 20 bytes (160 bits)
const KEY_LEN: usize = 20;

// Number of buckets
const N_KBUCKETS: usize = KEY_LEN * 8;

// Number of contacts in kbucket
const K_PARAM: usize = 20;

// ALPHA - degree of parallelism
const ALPHA: usize = 3;

// Timeout for Kademlia replication events
const TREPLICATE: u64 = 3600; 

#[cfg(test)]
mod tests {
    use super::node::{Node, NodeWithDistance, Distance, Key};
    use super::rpc::{Rpc, KademliaRequest};
    use super::kademlia::{KademliaInstance, RoutingTable, Bucket};
    use super::blockchain::{Block};
    use super::bootstrap::{Bootstrap};
    use super::aux;
    use super::{N_KBUCKETS, KEY_LEN};
    use log::{info};
    use std::time::Duration;
    use std::thread::{sleep};

    #[test]
    fn node_with_dist_test() {
        let node1 = Node::new(aux::get_ip().unwrap(), 1334);
        let node2 = Node::new(aux::get_ip().unwrap(), 1335);
        let dist = Distance::new(&node1.id, &node2.id);
        let nwd1 = NodeWithDistance(node1.clone(), dist.clone());
        let nwd2 = NodeWithDistance(node2.clone(), dist.clone());

        assert_eq!(nwd1, nwd2)
    }

    #[test]
    fn ping_test() {
        let node1 = Node::new(aux::get_ip().unwrap(), 1341);
        let node2 = Node::new(aux::get_ip().unwrap(), 1342);

        let kad1 = KademliaInstance::new(node1.addr.clone(), node1.port.clone(), None);
        let kad2 = KademliaInstance::new(node2.addr.clone(), node2.port.clone(), None);

        assert_eq!(kad1.ping(node2.clone()), true);
        assert_eq!(kad2.ping(node1.clone()), true);
    }

    #[test]
    fn get_bucket_index_test() {
        println!("---");
        println!("TEST LEADING ZEROS FROM XOR DISTANCE:");
        println!("---");
        let node1 = Node::new(aux::get_ip().unwrap(), 1332);
        let node2 = Node::new(aux::get_ip().unwrap(), 1333);
        let d = Distance::new(&node1.id, &node2.id); 

        println!("Node1: {}", node1.get_addr());
        for nd in node1.id.0 {
            print!("{:08b} ", nd);
        }
        print!("\n");

        println!("Node2: {}", node2.get_addr());
        for nd in node2.id.0 {
            print!("{:08b} ", nd);
        }
        print!("\n");
        
        println!("Distance: ");
        for dis in d.0 {
            print!("{:08b} ", dis);
        }
        print!("\n");

        let mut leading_zeros = 0;
        let mut bucket_index = 0;
        let mut stop = false;
        for i in 0..KEY_LEN {
            for shift in 0..8 {
                if (d.0[i] << shift) & 0b10000000 != 0 {
                    bucket_index = (N_KBUCKETS - 1) - ((8 * i) + shift);
                    leading_zeros = shift;
                    stop = true;
                    break
                } 
            }
            if stop {
                break
            }
        }

        println!("Leading Zeros: {}, Bucket Index: {}", leading_zeros, bucket_index);

        assert_eq!(bucket_index, 159 - leading_zeros);
    }

    #[test]
    fn query_node_test() {
        let node1 = Node::new(aux::get_ip().unwrap(), 1337);
        let node2 = Node::new(aux::get_ip().unwrap(), 1338);

        let kad1 = KademliaInstance::new(node1.addr.clone(), node1.port.clone(), None);        
        let _kad2 = KademliaInstance::new(node2.addr.clone(), node2.port.clone(), Some(node1.clone()));

        // Node1 doesn't know of node2's existence.
        let mut query_node12 = kad1.query_node(node1.clone(), node2.id.clone()).unwrap();
        assert_eq!(query_node12.len(), 0);

        // although if we query node2 for himself (from node1),
        // then we populate the routing table with node2.
        // NOTE: A ping may be used instead of query_node
        query_node12 = kad1.query_node(node2.clone(), node2.id.clone()).unwrap();
        assert_eq!(query_node12.len(), 1);
        assert_eq!(query_node12.pop().unwrap().0.id, node2.id.clone());

        // running the initial query once more, one can verify
        // that node2 is in node1's routing table.
        query_node12 = kad1.query_node(node1.clone(), node2.id.clone()).unwrap();
        assert_eq!(query_node12.len(), 1);
        assert_eq!(query_node12.pop().unwrap().0.id, node2.id.clone());
    }

    #[test]
    fn find_node_test() {
        let node1 = Node::new(aux::get_ip().unwrap(), 1344);
        let node2 = Node::new(aux::get_ip().unwrap(), 1345);
        let node3 = Node::new(aux::get_ip().unwrap(), 1346);

        let kad1 = KademliaInstance::new(node1.addr.clone(), node1.port.clone(), Some(node2.clone()));        
        let kad2 = KademliaInstance::new(node2.addr.clone(), node2.port.clone(), Some(node1.clone()));
        let _kad3 = KademliaInstance::new(node3.addr.clone(), node3.port.clone(), Some(node2.clone()));

        // Node1 has node2 in his routing table,
        // Node2 has node1 in his routing table,
        // Node3 has node2 in his routing table
        let find_node2 = kad1.find_node(&node2.id.clone());
        let query_for_node2 = kad1.query_node(node1.clone(), node2.id.clone());
        assert_eq!(find_node2, query_for_node2.unwrap());

        // Add node3 to node2's routing table
        kad2.ping(node3.clone());

        // kad1.print_routing_table();
        // println!("\n");
        // kad2.print_routing_table();

        let same_bucket = kad1.same_bucket(&node2.id.clone(), &node3.id.clone());
        println!("Is node2 and node3 stored in the same bucket: {}", same_bucket);
        let find_node3 = kad1.find_node(&node3.id.clone());
        // let d12 = Distance::new(&node1.id.clone(), &node2.id.clone());
        let d13 = Distance::new(&node1.id.clone(), &node3.id.clone());
        let d23 = Distance::new(&node2.id.clone(), &node3.id.clone());
        let d33 = Distance::new(&node3.id.clone(), &node3.id.clone());

        let node1_index = find_node3.iter().position(|n| n.0.id == node1.id);
        let node2_index = find_node3.iter().position(|n| n.0.id == node2.id);
        let node3_index = find_node3.iter().position(|n| n.0.id == node3.id);

        // println!("find_node3: {:?}", find_node3);
        
        match node1_index {
            Some(i) => {
                let nwd = find_node3[i].clone();
                assert_eq!(nwd.1, d13);
            },
            None => {
                if same_bucket {
                    assert_eq!(node1_index, None)
                } else {
                    assert_ne!(node1_index, None)
                }
            }
        }
        match node2_index {
            Some(i) => {
                let nwd = find_node3[i].clone();
                assert_eq!(nwd.1, d23);
            },
            None => assert_ne!(node2_index, None)
        }
        match node3_index {
            Some(i) => {
                let nwd = find_node3[i].clone();
                assert_eq!(nwd.1, d33);
            },
            None => assert_ne!(node2_index, None)
        }
    }

    #[test]
    fn insert_get_test() {
        let node1 = Node::new(aux::get_ip().unwrap(), 1347);
        let node2 = Node::new(aux::get_ip().unwrap(), 1348);
        let node3 = Node::new(aux::get_ip().unwrap(), 1349);

        let kad1 = KademliaInstance::new(node1.addr.clone(), node1.port.clone(), None);        
        let _kad2 = KademliaInstance::new(node2.addr.clone(), node2.port.clone(), Some(node1.clone()));
        let _kad3 = KademliaInstance::new(node3.addr.clone(), node3.port.clone(), Some(node1.clone()));

        kad1.ping(node2.clone());
        kad1.ping(node3.clone());
        _kad2.ping(node3.clone());

        kad1.insert("test_key".to_owned(), "test_value".to_owned());
        assert_eq!(kad1.get("test_key".to_owned()).unwrap(), "test_value".to_string());

        // println!("KAD1:");
        // kad1.print_hashmap();
        // println!("KAD2:");
        // _kad2.print_hashmap();
        // println!("KAD3:");
        // _kad3.print_hashmap();
    }

    #[test]
    fn blockchain_test() {
        let node1 = Node::new(aux::get_ip().unwrap(), 1350);
        let node2 = Node::new(aux::get_ip().unwrap(), 1351);

        let kad1 = KademliaInstance::new(node1.addr.clone(), node1.port.clone(), Some(node2.clone()));
        let kad2 = KademliaInstance::new(node2.addr.clone(), node2.port.clone(), Some(node1.clone()));

        let res21 = kad2.query_blockchain(node1.clone());
        let res12 = kad1.query_blockchain(node2.clone());
        println!("res12: {:?}", res21);
        println!("res12: {:?}", res12);
    }

    #[test]
    fn bootstrap_test() {
        let boot = Bootstrap::new();
        println!("/n/n");
        boot.nodes[0].kademlia.print_blockchain();

        let register = boot.nodes[0].join_network(boot.nodes[1].clone());
        println!("Register: {}", register);

        boot.nodes[0].kademlia.print_blockchain();
        boot.nodes[1].kademlia.print_blockchain();
    }

    // #[test]
    // fn bootstrap_test() {
    //     let boot = Bootstrap::new();
    //     Bootstrap::init(boot.clone());
    //     //boot.mstrnodes[0].kademlia.print_routing_table();

    //     boot.authnodes[0].add_block(Block::new(1, "0000f816a87f806bb0073dcf026a64fb40c946b5abee2573702828694d5b4c43".to_string(), "test".to_string()));

    //     // Not syncronized
    //     //boot.authnodes[0].kademlia.print_blockchain();
    //     boot.authnodes[1].kademlia.print_blockchain();

    //     println!("wait: 20s");
    //     sleep(Duration::from_secs(20));
    //     boot.authnodes[1].kademlia.print_blockchain();
    // }

    // #[test]
    // fn bootstrap_add_node_test() {
    //     let boot = Bootstrap::new();
    //     Bootstrap::init(boot.clone());

    //     let new_node = LightNode::new(aux::get_ip().unwrap(), 1355, None);
    //     let try_node = Node::new(boot.routnodes[0].node.addr.clone(), boot.routnodes[0].node.port);
    //     let join = new_node.join_network(try_node);
        
    //     println!("\n******");
    //     println!("Join Network: {}", join);
    //     println!("LightNode:");
    //     new_node.kademlia.print_routing_table();
    //     println!("1st RoutingNode");
    //     boot.routnodes[0].kademlia.print_routing_table();
    //     println!("2nd RoutingNode");
    //     boot.routnodes[1].kademlia.print_routing_table();

    //     println!("wait: 20s");
    //     sleep(Duration::from_secs(20));
    //     // 1355 will now be present in rn1 routing table
    //     // as well as in rn0.
    //     println!("2nd RoutingNode");
    //     boot.routnodes[1].kademlia.print_routing_table();
    // }

    // /*
    //     TODO: add full test (with node addition + blockchain insert) x2
    // */

    // #[test]
    // fn fullscale_test() {
    //     let boot = Bootstrap::new();node: Nodes[0].node.addr.clone(), boot.routnodes[0].node.port);
    //     let ln1 = LightNode::new(aux::get_ip().unwrap(), 1355, None);
    //     let ln2 = LightNode::new(aux::get_ip().unwrap(), 1356, None);

    //     let ln1_register = ln1.join_network(entry_node.clone());
    //     println!("LN1 Register: {}", ln1_register);

    //     let ln2_register = ln2.join_network(entry_node);
    //     println!("LN2 Register: {}", ln2_register);

    //     boot.authnodes[0].add_block(Block::new(1, "0000f816a87f806bb0073dcf026a64fb40c946b5abee2573702828694d5b4c43".to_string(), "test0".to_string()));
    //     boot.authnodes[1].add_block(Block::new(1, "0000f816a87f806bb0073dcf026a64fb40c946b5abee2573702828694d5b4c43".to_string(), "test1".to_string()));

    //     println!("wait: 20s");
    //     sleep(Duration::from_secs(20));
    //     println!("1st RoutingNode");
    //     boot.routnodes[0].kademlia.print_routing_table();
    //     println!("2nd RoutingNode");
    //     boot.routnodes[1].kademlia.print_routing_table();
    //     println!("1st AuthNode:");
    //     boot.authnodes[0].kademlia.print_blockchain();
    //     println!("2nd AuthNode:");
    //     boot.authnodes[1].kademlia.print_blockchain();

    //     println!();
    //     println!("wait: 20s");
    //     sleep(Duration::from_secs(20));
    //     println!("1st AuthNode:");
    //     boot.authnodes[0].kademlia.print_blockchain();
    //     println!("2nd AuthNode:");
    //     boot.authnodes[1].kademlia.print_blockchain();
    // }
}