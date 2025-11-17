#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, reason = "Panics are forgivable while testing")]
//! An example of the DeviceSet derive macro and it's syntax.
//!
//! This macro allows for devices to be defined in a unified location for easy reference

use control::manager::Manager;
use home_control::arp::{ArpDevice, MacAddr};
use macros::DeviceSet;
use rumqttc::MqttOptions;
use std::net::Ipv4Addr;
use std::time::Duration;
use zigbee::devices::aqara::{RollerShadeDriver, SmartWallSwitchSingle, WaterLeakSensor};
use zigbee::devices::aurora::DoubleWallSocketTypeG;
use zigbee::devices::philips::{HueSmartButton, Light};
use zigbee::devices::sonoff::{ContactSensor, TemperatureAndHumiditySensor, WirelessButton};

#[allow(dead_code)]
#[derive(DeviceSet)]
struct Devices {
    office_light: Light,
    test_button: HueSmartButton,
    office_button: WirelessButton,
    upstairs_thermostat: TemperatureAndHumiditySensor,
    back_door: ContactSensor,
    front_door: ContactSensor,
    back_yard_light: SmartWallSwitchSingle,
    bedroom_button: HueSmartButton,
    bedroom_shades: RollerShadeDriver,
    #[device(ip = Ipv4Addr::new(192,168,1,62))]
    bedroom_light: wiz::Light,
    downstairs_thermostat: TemperatureAndHumiditySensor,
    entrance_light: Light,
    front_door_button: HueSmartButton,
    guest_bedroom_light: Light,
    guest_room_button: HueSmartButton,
    leak_sensor: WaterLeakSensor,
    kitchen_button: HueSmartButton,
    kitchen_light_north: Light,
    kitchen_light_south: Light,
    living_room_button: HueSmartButton,
    #[device(ip = Ipv4Addr::new(192,168,1,61))]
    living_room_light: wiz::Light,
    main_bathroom_light: Light,
    stairs_button: HueSmartButton,
    toilet_button: HueSmartButton,
    toilet_light: Light,
    upstairs_hallway_button: HueSmartButton,
    upstairs_hallway_light: Light,
    upstairs_hallway_sockets: DoubleWallSocketTypeG,
    utility_light: Light,
    #[device(
        timeout = Duration::from_secs(2),
        confirm_interval = Duration::from_secs(30),
        scan_interval = Duration::from_secs(10),
        ip_range = Ipv4Addr::new(192,168,1,1)..Ipv4Addr::new(192,168,1,255),
        device = MacAddr(0xe8, 0x78, 0x29, 0xc5, 0xaf, 0x6f)
    )]
    dylan_phone: ArpDevice,
}

#[tokio::main]
async fn main() {
    let mut manager = Manager::builder()
        .add_device_manager(
            zigbee::Manager::builder()
                .mqtt_options(MqttOptions::new("test", "localhost", 1883))
                .build(),
        )
        .add_device_manager(arp::ArpManager::new())
        .build();
    let _devices: Devices = manager.create().await.expect("failed to create devices");
}
