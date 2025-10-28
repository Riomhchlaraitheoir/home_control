use control::SwitchState;
use macros::zigbee_device;

// https://www.zigbee2mqtt.io/devices/QBKG04LM.html
zigbee_device! {
    pub SmartWallSwitchSingle {
        get set toggle "switch" => enum SwitchState {
            "ON" => On,
            "OFF" => Off,
        },
        stream set "operation_mode" => enum OperationMode {
            "control_relay" => ControlRelay,
            "decoupled" => Decoupled,
        },
        stream "action" => enum Action {
            "release" => Release,
            "hold" => Hold,
            "double" => Double,
            "single" => Single,
            "hold_release" => HoldRelease,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Action {
    Release,
    Hold,
    Double,
    Single,
    HoldRelease
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum OperationMode {
    ControlRelay,
    Decoupled
}

zigbee_device! {
    pub RollerShadeDriver {
        get "state" => enum RollerShadeDriverState {
            "OPEN" => Open,
            "CLOSE" => Close
        },
        set "command" => enum RollerShadeDriverStateCommand {
            "OPEN" => Open,
            "CLOSE" => Close,
            "STOP" => Stop
        },
        get "battery" => u8<0, 100>,
        stream "device_temperature" => temperature: i32,
        get "charging_status" => bool,
        stream "motor_state" => enum RollerShadeDriverMotorState {
            "closing" => Closing,
            "opening" => Opening,
            "stopped" => Stopped,
            "blocked" => Blocked,
        },
        stream "running" => bool,
        get set "motor_speed" => enum RollerShadeDriverMotorSpeed {
            "low" => Low,
            "medium" => Medium,
            "high" => High,
        }
    }
}


#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum RollerShadeDriverState { Open, Close }

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum RollerShadeDriverStateCommand { Open, Close, Stop }

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum RollerShadeDriverMotorState { Closing, Opening, Stopped, Blocked }

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum RollerShadeDriverMotorSpeed { Low, Medium, High }


zigbee_device! {
    pub WaterLeakSensor {
        stream "battery" => u8<0, 100>,
        stream "voltage" => u32,
        stream "device_temperature" => i32,
        stream "power_outage_count" => u32,
        stream "trigger_count" => u32,
        stream "water_leak" => bool,
        stream "battery_low" => bool,
    }
}