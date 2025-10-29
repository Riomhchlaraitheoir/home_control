use derive_more::Deref;
use pnet::datalink::{Channel, DataLinkReceiver, DataLinkSender, NetworkInterface};
use pnet::packet::arp::{ArpHardwareTypes, ArpOperations, ArpPacket, MutableArpPacket};
use pnet::packet::ethernet::{EtherTypes, EthernetPacket, MutableEthernetPacket};
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};
use std::ops::Range;
use std::thread::sleep;
use std::time::{Duration, Instant};
use bon::bon;
use futures::{Stream, StreamExt};
use thiserror::Error;
use tokio::sync::watch::{Receiver, Sender, channel};
use tokio_stream::wrappers::WatchStream;
use tracing::{debug, debug_span, trace};
use control::{Device, ExposesSubManager};

pub use pnet::util::MacAddr;

#[derive(Debug)]
pub struct NetworkScannerConfig {
    /// The name of the target device (to be included in logs)
    pub name: String,
    /// The name of the network interface to use
    pub interface_name: Option<String>,
    /// the length of time to wait before deeming the device offline
    pub timeout: Duration,
    /// the length of time to wait before confirming that a device is still online
    pub confirm_interval: Duration,
    /// the length of time to wait before scanning for an offline device
    pub scan_interval: Duration,
    /// The range of IP addresses to check
    pub ip_range: Range<Ipv4Addr>,
    /// The device to scan for
    pub device: MacAddr,
}

#[derive(Default)]
pub struct ArpManager {
    scanners: Vec<ArpScanner>
}

impl ArpManager {
    pub fn run(self) {
        for scanner in self.scanners {
            std::thread::spawn(|| scanner.run());
        }
    }
}

#[derive(Debug, Deref)]
pub struct ArpScanner {
    #[deref]
    config: NetworkScannerConfig,
    interface: NetworkInterface,
    sender: Sender<Option<Ipv4Addr>>,
    local: (MacAddr, Ipv4Addr),
}

pub struct ArpDevice(Receiver<Option<Ipv4Addr>>);

#[bon]
impl ArpDevice {
    #[builder]
    pub fn create(
        manager: &mut impl ExposesSubManager<ArpManager>,
        name: String,
        /// The name of the network interface to use
        interface_name: Option<String>,
        /// the length of time to wait before deeming the device offline
        timeout: Duration,
        /// the length of time to wait before confirming that a device is still online
        confirm_interval: Duration,
        /// the length of time to wait before scanning for an offline device
        scan_interval: Duration,
        /// The range of IP addresses to check
        ip_range: Range<Ipv4Addr>,
        /// The device to scan for
        device: MacAddr,
    ) -> Result<Self, Error> {
        Self::new(manager.exclusive(), NetworkScannerConfig {
            name,
            interface_name,
            timeout,
            confirm_interval,
            scan_interval,
            ip_range,
            device,
        })
    }

    pub fn ip_addr(&self) -> impl Stream<Item = Option<Ipv4Addr>> {
        WatchStream::from_changes(self.0.clone())
    }

    pub fn online(&self) -> impl Stream<Item = bool> {
        self.ip_addr().map(|ip| ip.is_some())
    }
}

impl Device for ArpDevice {
    type Args = NetworkScannerConfig;
    type Manager = ArpManager;
    type Error = Error;

    fn new(manager: &mut Self::Manager, config: NetworkScannerConfig) -> Result<Self, Error> {
        let (scanner, receiver) = ArpScanner::new(config)?;
        manager.scanners.push(scanner);
        Ok(ArpDevice(receiver))
    }
}

struct State {
    sender: Box<dyn DataLinkSender>,
    receiver: Box<dyn DataLinkReceiver>,
    template: ArpTemplate,
}

impl ArpScanner {
    pub fn new(config: NetworkScannerConfig) -> Result<(Self, Receiver<Option<Ipv4Addr>>), Error> {
        let interface = pnet::datalink::interfaces()
            .into_iter()
            .find(|i| config.interface_name.as_ref().is_none_or(|name| name == &i.name) && !i.is_loopback())
            .ok_or_else(|| Error::InterfaceNotFound(config.interface_name.clone()))?;
        let local_ipv4 = interface
            .ips
            .iter()
            .find_map(|ip| match ip.ip() {
                IpAddr::V4(addr) => Some(addr),
                IpAddr::V6(_) => None,
            })
            .unwrap();

        let (sender, receiver) = channel(None);

        let local_mac = interface.mac.unwrap();
        Ok((
            Self {
                config,
                interface,
                sender,
                local: (local_mac, local_ipv4),
            },
            receiver,
        ))
    }

