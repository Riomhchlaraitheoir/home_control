use control::{ReadValue, Sensor, SwitchState, ToggleValue};
use macros::zigbee_device;

// https://www.zigbee2mqtt.io/devices/AU-A1ZBDSS.html
zigbee_device! {
    /// A Double Type G (UK) wall socket
    pub DoubleWallSocketTypeG {
        "https://www.zigbee2mqtt.io/devices/AU-A1ZBDSS.html",
        /// The state of the left switch
        get set toggle "state_left" => enum SwitchState {
            "ON" => On,
            "OFF" => Off
        },
        /// The state of the right switch
        get set toggle "state_right" => enum SwitchState {
            "ON" => On,
            "OFF" => Off
        },
        /// The power consumption of the left socket
        stream "power_left" => u32,
        /// The power consumption of the right socket
        stream "power_right" => u32,
        /// The LED brightness of the switches
        get set "brightness" => led_brightness: u8<0, 254>
    }
}

impl DoubleWallSocketTypeG {
    /// The left socket
    pub fn left(&self) -> impl WallSocket<'_> {
        WallSocketTypeG {
            state: self.state_left(),
            power: self.power_left(),
        }
    }

    /// The right socket
    pub fn right(&self) -> impl WallSocket<'_> {
        WallSocketTypeG {
            state: self.state_right(),
            power: self.power_right(),
        }
    }
}

struct WallSocketTypeG<'a, State, Power> {
    state: &'a State,
    power: &'a Power,
}

impl<'a, State, Power> WallSocket<'a> for WallSocketTypeG<'a, State, Power>
where
    State: Clone + ReadValue<Item = SwitchState> + ToggleValue<Item = SwitchState>,
    Power: Clone + Sensor<Item = u32>
{
    fn state(&self) -> &'a (impl ReadValue<Item = SwitchState> + ToggleValue<Item = SwitchState>) {
        self.state
    }

    fn power(&self) -> &'a impl Sensor<Item = u32> {
        self.power
    }
}

/// A smart wall socket
pub trait WallSocket<'a> {
    /// The state of the switch
    fn state(&self) -> &'a (impl ReadValue<Item = SwitchState> + ToggleValue<Item = SwitchState>);
    /// The power consumption
    fn power(&self) -> &'a impl Sensor<Item = u32>;
}