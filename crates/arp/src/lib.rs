#![doc= include_str!("../README.md")]

use bon::bon;
use derive_more::Deref;
use futures::{Stream, StreamExt};
use pnet::datalink::{Channel, DataLinkReceiver, DataLinkSender, NetworkInterface};
use pnet::packet::arp::{ArpHardwareTypes, ArpOperations, ArpPacket, MutableArpPacket};
use pnet::packet::ethernet::{EtherTypes, EthernetPacket, MutableEthernetPacket};
use std::collections::HashMap;
use std::io;
use std::net::{IpAddr, Ipv4Addr};
use std::ops::Range;
use std::thread::sleep;
use std::time::{Duration, Instant};
use tokio::sync::watch::{Receiver, Sender, channel};
use tokio_stream::wrappers::WatchStream;
use tracing::{debug, debug_span, error, trace};

use control::device::Device;
use control::manager::DeviceManager;
pub use pnet::util::MacAddr;
use thiserror::Error;
use tokio::spawn;
use tokio::task::spawn_blocking;
use tokio_util::sync::CancellationToken;

/// The configuration data for a ARP network scanner
#[derive(Debug)]
pub struct NetworkScannerConfig {
    /// The name of the target device (to be included in logs)
    pub name: String,
    /// The name of the network interface to use
    pub interface_name: Option<String>,
    /// the length of time to wait for an ARP reply before deeming the device offline
    pub timeout: Duration,
    /// the interval between each confirmation that a device is still online
    pub confirm_interval: Duration,
    /// the interval between each scan for the device while it is offline
    pub scan_interval: Duration,
    /// The range of IP addresses to check
    pub ip_range: Range<Ipv4Addr>,
    /// The device to scan for
    pub device: MacAddr,
}

/// A manager of ARP scanners. Collects created scanners until ready to begin scanning
#[derive(Default)]
pub struct ArpManager {
    scanners: Vec<ArpScanner>,
}

impl DeviceManager for ArpManager {
    fn start(self: Box<Self>, token: CancellationToken) {
        spawn(self.run(token));
    }
}

impl ArpManager {
    /// Create a new manager
    pub fn new() -> Self {
        Self::default()
    }

    /// Run all scanners
    pub async fn run(self, token: CancellationToken) {
        let handles = self
            .scanners
            .into_iter()
            .map(|scanner| spawn_blocking(|| scanner.run()))
            .collect::<Vec<_>>();
        token.cancelled().await;
        for handle in handles {
            handle.abort();
        }
    }
}

/// The ARP scanner, separate from the ARP device, this is the part that performs the actual
/// scanning
#[derive(Debug, Deref)]
pub struct ArpScanner {
    #[deref]
    config: NetworkScannerConfig,
    interface: NetworkInterface,
    sender: Sender<Option<Ipv4Addr>>,
    local: (MacAddr, Ipv4Addr),
}

/// An ARP device, this represents a watched device and exposes some methods for getting current
/// status and listening for changes
pub struct ArpDevice(Receiver<Option<Ipv4Addr>>);

#[bon]
impl ArpDevice {
    #[allow(
        missing_docs,
        reason = "This item is hidden since it's only intended for use in macros"
    )]
    #[doc(hidden)]
    #[builder]
    pub async fn create(
        manager: &mut ArpManager,
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
    ) -> anyhow::Result<Self> {
        Self::new(
            manager,
            NetworkScannerConfig {
                name,
                interface_name,
                timeout,
                confirm_interval,
                scan_interval,
                ip_range,
                device,
            },
        )
        .await
    }

    /// Returns the IP address of the device if it is connected to the network, and None otherwise
    pub fn ip_addr(&self) -> Option<Ipv4Addr> {
        *self.0.borrow()
    }

    /// Returns true if the device is currently connected to the network
    pub fn online(&self) -> bool {
        self.0.borrow().is_some()
    }

    /// Returns a stream of updates from the scanner, if the value is `None`, that implies that
    /// the device is not connected to the network,. otherwise when the value is `Some(ip_addr)`
    /// it means that the device is connected and has the given IP address
    pub fn ip_addr_changes(&self) -> impl Stream<Item = Option<Ipv4Addr>> {
        WatchStream::from_changes(self.0.clone())
    }

    /// Returns a stream of changes to the online status of the device
    pub fn online_changes(&self) -> impl Stream<Item = bool> {
        self.ip_addr_changes().map(|ip| ip.is_some())
    }
}

impl Device for ArpDevice {
    type Args = NetworkScannerConfig;
    type Manager = ArpManager;

