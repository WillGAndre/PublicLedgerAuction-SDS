pub mod node;
pub mod aux;
pub mod rpc;
pub mod kademlia;
pub mod blockchain;
pub mod bootstrap;
pub mod pubsub;

/**
 * BASED ON: KademliaBriefOverview.pdf 
**/

// 20 bytes (160 bits)
pub const KEY_LEN: usize = 20;

// Number of buckets
pub const N_KBUCKETS: usize = KEY_LEN * 8;

// Number of contacts in kbucket
pub const K_PARAM: usize = 20;

// ALPHA - degree of parallelism
pub const ALPHA: usize = 3;

// Timeout for Kademlia replication events
pub const TREPLICATE: u64 = 3600; 

// Timeout in secs
pub const NODETIMEOUT: u64 = 1;

// cargo test -- --nocapture --test pub_teardown_test
#[cfg(test)]
mod tests {
    use super::node::{Node, NodeWithDistance, Distance, Key};
    use super::rpc::{Rpc, KademliaRequest};
    use super::kademlia::{KademliaInstance, RoutingTable, Bucket};
    use super::blockchain::{Block};
    use super::bootstrap::{Bootstrap, AppNode, App};
    use super::pubsub::{PubSubInstance};
    use super::aux;
    use super::{N_KBUCKETS, KEY_LEN, NODETIMEOUT};
    use log::{info};
    use std::time::Duration;
    use std::thread::{sleep, spawn};
    use std::sync::mpsc::{channel, Sender};

    use base64::{decode};

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

    // NOTE: appnode and join network (bootnode0) have the same global (updated) blockchain,
    //       while bootnode1/2/3 still have the local chain.
    //
    // IMP:  blockchain should be queried before any action 
    #[test]
    fn bootstrap_test() {
        let boot = Bootstrap::new();
        let appnode = AppNode::new(aux::get_ip().unwrap(), 1335, None);

        println!();

        let register = appnode.join_network(boot.nodes[0].node.clone());
        println!("Register: {}", register);

        let appnodechain = appnode.kademlia.blockchain.lock()
            .expect("Error setting lock in test");
        let appnodechain_str = appnodechain.string();
        drop(appnodechain);
        let boot0chain = boot.nodes[0].kademlia.blockchain.lock()
            .expect("Error setting lock in test");
        let bootchain_str = boot0chain.string();
        drop(boot0chain);

        assert_eq!(appnodechain_str, bootchain_str);

        let boot1chain = boot.nodes[1].kademlia.blockchain.lock()
            .expect("Error setting lock in test");
        let boot1chain_str = boot1chain.string();
        drop(boot1chain);

        let boot2chain = boot.nodes[2].kademlia.blockchain.lock()
            .expect("Error setting lock in test");
        let boot2chain_str = boot2chain.string();
        drop(boot2chain);

        let boot3chain = boot.nodes[3].kademlia.blockchain.lock()
            .expect("Error setting lock in test");
        let boot3chain_str = boot3chain.string();
        drop(boot3chain);

        assert_eq!(boot1chain_str, boot2chain_str);
        assert_eq!(boot1chain_str, boot3chain_str);
        assert_eq!(boot2chain_str, boot2chain_str);
        
        // prints ---
        // appnode.kademlia.print_routing_table();
        // println!(" --- ");
        // boot.nodes[0].kademlia.print_routing_table();
        // println!(" --- ");
        // boot.nodes[1].kademlia.print_routing_table();
        // println!(" --- ");
        // boot.nodes[2].kademlia.print_routing_table();
        // println!(" --- ");
        // boot.nodes[3].kademlia.print_routing_table();

        // appnode.kademlia.print_blockchain();
        // println!(" --- ");
        // boot.nodes[0].kademlia.print_blockchain();
        // println!(" --- ");
        // boot.nodes[1].kademlia.print_blockchain();
        // println!(" --- ");
        // boot.nodes[2].kademlia.print_blockchain();
    }

