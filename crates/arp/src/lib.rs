use pnet::datalink::{Channel, DataLinkReceiver, DataLinkSender, NetworkInterface};
use pnet::packet::arp::{ArpHardwareTypes, ArpOperations, ArpPacket, MutableArpPacket};
use pnet::packet::ethernet::{EtherTypes, EthernetPacket, MutableEthernetPacket};
use pnet::util::MacAddr;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};
use std::ops::Range;
use std::thread::sleep;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::watch::{Receiver, Sender, channel};

#[derive(Debug, Clone)]
pub struct NetworkDevice {
    pub ip: Ipv4Addr,
    pub mac: MacAddr,
}

#[derive(Debug)]
pub struct NetworkScanner {
    pub timeout: Duration,
    pub interval: Duration,
    pub interface: NetworkInterface,
    pub senders: HashMap<MacAddr, Sender<Option<Ipv4Addr>>>,
    pub ip_range: Range<Ipv4Addr>,
    pub local: (MacAddr, Ipv4Addr),
}

impl NetworkScanner {
    pub fn new(
        interface_name: &str,
        ip_range: Range<Ipv4Addr>,
        timeout: Duration,
        interval: Duration,
    ) -> Result<Self, Error> {
        let interface = pnet::datalink::interfaces()
            .into_iter()
            .find(|i| i.name == interface_name && !i.is_loopback())
            .ok_or_else(|| Error::InterfaceNotFound(interface_name.to_string()))?;
        let local_ipv4 = interface
            .ips
            .iter()
            .find_map(|ip| match ip.ip() {
                IpAddr::V4(addr) => Some(addr),
                IpAddr::V6(_) => None,
            })
            .unwrap();

        let local_mac = interface.mac.unwrap();
        Ok(Self {
            interval,
            timeout,
            interface,
            senders: HashMap::new(),
            ip_range,
            local: (local_mac, local_ipv4),
        })
    }

    pub fn subscribe(&mut self, mac: MacAddr) -> Receiver<Option<Ipv4Addr>> {
        let (sender, receiver) = channel(None);
        self.senders.insert(mac, sender);
        receiver
    }

    pub fn run(self) -> ! {
        let mut cache = HashMap::<Ipv4Addr, MacAddr>::new();
        let (mut sender, mut receiver) = build_eth_channel(&self.interface);
        let (source_mac, source_ipv4) = self.local;
        let mut pkt_buf =
            [0u8; EthernetPacket::minimum_packet_size() + ArpPacket::minimum_packet_size()];

        // Use scope blocks so we can reborrow our buffer
        {
            // Build our base ethernet frame
            let mut pkt_eth = MutableEthernetPacket::new(&mut pkt_buf).unwrap();

            pkt_eth.set_destination(MacAddr::broadcast());
            pkt_eth.set_source(source_mac);
            pkt_eth.set_ethertype(EtherTypes::Arp);
        }

        {
            // Build the ARP frame on top of the ethernet frame
            let mut pkt_arp =
                MutableArpPacket::new(&mut pkt_buf[EthernetPacket::minimum_packet_size()..])
                    .unwrap();

            pkt_arp.set_hardware_type(ArpHardwareTypes::Ethernet);
            pkt_arp.set_protocol_type(EtherTypes::Ipv4);
            pkt_arp.set_hw_addr_len(6);
            pkt_arp.set_proto_addr_len(4);
            pkt_arp.set_operation(ArpOperations::Request);
            pkt_arp.set_sender_hw_addr(source_mac);
            pkt_arp.set_sender_proto_addr(source_ipv4);
            pkt_arp.set_target_hw_addr(MacAddr::broadcast());
        }
        let mut ips = self.ip_range.clone().cycle();
        'ip_loop: loop {
            sleep(self.interval);

            let Some(ip) = ips.next() else {
                unreachable!("ips is a cycling iterator, it will never end")
            };
            if ip == source_ipv4 {
                // do not check this machine's IP, that would be silly
                continue;
            }
            {
                let mut pkt_arp =
                    MutableArpPacket::new(&mut pkt_buf[EthernetPacket::minimum_packet_size()..])
                        .unwrap();
                pkt_arp.set_target_proto_addr(ip);
            }
            let start = Instant::now();
            sender.send_to(&pkt_buf, None).unwrap().unwrap();

            let mac = loop {
                let buf = receiver.next().unwrap();

                if buf.len()
                    < EthernetPacket::minimum_packet_size() + ArpPacket::minimum_packet_size()
                {
                    if Instant::now().duration_since(start) > self.timeout {
                        break None;
                    }
                    continue;
                }

                let pkt_arp =
                    ArpPacket::new(&buf[EthernetPacket::minimum_packet_size()..]).unwrap();

                if pkt_arp.get_sender_proto_addr() == ip
                    && pkt_arp.get_target_hw_addr() == source_mac
                {
                    break Some(pkt_arp.get_sender_hw_addr());
                }

                if Instant::now().duration_since(start) > self.timeout {
                    break None;
                }
            };
            if let Some(latest) = mac {
                // if mac address has been found
                if let Some(previous) = cache.insert(ip, latest) {
                    if latest == previous {
                        // no update continue to next ip
                        continue 'ip_loop;
                    } else {
                        if let Some(sender) = self.senders.get(&previous) {
                            sender
                                .send(None)
                                .expect("failed to send ARP update to channel");
                        }
                        if let Some(sender) = self.senders.get(&latest) {
                            sender
                                .send(Some(ip))
                                .expect("failed to send ARP update to channel");
                        }
                    }
                } else if let Some(sender) = self.senders.get(&latest) {
                    sender
                        .send(Some(ip))
                        .expect("failed to send ARP update to channel");
                }
            } else if let Some(previous) = cache.remove(&ip)
                && let Some(sender) = self.senders.get(&previous)
            {
                sender
                    .send(None)
                    .expect("failed to send ARP update to channel");
            }
        }
    }
}

/// Construct a sender/receiver channel from an interface
fn build_eth_channel(
    interface: &NetworkInterface,
) -> (Box<dyn DataLinkSender>, Box<dyn DataLinkReceiver>) {
    let cfg = pnet::datalink::Config::default();
    match pnet::datalink::channel(interface, cfg) {
        Ok(Channel::Ethernet(tx, rx)) => (tx, rx),
        Ok(_) => panic!("Unknown channel type"),
        Err(e) => panic!("Channel error: {e}"),
    }
}

/// Simple error types for this demo
#[derive(Debug, Error)]
pub enum Error {
    #[error("Interface {0} not found")]
    /// Interface of this name did not exist
    InterfaceNotFound(String),
}