    async fn new(
        manager: &mut Self::Manager,
        config: NetworkScannerConfig,
    ) -> anyhow::Result<Self> {
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
    fn new(config: NetworkScannerConfig) -> Result<(Self, Receiver<Option<Ipv4Addr>>), Error> {
        let interface = pnet::datalink::interfaces()
            .into_iter()
            .find(|i| {
                config
                    .interface_name
                    .as_ref()
                    .is_none_or(|name| name == &i.name)
                    && !i.is_loopback()
            })
            .ok_or_else(|| Error::InterfaceNotFound(config.interface_name.clone()))?;
        let local_ipv4 = interface
            .ips
            .iter()
            .find_map(|ip| match ip.ip() {
                IpAddr::V4(addr) => Some(addr),
                IpAddr::V6(_) => None,
            })
            .ok_or(Error::IPv4NotSupported)?;

        let (sender, receiver) = channel(None);

        let local_mac = interface.mac.ok_or(Error::NoMacAddr)?;
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

    /// Runs the ARP scanner on this thread, will never return.
    ///
    /// keeps scanning forever sleeping the thread between scans, updates are communicated to the
    /// `ArpDevice` using a channel
    fn run(self) {
        debug_span!(target: "arp", "ARP scanner running for device: {}", self.name);
        let (sender, receiver) = match build_eth_channel(&self.interface) {
            Ok(channel) => channel,
            Err(error) => {
                error!("Error sending ARP frame: {error}");
                return;
            }
        };
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
            let result = self.sender.send(current_ip);
            if let Err(error) = result {
                error!("Error sending ARP IP: {}", error);
            }

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
            match state.sender.send_to(pkt, None) {
                Some(Ok(())) => {}
                None => {
                    error!("Error sending ARP to IP: {ip}");
                }
                Some(Err(error)) => {
                    error!("Error sending ARP to IP: {ip}; {error}");
                }
            }
        }

        loop {
            let buf = match state.receiver.next() {
                Ok(buf) => buf,
                Err(error) => {
                    error!("failed to read frame from interface: {error}");
                    continue;
                }
            };

            if buf.len() < EthernetPacket::minimum_packet_size() + ArpPacket::minimum_packet_size()
            {
                if Instant::now().duration_since(start) > self.timeout {
                    break;
                }
                continue;
            }

            let Some(pkt_arp) = ArpPacket::new(&buf[EthernetPacket::minimum_packet_size()..])
            else {
                error!("Buffer not large enough for ARP frame");
                continue;
            };

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
            match state.sender.send_to(pkt, None) {
                Some(Ok(())) => {}
                Some(Err(error)) => {
                    error!("Error sending ARP frame: {error}");
                    continue;
                }
                None => {
                    error!("Error sending ARP frame");
                    continue;
                }
            };
        }
        let start = Instant::now();

        loop {
            let buf = match state.receiver.next() {
                Ok(buf) => buf,
                Err(error) => {
                    error!("Error receiving ARP frame: {error}");
                    continue;
                }
            };

            if buf.len() < EthernetPacket::minimum_packet_size() + ArpPacket::minimum_packet_size()
            {
                if Instant::now().duration_since(start) > self.timeout {
                    break None;
                }
                continue;
            }

            let Some(pkt_arp) = ArpPacket::new(&buf[EthernetPacket::minimum_packet_size()..])
            else {
                error!("Buffer not large enough for ARP frame");
                continue;
            };

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
        match state.sender.send_to(pkt, None) {
            Some(Ok(())) => {}
            Some(Err(error)) => {
                error!("Error sending ARP frame: {error}");
                return false;
            }
            None => {
                error!("Error sending ARP frame");
                return false;
            }
        };
        let start = Instant::now();

        loop {
            let buf = match state.receiver.next() {
                Ok(buf) => buf,
                Err(error) => {
                    error!("Error receiving ARP frame: {error}");
                    continue;
                }
            };

            if buf.len() < EthernetPacket::minimum_packet_size() + ArpPacket::minimum_packet_size()
            {
                if Instant::now().duration_since(start) > self.timeout {
                    break false;
                }
                continue;
            }

            let Some(pkt_arp) = ArpPacket::new(&buf[EthernetPacket::minimum_packet_size()..])
            else {
                error!("Buffer not large enough for ARP frame");
                continue;
            };

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
            #[allow(clippy::expect_used)]
            let mut pkt_eth = MutableEthernetPacket::new(&mut pkt_buf)
                .expect("buffer is large enough for EthernetPacket");

            pkt_eth.set_destination(MacAddr::broadcast());
            pkt_eth.set_source(source_mac);
            pkt_eth.set_ethertype(EtherTypes::Arp);
        }

        {
            // Build the ARP frame on top of the ethernet frame
            #[allow(clippy::expect_used)]
            let mut pkt_arp =
                MutableArpPacket::new(&mut pkt_buf[EthernetPacket::minimum_packet_size()..])
                    .expect("buffer is large enough for ArpPacket");

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
            #[allow(clippy::expect_used)]
            let mut pkt_eth = MutableEthernetPacket::new(&mut self.0)
                .expect("buffer is large enough for EthernetPacket");

            pkt_eth.set_destination(mac);
        }
        {
            #[allow(clippy::expect_used)]
            let mut pkt_arp =
                MutableArpPacket::new(&mut self.0[EthernetPacket::minimum_packet_size()..])
                    .expect("buffer is large enough for ArpPacket");
            pkt_arp.set_target_proto_addr(ip);
            pkt_arp.set_target_hw_addr(mac)
        }
        &self.0
    }
}

type NetworkChannel = (Box<dyn DataLinkSender>, Box<dyn DataLinkReceiver>);

/// Construct a sender/receiver channel from an interface
fn build_eth_channel(
    interface: &NetworkInterface,
) -> Result<NetworkChannel, io::Error> {
    let cfg = pnet::datalink::Config::default();
    Ok(match pnet::datalink::channel(interface, cfg)? {
        Channel::Ethernet(tx, rx) => (tx, rx),
        _ => unreachable!("Unknown Channel enum variant"),
    })
}

/// Simple error types for this demo
#[derive(Debug, Error)]
pub enum Error {
    #[error("Interface {0:?} not found")]
    /// Interface of this name did not exist
    InterfaceNotFound(Option<String>),
    #[error("interface does not support IPv4")]
    /// No IPv4 address found on interface
    IPv4NotSupported,
    /// No MAC address found for network interface
    #[error("No mac address found for interface")]
    NoMacAddr,
}
