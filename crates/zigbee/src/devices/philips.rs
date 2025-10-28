use control::{ButtonEvent, SwitchState};
use macros::zigbee_device;

zigbee_device!{
    pub HueSmartButton {
        stream "action" => events: enum ButtonEvent {
            "press" => Press,
            "hold" => Hold,
            "release" => Release,
        },
        stream "action" => switch: enum SwitchState {
            "on" => On,
            "off" => Off,
        }
    }
}

zigbee_device!{
    pub Light {
        get set toggle "state" => enum SwitchState {
            "ON" => On,
            "OFF" => Off,
        },
        get set "brightness" => u8<0, 254>
    }
}