    #[test]
    fn pubsub_test() {
        let boot = Bootstrap::new();
        let appnode = AppNode::new(aux::get_ip().unwrap(), 1335, None);

        println!();

        let register = appnode.join_network(boot.nodes[0].node.clone());
        println!("AppNode register: {}", register);

        println!();
        println!("BootNode0 published test");
        boot.nodes[0].publish(String::from("test"), chrono::Local::now() + chrono::Duration::minutes(15));

        // NOTE: Without gets only BootNode3 would have this topic.
        // Because BootNode2 can find the topic, he adds the info to the
        // first intermediate node (from the get search) (H1) or to all 
        // intermediate nodes (H2).
        //
        // FOR MORE INFO: Check else statement in get (kademlia)
        // 
        // AppNode get fails since he has no one near the key ("test"),
        // the node near is BootNode3 but he doesnt have him in his routingtable.
        let get_appnode = appnode.kademlia.get(String::from("test"));
        println!("AppNode GET: {:?}", get_appnode);
        let get_bootnode = boot.nodes[2].kademlia.get(String::from("test"));
        println!("BootNode2 GET: {:?}", get_bootnode);

        let query_value = appnode.kademlia.query_value(boot.nodes[0].node.clone(), String::from("test"));
        println!("AppNode QUERY topic to BootNode0: {:?}", query_value);

        println!();
        println!("---");

        println!("AppNode DHT: {}", appnode.kademlia.print_hashmap());
        println!(" --- ");
        println!("BootNode0 DHT: {}", boot.nodes[0].kademlia.print_hashmap());
        println!(" --- ");
        println!("BootNode1 DHT: {}", boot.nodes[1].kademlia.print_hashmap());
        println!(" --- ");
        println!("BootNode2 DHT: {}", boot.nodes[2].kademlia.print_hashmap());
        println!(" --- ");
        println!("BootNode3 DHT: {}", boot.nodes[3].kademlia.print_hashmap());
    }

    #[test]
    fn pubsub_subscribe_test() {
        let boot = Bootstrap::new();
        println!();

        // PUBLISH
        println!("BootNode0 published test");
        boot.nodes[0].publish(String::from("test"), chrono::Local::now() + chrono::Duration::minutes(15));
        
        let get_bootnode = boot.nodes[2].kademlia.get(String::from("test"));
        println!("BootNode2 GET: {:?}", get_bootnode);
        
        // SUBSCRIBE 1
        println!("BootNode2 Subscribe test: ");
        boot.nodes[2].subscribe(String::from("test"));
        
        let get_bootnode2 = boot.nodes[2].kademlia.get(String::from("test")).unwrap();
        println!("BootNode2 GET: {:?}", decode_data(get_bootnode2));

        let get_bootnode1 = boot.nodes[1].kademlia.get(String::from("test")).unwrap();
        println!("BootNode1 GET: {:?}", decode_data(get_bootnode1));
        
        // SUBSCRIBE 2
        println!("BootNode1 Subscribe test: ");
        boot.nodes[1].subscribe(String::from("test"));

        let appnode = AppNode::new(aux::get_ip().unwrap(), 1335, None);

        println!();

        let register = appnode.join_network(boot.nodes[0].node.clone());
        println!("AppNode register: {}", register);
        let get_appnode = appnode.kademlia.get(String::from("test")).unwrap();
        println!("AppNode GET: {:?}", decode_data(get_appnode));

        println!("AppNode Subscribe test: ");
        appnode.subscribe(String::from("test"));

        // let subscribe = appnode.subscribe_network(String::from("test"), boot.nodes[3].clone());
        // println!("AppNode Subscribe Network test: {}", subscribe);
        
        let get_appnode = appnode.kademlia.get(String::from("test")).unwrap();
        println!("AppNode GET: {:?}", decode_data(get_appnode));

        let addmsg_appnode = appnode.add_msg(String::from("test"), String::from("testmsg"));
        println!("AppNode SendMsg test: {}", addmsg_appnode);
        let get_appnode = appnode.kademlia.get(String::from("test")).unwrap();
        println!("AppNode GET: {:?}", decode_data(get_appnode));


        println!();
        println!();
        // prints ---
        let get_bootnode0 = boot.nodes[0].kademlia.get(String::from("test")).unwrap();
        println!("BootNode0 GET: {:?}", decode_data(get_bootnode0));
        let get_bootnode1 = boot.nodes[1].kademlia.get(String::from("test")).unwrap();
        println!("BootNode1 GET: {:?}", decode_data(get_bootnode1));
        let get_bootnode2 = boot.nodes[2].kademlia.get(String::from("test")).unwrap();
        println!("BootNode2 GET: {:?}", decode_data(get_bootnode2));
        let get_bootnode3 = boot.nodes[3].kademlia.get(String::from("test")).unwrap();
        println!("BootNode3 GET: {:?}", decode_data(get_bootnode3));
    }

