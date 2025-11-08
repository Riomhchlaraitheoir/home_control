use control::{ButtonEvent, SwitchState};
use macros::zigbee_device;

zigbee_device!{
    /// A Philips Hue Smart Button
    pub HueSmartButton {
        "https://www.zigbee2mqtt.io/devices/8718699693985.html",
        /// The button events detected by the button
        stream "action" => events: enum ButtonEvent {
            "press" => Press,
            "hold" => Hold,
            "release" => Release,
        },
        /// The switch events detected by the button.
        ///
        /// This button is able to function as a switch in addition to a simple button
        stream "action" => switch: enum SwitchState {
            "on" => On,
            "off" => Off,
        }
    }
}

zigbee_device!{
    /// Hue white A60 bulb B22 1055lm with Bluetooth
    pub Light {
        "https://www.zigbee2mqtt.io/devices/9290024693.html#philips-9290024693",
        /// The current state of the bulb, on or off
        get set toggle "state" => enum SwitchState {
            "ON" => On,
            "OFF" => Off,
        },
        /// The current brightness of the bulb, expressed as a u8
        get set "brightness" => u8<0, 254>
    }
}
