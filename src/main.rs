extern crate kad;
use kad::bootstrap::Bootstrap;

//  RUST_LOG=info cargo r
fn main() {
    env_logger::init();
    let boot = Bootstrap::new();
    Bootstrap::full_bk_sync(boot.clone());
    loop {}
}