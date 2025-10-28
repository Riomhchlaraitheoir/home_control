use arp::{NetworkScanner, NetworkScannerConfig};
use pnet::datalink::MacAddr;
use std::net::Ipv4Addr;
use std::time::Duration;
use simple_log::Level;

#[tokio::main]
async fn main() {
    simple_log::new(
        simple_log::LogConfigBuilder::builder()
            .level(Level::Trace).unwrap()
            .build()
    ).unwrap();
    let (scanner, mut receiver) = NetworkScanner::new(
        "enp14s0",
        NetworkScannerConfig {
            timeout: Duration::from_secs(5),
            confirm_interval: Duration::from_secs(30),
            scan_interval: Duration::from_secs(5),
            ip_range: Ipv4Addr::new(192, 168, 1, 1)..Ipv4Addr::new(192, 168, 1, 254),
            device: MacAddr(0xe8, 0x78, 0x29, 0xc5, 0xaf, 0x6f), // e8:78:29:c5:af:6f
        }
    ).unwrap();
    std::thread::spawn(|| {
        scanner.run();
    });
    loop {
        receiver.changed().await.unwrap();
        let ip = *receiver.borrow_and_update();
        println!("Dylan's phone: {ip:?}");
    }
}