    pub fn run(self) -> ! {
        debug_span!(target: "arp", "ARP scanner running for device: {}", self.name);
        let (sender, receiver) = build_eth_channel(&self.interface);
        let (source_mac, source_ip) = self.local;
        let template = ArpTemplate::new(source_mac, source_ip);
        let mut state = State {
            sender,
            receiver,
            template,
        };
        debug!("Beginning device loop");
        let mut current_ip = None;
        let mac = self.device;
        loop {
            trace!("Checking {}", mac);
            if let Some(ip) = current_ip {
                debug!("confirming IP: {ip}");
                if !self.confirm_ip(&mut state, mac, ip) {
                    debug!("IP outdated");
                    current_ip = None;
                } else {
                    debug!("IP confirmed")
                }
            }
            if current_ip.is_none() {
                debug!("Scanning for new IP");
                current_ip = self.get_ip_for(&mut state, mac);
                if current_ip.is_none() {
                    trace!("Device is offline")
                }
            }
            self.sender
                .send(current_ip)
                .expect("failed to send ARP update to channel");

            if current_ip.is_none() {
                sleep(self.scan_interval);
            } else {
                sleep(self.confirm_interval);
            }
        }
    }

    #[allow(dead_code)] // not currently used, but may still be a useful example
    fn all_current_ips(&self, state: &mut State, ip_by_mac: &mut HashMap<MacAddr, Ipv4Addr>) {
        ip_by_mac.clear();
        let (source_mac, source_ip) = self.local;
        let start = Instant::now();
        for ip in self.ip_range.clone() {
            if ip == source_ip {
                // do not check this machine's IP, that would be silly
                continue;
            }
            let pkt = state.template.execute(ip, MacAddr::broadcast());
            state.sender.send_to(pkt, None).unwrap().unwrap();
        }

        loop {
            let buf = state.receiver.next().unwrap();

            if buf.len() < EthernetPacket::minimum_packet_size() + ArpPacket::minimum_packet_size()
            {
                if Instant::now().duration_since(start) > self.timeout {
                    break;
                }
                continue;
            }

            let pkt_arp = ArpPacket::new(&buf[EthernetPacket::minimum_packet_size()..]).unwrap();

            if pkt_arp.get_target_hw_addr() == source_mac {
                let ip = pkt_arp.get_sender_proto_addr();
                let mac = pkt_arp.get_sender_hw_addr();
                ip_by_mac.insert(mac, ip);
            }

            if Instant::now().duration_since(start) > self.timeout {
                break;
            }
        }
    }

    fn get_ip_for(&self, state: &mut State, mac: MacAddr) -> Option<Ipv4Addr> {
        for ip in self.ip_range.clone() {
            if ip == self.local.1 {
                // do not check this machine's IP, that would be silly
                continue;
            }
            let pkt = state.template.execute(ip, mac);
            state.sender.send_to(pkt, None).unwrap().unwrap();
        }
        let start = Instant::now();

        loop {
            let buf = state.receiver.next().unwrap();

            if buf.len() < EthernetPacket::minimum_packet_size() + ArpPacket::minimum_packet_size()
            {
                if Instant::now().duration_since(start) > self.timeout {
                    break None;
                }
                continue;
            }

            let pkt_arp = ArpPacket::new(&buf[EthernetPacket::minimum_packet_size()..]).unwrap();

            if pkt_arp.get_target_hw_addr() == self.local.0 && pkt_arp.get_sender_hw_addr() == mac {
                break Some(pkt_arp.get_sender_proto_addr());
            }

            if Instant::now().duration_since(start) > self.timeout {
                break None;
            }
        }
    }

    fn confirm_ip(&self, state: &mut State, mac: MacAddr, ip: Ipv4Addr) -> bool {
        let pkt = state.template.execute(ip, mac);
        state.sender.send_to(pkt, None).unwrap().unwrap();
        let start = Instant::now();

        loop {
            let buf = state.receiver.next().unwrap();

            if buf.len() < EthernetPacket::minimum_packet_size() + ArpPacket::minimum_packet_size()
            {
                if Instant::now().duration_since(start) > self.timeout {
                    break false;
                }
                continue;
            }

            let pkt_arp = ArpPacket::new(&buf[EthernetPacket::minimum_packet_size()..]).unwrap();

            if pkt_arp.get_target_hw_addr() == self.local.0
                && pkt_arp.get_sender_hw_addr() == mac
                && pkt_arp.get_sender_proto_addr() == ip
            {
                break true;
            }

            if Instant::now().duration_since(start) > self.timeout {
                break false;
            }
        }
    }
}

const COMBINED_PACKET_SIZE: usize =
    EthernetPacket::minimum_packet_size() + ArpPacket::minimum_packet_size();

struct ArpTemplate([u8; COMBINED_PACKET_SIZE]);

impl ArpTemplate {
    fn new(source_mac: MacAddr, source_ip: Ipv4Addr) -> Self {
        let mut pkt_buf = [0u8; COMBINED_PACKET_SIZE];

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
            pkt_arp.set_sender_proto_addr(source_ip);
        }
        Self(pkt_buf)
    }

    fn execute(&mut self, ip: Ipv4Addr, mac: MacAddr) -> &[u8] {
        {
            // Build our base ethernet frame
            let mut pkt_eth = MutableEthernetPacket::new(&mut self.0).unwrap();

            pkt_eth.set_destination(mac);
        }
        {
            let mut pkt_arp =
                MutableArpPacket::new(&mut self.0[EthernetPacket::minimum_packet_size()..])
                    .unwrap();
            pkt_arp.set_target_proto_addr(ip);
            pkt_arp.set_target_hw_addr(mac)
        }
        &self.0
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
    #[error("Interface {0:?} not found")]
    /// Interface of this name did not exist
    InterfaceNotFound(Option<String>),
}
