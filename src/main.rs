extern crate kad;
use kad::bootstrap::Bootstrap;

fn main() {
    let boot = Bootstrap::new();
    Bootstrap::full_bk_sync(boot.clone());
    loop {}
}