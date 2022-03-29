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
        let node1 = Node::new(aux::get_ip().unwrap(), 1336);
        let node2 = Node::new(aux::get_ip().unwrap(), 1337);
        let dist = Distance::new(&node1.id, &node2.id);
        let nwd1 = NodeWithDistance(node1.clone(), dist.clone());
        let nwd2 = NodeWithDistance(node2.clone(), dist.clone());

        assert_eq!(nwd1, nwd2)
    }

    /*
    #[test]
    fn routingtable_test() {
        let node1 = Node::new(aux::get_ip().unwrap(), 1334);
        let node2 = Node::new(aux::get_ip().unwrap(), 1335);
        let node3 = Node::new(aux::get_ip().unwrap(), 1336);
        let node4 = Node::new(String::from("asdasdasdasda"), 1337);

        let mut route1 = RoutingTable::new(node1.clone(), Some(node2.clone()));
        let _route2 = RoutingTable::new(node2.clone(), Some(node1.clone()));
        route1.update_routing_table(node3.clone());
        route1.update_routing_table(node4.clone());

        let mut route1_buckets_history: Vec<(usize, Bucket)> = Vec::new();
        let mut bucket_index: usize = 0;
        for bucket in route1.kbuckets.iter() {
            if bucket.nodes.len() > 0 {
                route1_buckets_history.push((bucket_index, bucket.clone()))
            }
            bucket_index += 1;
        }

        println!("  *   ------   *   ");
        println!("Node1: {}", node1.get_node());
        println!("Node2: {}", node2.get_node());
        println!("Node3: {}", node3.get_node());
        println!("Node4: {}", node4.get_node());
        println!("Distance from node1 to node2: {:?}", Distance::new(&node1.id, &node2.id));
        println!("{:?}", route1_buckets_history);
    }
    */

    #[test]
    fn kad_test() {
        let node1 = Node::new(aux::get_ip().unwrap(), 1341);
        let node2 = Node::new(aux::get_ip().unwrap(), 1342);

        let kad1 = KademliaInstance::new(node1.addr.clone(), node1.port.clone(), None);
        let kad2 = KademliaInstance::new(node2.addr.clone(), node2.port.clone(), Some(node1.clone()));

        assert_eq!(kad1.ping(node2.clone()), true);
        assert_eq!(kad2.ping(node1.clone()), true);
    }


    // TODO: VERIFY TEST
    #[test]
    fn find_node_test() {
        let node1 = Node::new(aux::get_ip().unwrap(), 1337);
        let node2 = Node::new(aux::get_ip().unwrap(), 1338);
        let node3 = Node::new(aux::get_ip().unwrap(), 1339);


        let kad1 = KademliaInstance::new(node1.addr.clone(), node1.port.clone(), None);
        let kad2 = KademliaInstance::new(node2.addr.clone(), node2.port.clone(), Some(node1.clone()));
        let kad3 = KademliaInstance::new(node3.addr.clone(), node3.port.clone(), Some(node2.clone()));

        let result1_to_3 = kad1.find_node(node2.clone(), node3.id);

        println!("\n");
        println!("Query node3 to node2 from node1: {:?}", result1_to_3);
    }

    #[test]
    fn get_bucket_index_test() {
        println!("---");
        println!("TEST LEADING ZEROS FROM XOR DISTANCE:");
        println!("---");
        let node1 = Node::new(aux::get_ip().unwrap(), 1336);
        let node2 = Node::new(aux::get_ip().unwrap(), 1337);
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
}