    #[test]
    fn pubsub_addmsg_test() {
        let boot = Bootstrap::new();
        Bootstrap::full_bk_sync(boot.clone());
        println!();

        let appnode0 = AppNode::new(aux::get_ip().unwrap(), 1335, None);
        let appnode1 = AppNode::new(aux::get_ip().unwrap(), 1336, None);
        let appnode2 = AppNode::new(aux::get_ip().unwrap(), 1337, None);

        let _register0 = appnode0.join_network(boot.nodes[0].node.clone());
        //println!("AppNode register: {}", register0);

        let _register1 = appnode1.join_network(boot.nodes[1].node.clone()); // boot.nodes[1].clone()
        //println!("AppNode register: {}", register1);

        let _register2 = appnode2.join_network(boot.nodes[2].node.clone()); // boot.nodes[2].clone()
        //println!("AppNode register: {}", register2);

        println!("AppNode1 publish: test + subscribe from AppNode0 and AppNode2");
        appnode1.publish(String::from("test"), chrono::Local::now() + chrono::Duration::minutes(15));
        appnode0.subscribe(String::from("test"));
        appnode2.subscribe(String::from("test"));

        let _addmsg_appnode0 = appnode0.add_msg(String::from("test"), String::from("testmsg:APPNODE0"));
        //println!("AppNode SendMsg test: {}", addmsg_appnode0);

        let _addmsg_appnode1 = appnode1.add_msg(String::from("test"), String::from("testmsg:APPNODE1"));
        //println!("AppNode SendMsg test: {}", addmsg_appnode1);

        let _addmsg_appnode2 = appnode2.add_msg(String::from("test"), String::from("testmsg:APPNODE2"));
        //println!("AppNode SendMsg test: {}", addmsg_appnode2);

        println!();
        println!();
        // prints ---
        let get_bootnode0 = boot.nodes[0].kademlia.get(String::from("test")).unwrap();
        println!("BootNode0 GET: {:?}", decode_data(get_bootnode0));
        let get_bootnode1 = boot.nodes[1].kademlia.get(String::from("test")).unwrap();
        println!("BootNode1 GET: {:?}", decode_data(get_bootnode1));
        let get_bootnode2 = boot.nodes[2].kademlia.get(String::from("test")).unwrap();
        println!("BootNode2 GET: {:?}", decode_data(get_bootnode2));
        let get_bootnode3 = boot.nodes[3].kademlia.get(String::from("test")).unwrap();
        println!("BootNode3 GET: {:?}", decode_data(get_bootnode3));

        let get_appnode0 = appnode0.kademlia.get(String::from("test")).unwrap();
        println!("AppNode0 GET: {:?}", decode_data(get_appnode0));
        let get_appnode1 = appnode1.kademlia.get(String::from("test")).unwrap();
        println!("AppNode1 GET: {:?}", decode_data(get_appnode1));
        let get_appnode2 = appnode2.kademlia.get(String::from("test")).unwrap();
        println!("AppNode0 GET: {:?}", decode_data(get_appnode2));

        println!();
        println!("AppNode0 BK:");
        appnode0.kademlia.print_blockchain();
        println!("AppNode1 BK:");
        appnode1.kademlia.print_blockchain();
        println!("AppNode2 BK:");
        appnode2.kademlia.print_blockchain();
        println!();
        println!("AppNode0 HMP:");
        println!("{}", appnode0.kademlia.print_hashmap());
        println!("AppNode1 HMP:");
        println!("{}", appnode1.kademlia.print_hashmap());
        println!("AppNode2 HMP:");
        println!("{}", appnode2.kademlia.print_hashmap());
        println!();
        println!(" --- ");
        println!("BootNode0 BK:");
        boot.nodes[0].kademlia.print_blockchain();
        println!(" --- ");
        println!("BootNode1 BK:");
        boot.nodes[1].kademlia.print_blockchain();
        println!(" --- ");
        println!("BootNode2 BK:");
        boot.nodes[2].kademlia.print_blockchain();
        println!(" --- ");
        println!("BootNode3 BK:");
        boot.nodes[3].kademlia.print_blockchain()
    }

