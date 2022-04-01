pub mod node;
pub mod aux;
pub mod rpc;
pub mod kademlia;

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
    use super::node::{Node, NodeWithDistance, Distance};
    use super::rpc::{Rpc, KademliaRequest};
    use super::kademlia::{KademliaInstance, RoutingTable, Bucket};
    use super::aux;
    use super::{N_KBUCKETS, KEY_LEN};
    use log::{info};

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

        assert_eq!(leading_zeros, 0);
        assert_eq!(bucket_index, 159);
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
        let node1 = Node::new(aux::get_ip().unwrap(), 1343);
        let node2 = Node::new(aux::get_ip().unwrap(), 1344);
        let node3 = Node::new(aux::get_ip().unwrap(), 1345);

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

        //kad1.print_routing_table();
        //println!("\n");

        // Find node3 (from node1)
        let find_node3 = kad1.find_node(&node3.id.clone());
        assert_eq!(find_node3.len(), 2);
        assert_eq!(find_node3[0].0.id, node2.id);
        assert_eq!(find_node3[1].0.id, node3.id);

        //kad1.print_routing_table();
    }
}