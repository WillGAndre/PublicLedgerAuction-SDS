use std::net::UdpSocket;

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

