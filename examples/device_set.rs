#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, reason = "Panics are forgivable while testing")]
//! An example of the DeviceSet derive macro and it's syntax.
//!
//! This macro allows for devices to be defined in a unified location for easy reference

use control::Manager;
use tintean::arp::{ArpDevice, MacAddr};
use macros::DeviceSet;
use rumqttc::MqttOptions;
use std::net::Ipv4Addr;
use std::time::Duration;
use derive_more::Display;
use zigbee::devices::aqara::{RollerShadeDriver, SmartWallSwitchSingle, WaterLeakSensor};
use zigbee::devices::aurora::DoubleWallSocketTypeG;
use zigbee::devices::philips::{HueSmartButton, Light};
use zigbee::devices::sonoff::{ContactSensor, TemperatureAndHumiditySensor, WirelessButton};

#[derive(Display)]
enum Room {
    #[display("Office")]
    Office,
    #[display("Master bedroom")]
    Bedroom,
    #[display("Landing")]
    Landing,
    #[display("Guest bedroom")]
    Guest,
    #[display("Bathroom")]
    Bathroom,
    #[display("Hallway")]
    Hallway,
    #[display("Living room")]
    Living,
    #[display("Toilet")]
    Toilet,
    #[display("Kitchen")]
    Kitchen,
    #[display("Utility")]
    Utility,
    #[display("Outside")]
    Outside
}

#[allow(dead_code)]
#[derive(DeviceSet)]
struct Devices {
    /// The light in the office
    #[device(tags = {
        room = Room::Office
    })]
    office_light: Light,

    #[device(tags = {
        room = Room::Office
    })]
    test_button: HueSmartButton,

    #[device(tags = {
        room = Room::Office
    })]
    office_button: WirelessButton,

    #[device(tags = {
        room = Room::Landing
    })]
    upstairs_thermostat: TemperatureAndHumiditySensor,

    #[device(tags = {
        room = Room::Kitchen
    })]
    back_door: ContactSensor,

    #[device(tags = {
        room = Room::Hallway
    })]
    front_door: ContactSensor,

    #[device(tags = {
        room = Room::Outside
    })]
    back_yard_light: SmartWallSwitchSingle,

    #[device(tags = {
        room = Room::Bedroom
    })]
    bedroom_button: HueSmartButton,

    #[device(tags = {
        room = Room::Bedroom
    })]
    bedroom_shades: RollerShadeDriver,

    #[device(
        ip = Ipv4Addr::new(192,168,1,62),
        tags = {
            room = Room::Bedroom
        }
    )]
    bedroom_light: wiz::Light,

    #[device(tags = {
        room = Room::Hallway
    })]
    downstairs_thermostat: TemperatureAndHumiditySensor,

    #[device(tags = {
        room = Room::Hallway
    })]
    entrance_light: Light,

    #[device(tags = {
        room = Room::Hallway
    })]
    front_door_button: HueSmartButton,

    #[device(tags = {
        room = Room::Hallway
    })]
    guest_bedroom_light: Light,

    #[device(tags = {
        room = Room::Guest
    })]
    guest_room_button: HueSmartButton,

    #[device(tags = {
        room = Room::Kitchen
    })]
    leak_sensor: WaterLeakSensor,

    #[device(tags = {
        room = Room::Kitchen
    })]
    kitchen_button: HueSmartButton,

    #[device(tags = {
        room = Room::Kitchen
    })]
    kitchen_light_north: Light,

    #[device(tags = {
        room = Room::Kitchen
    })]
    kitchen_light_south: Light,

    #[device(tags = {
        room = Room::Living
    })]
    living_room_button: HueSmartButton,

    #[device(ip = Ipv4Addr::new(192,168,1,61))]
    #[device(tags = {
        room = Room::Living
    })]
    living_room_light: wiz::Light,

    #[device(tags = {
        room = Room::Bathroom
    })]
    main_bathroom_light: Light,

    #[device(tags = {
        room = Room::Hallway
    })]
    stairs_button: HueSmartButton,

    #[device(tags = {
        room = Room::Hallway
    })]
    toilet_button: HueSmartButton,

    #[device(tags = {
        room = Room::Toilet
    })]
    toilet_light: Light,

    #[device(tags = {
        room = Room::Landing
    })]
    upstairs_hallway_button: HueSmartButton,

    #[device(tags = {
        room = Room::Landing
    })]
    upstairs_hallway_light: Light,

    #[device(tags = {
        room = Room::Landing
    })]
    upstairs_hallway_sockets: DoubleWallSocketTypeG,

    #[device(tags = {
        room = Room::Utility
    })]
    utility_light: Light,

    #[device(
        timeout = Duration::from_secs(2),
        confirm_interval = Duration::from_secs(30),
        scan_interval = Duration::from_secs(10),
        ip_range = Ipv4Addr::new(192,168,1,1)..Ipv4Addr::new(192,168,1,255),
        device = MacAddr(0xe8, 0x78, 0x29, 0xc5, 0xaf, 0x6f),
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
