use std::net::UdpSocket;
use std::sync::{LockResult};

pub fn get_ip() -> Option<String> {
    let socket = UdpSocket::bind("0.0.0.0:0").expect("localhost bind");

    // google DNS
    match socket.connect("8.8.8.8:80") {
        Ok(_) => (),
        Err(_) => return None,
    }

    match socket.local_addr() {
        Ok(addr) => return Some(addr.ip().to_string()),
        Err(_) => return None, 
    }
}

// https://doc.rust-lang.org/stable/std/sync/type.LockResult.html
pub trait LockResultRes {
    type Guard;

    // https://doc.rust-lang.org/stable/std/sync/struct.Mutex.html#poisoning
    fn get_guard(self) -> Self::Guard;
}

impl<Guard> LockResultRes for LockResult<Guard> {
    type Guard = Guard;

    fn get_guard(self) -> Guard {
        self.unwrap_or_else(|e| e.into_inner())
    }
}