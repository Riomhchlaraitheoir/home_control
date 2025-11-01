# ARP

[Address Resolution Protocol](https://en.wikipedia.org/wiki/Address_Resolution_Protocol) is a level 2 networking 
protocol designed to allow the discovery if the MAC address for a given IPv4 address. Note that IPv6 is not supported,
IPv6 uses [NDP](https://en.wikipedia.org/wiki/Neighbor_Discovery_Protocol) instead 

This integration uses it somewhat differently, though not without precedent. This integration adds ARP scanning.
This is the practice of broadcasting over the LAN asking for the mac address for each IP address in a certain range.

This means that a single ARP packet is broadcast for each IP address in the range, each device on the LAN with an IP
address in the correct range will then respond with the IP+MAC.

The intention of this integration is to detect when a given device is connected to the LAN, therefore it does not need
to broadcast ARP packets since it already knows the MAC address, so it sends only to the relevant MAC. When a device is
connected this integration periodically sends a single ARP to confirm the device is still connected, when the device is
offline the scanner starts scanning more broadly to detect when it comes online again

To use this simple create a `arp::ArpDevice`