    #[test]
    fn simple_bk_sync_test() {
        let boot = Bootstrap::new();
        Bootstrap::full_bk_sync(boot.clone());
        println!();

        let appnode0 = AppNode::new(aux::get_ip().unwrap(), 1335, None);
        let appnode1 = AppNode::new(aux::get_ip().unwrap(), 1336, None);
        let appnode2 = AppNode::new(aux::get_ip().unwrap(), 1337, None);


        let _register0 = appnode0.join_network(boot.nodes[0].node.clone());
        let _register1 = appnode1.join_network(boot.nodes[1].node.clone());
        let _register2 = appnode2.join_network(boot.nodes[2].node.clone());

        println!();
        println!("AppNode0 BK:");
        appnode0.kademlia.print_blockchain();
        println!(" --- ");
        println!("AppNode1 BK:");
        appnode1.kademlia.print_blockchain();
        println!(" --- ");
        println!("BootNode0 BK:");
        boot.nodes[0].kademlia.print_blockchain();
        println!(" --- ");
        println!("BootNode1 BK:");
        boot.nodes[1].kademlia.print_blockchain();
        println!(" --- ");
        println!("BootNode2 BK:");
        boot.nodes[2].kademlia.print_blockchain();
        println!(" --- ");
        println!("BootNode3 BK:");
        boot.nodes[3].kademlia.print_blockchain()
    }

    #[test]
    pub fn thread_register() {
        let boot = Bootstrap::new();
        Bootstrap::full_bk_sync(boot.clone());
        println!();

        let appnode0 = AppNode::new(aux::get_ip().unwrap(), 1335, None);
        let appnode1 = AppNode::new(aux::get_ip().unwrap(), 1336, None);
        let appnode2 = AppNode::new(aux::get_ip().unwrap(), 1337, None);
        let appnodes = vec![appnode0.clone(), appnode1.clone(), appnode2.clone()];
        let mut threads = vec![];

        for i in 0..3 {
            let appnode = appnodes[i].clone();
            let bootnode = boot.nodes[i].clone();
            threads.push(spawn(move || {
                let mut res = String::new();
                let mut reg = appnode.join_network(bootnode.node.clone());
                if !reg {
                    sleep(Duration::from_secs(NODETIMEOUT));
                    reg = appnode.join_network(bootnode.node);
                }
                res.push_str(&format!("REGISTER APPNODE{}: {}", i, reg));
                return res
            }));
        }

        for thr in threads {
            let ans = thr.join();
            println!("{:?}", ans);
            println!(" --- ");
            boot.nodes[0].kademlia.print_blockchain();
            println!(" --- ");
            boot.nodes[1].kademlia.print_blockchain();
            println!(" --- ");
            boot.nodes[2].kademlia.print_blockchain();
            println!(" --- ");
        }
    }

    #[test]
    fn app_publish_test() {
        let boot = Bootstrap::new();
        Bootstrap::full_bk_sync(boot.clone());
        println!();

        let appnode0 = App::new(aux::get_ip().unwrap(), 1342, boot.nodes[0].node.clone());
        let appnode1 = App::new(aux::get_ip().unwrap(), 1343, boot.nodes[1].node.clone());
        let appnode2 = App::new(aux::get_ip().unwrap(), 1344, boot.nodes[2].node.clone());

        appnode0.publish(String::from("test"));

        sleep(Duration::from_secs(NODETIMEOUT * 2));

        let sub1 = appnode1.subscribe(String::from("test"));
        let sub2 = appnode2.subscribe(String::from("test"));

        //sleep(Duration::from_secs(NODETIMEOUT * 2));

        if !sub1 {
            appnode1.subscribe(String::from("test"));
        }
        if !sub2 {
            appnode2.subscribe(String::from("test"));
        }

        appnode1.add_msg(String::from("test"), String::from("bid 100"));
        appnode2.add_msg(String::from("test"), String::from("bid 200"));
        appnode1.add_msg(String::from("test"), String::from("bid 150"));

        // test print
        // println!("{}", appnode0.get_json(String::from("test")));

        println!();
        println!();
        // prints ---
        let get_bootnode0 = boot.nodes[0].kademlia.get(String::from("test")).unwrap();
        println!("BootNode0 GET: {:?}", decode_data(get_bootnode0));
        let get_bootnode1 = boot.nodes[1].kademlia.get(String::from("test")).unwrap();
        println!("BootNode1 GET: {:?}", decode_data(get_bootnode1));
        let get_bootnode2 = boot.nodes[2].kademlia.get(String::from("test")).unwrap();
        println!("BootNode2 GET: {:?}", decode_data(get_bootnode2));
        let get_bootnode3 = boot.nodes[3].kademlia.get(String::from("test")).unwrap();
        println!("BootNode3 GET: {:?}", decode_data(get_bootnode3));

        println!();
        println!("AppNode0 BK:");
        appnode0.appnode.kademlia.print_blockchain();
        println!("AppNode1 BK:");
        appnode1.appnode.kademlia.print_blockchain();
        println!("AppNode2 BK:");
        appnode2.appnode.kademlia.print_blockchain();
        println!();
        // println!("AppNode0 HMP:");
        // println!("{}", appnode0.appnode.kademlia.print_hashmap());
        // println!("AppNode1 HMP:");
        // println!("{}", appnode1.appnode.kademlia.print_hashmap());
        // println!("AppNode2 HMP:");
        // println!("{}", appnode2.appnode.kademlia.print_hashmap());
        // println!();
        println!(" --- ");
        println!("BootNode0 BK:");
        boot.nodes[0].kademlia.print_blockchain();
        println!(" --- ");
        println!("BootNode1 BK:");
        boot.nodes[1].kademlia.print_blockchain();
        println!(" --- ");
        println!("BootNode2 BK:");
        boot.nodes[2].kademlia.print_blockchain();
        println!(" --- ");
        println!("BootNode3 BK:");
        boot.nodes[3].kademlia.print_blockchain();
        println!();
        println!();
        println!("AppNode0 topics found: {:?}", appnode0.topics);
        println!("AppNode1 topics found: {:?}", appnode1.topics);
        println!("AppNode2 topics found: {:?}", appnode2.topics);
    }

