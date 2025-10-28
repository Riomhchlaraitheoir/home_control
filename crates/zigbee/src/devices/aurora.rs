use control::{ReadValue, Sensor, SwitchState, ToggleValue};
use macros::zigbee_device;

// https://www.zigbee2mqtt.io/devices/AU-A1ZBDSS.html
zigbee_device! {
    pub DoubleWallSocketTypeG {
        get set toggle "state_left" => enum SwitchState {
            "ON" => On,
            "OFF" => Off
        },
        get set toggle "state_right" => enum SwitchState {
            "ON" => On,
            "OFF" => Off
        },
        stream "power_left" => u32,
        stream "power_right" => u32,
        get set "brightness" => led_brightness: u8<0, 254>
    }
}

impl DoubleWallSocketTypeG {
    fn left(&self) -> impl WallSocket<'_> {
        WallSocketTypeG {
            state: self.state_left(),
            power: self.power_left(),
        }
    }

    fn right(&self) -> impl WallSocket<'_> {
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

pub trait WallSocket<'a> {
    fn state(&self) -> &'a (impl ReadValue<Item = SwitchState> + ToggleValue<Item = SwitchState>);
    fn power(&self) -> &'a impl Sensor<Item = u32>;
}