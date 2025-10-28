use std::net::Ipv4Addr;
use std::time::Duration;
use pnet::datalink::MacAddr;
use arp::NetworkScanner;

#[tokio::main]
async fn main() {
    let mut scanner = NetworkScanner::new(
        "enp14s0",
        Ipv4Addr::new(192,168,1,1)..Ipv4Addr::new(192,168,1,254),
        Duration::from_millis(10),
        Duration::from_millis(50)
    ).unwrap();
    let mut receiver = scanner.subscribe(MacAddr(0xe8,0x78,0x29,0xc5,0xaf,0x6f));
    std::thread::spawn(|| {
        scanner.run();
    });
    loop {
        match *receiver.borrow_and_update() {
            None => {
                println!("Dylan's phone is not connected to the home wifi");
            }
            Some(ip) => {
                println!("Dylan's phone is connected to the home wifi on {ip}");
            }
        }
        receiver.changed().await.unwrap();
    }
}