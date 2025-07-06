use std::net::UdpSocket;

fn main() {
    println!("Hello, world!");
    let socket = UdpSocket::bind("127.0.0.1:9476").unwrap();
    socket.connect("127.0.0.1:9475").unwrap();

    loop {
        let mut buf = [0u8; 8192];
        socket.recv_from(&mut buf).unwrap();
        println!("{:?}", buf);
    }

}