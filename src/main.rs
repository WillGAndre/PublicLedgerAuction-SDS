extern crate kad;
use kad::bootstrap::Bootstrap;

// TODO: ADD BK logs + verify pubsub teardown

fn main() {
    let boot = Bootstrap::new();
    Bootstrap::full_bk_sync(boot.clone());
    loop {}
}