    #[test]
    fn pub_teardown_test() {
        let boot = Bootstrap::new();
        Bootstrap::full_bk_sync(boot.clone());
        println!();

        let appnode0 = App::new(aux::get_ip().unwrap(), 1342, boot.nodes[0].node.clone());
        let appnode1 = App::new(aux::get_ip().unwrap(), 1343, boot.nodes[1].node.clone());

        appnode0.publish(String::from("test"));

        let mut sub1 = appnode1.subscribe(String::from("test"));
        if !sub1 {
            sub1 = appnode1.subscribe(String::from("test"));
        }
        let mut add1 = appnode1.add_msg(String::from("test"), String::from("bid 100"));
        if !add1 {
            add1 = appnode1.add_msg(String::from("test"), String::from("bid 100"));
        }

        sleep(Duration::from_secs(62));

        println!();
        println!();
        // prints ---
        let get_bootnode0 = boot.nodes[0].kademlia.get(String::from("test")).unwrap();
        println!("BootNode0 GET: {:?}", decode_data(get_bootnode0));
        let get_bootnode1 = boot.nodes[1].kademlia.get(String::from("test")).unwrap();
        println!("BootNode1 GET: {:?}", decode_data(get_bootnode1));
        let get_bootnode2 = boot.nodes[2].kademlia.get(String::from("test")).unwrap();
        println!("BootNode2 GET: {:?}", decode_data(get_bootnode2));
        let get_bootnode3 = boot.nodes[3].kademlia.get(String::from("test")).unwrap();
        println!("BootNode3 GET: {:?}", decode_data(get_bootnode3));

        println!();
        println!("AppNode0 BK:");
        appnode0.appnode.kademlia.print_blockchain();
        println!("AppNode1 BK:");
        appnode1.appnode.kademlia.print_blockchain();
        println!();
        // println!("AppNode0 HMP:");
        // println!("{}", appnode0.appnode.kademlia.print_hashmap());
        // println!("AppNode1 HMP:");
        // println!("{}", appnode1.appnode.kademlia.print_hashmap());
        // println!("AppNode2 HMP:");
        // println!("{}", appnode2.appnode.kademlia.print_hashmap());
        // println!();
        println!(" --- ");
        println!("BootNode0 BK:");
        boot.nodes[0].kademlia.print_blockchain();
        println!(" --- ");
        println!("BootNode1 BK:");
        boot.nodes[1].kademlia.print_blockchain();
        println!(" --- ");
        println!("BootNode2 BK:");
        boot.nodes[2].kademlia.print_blockchain();
        println!(" --- ");
        println!("BootNode3 BK:");
        boot.nodes[3].kademlia.print_blockchain();
        println!();
        println!();
        println!("AppNode0 topics found: {:?}", appnode0.topics);
        println!("AppNode1 topics found: {:?}", appnode1.topics);
    }    

    fn decode_data(data: String) -> String {
        String::from_utf8(decode(data.to_string()).expect("Error decoding data"))
                .expect("Error converting data to string")
    }
}