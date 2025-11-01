use control::SwitchState;
use macros::zigbee_device;

zigbee_device! {
    /// A single rocker smart wall switch
    ///
    /// This switch has both a rocker which can trigger anything and a physical switch
    /// intended to control a non-smart light, these can be coupled on the device
    pub SmartWallSwitchSingle {
        "https://www.zigbee2mqtt.io/devices/QBKG04LM.html",
        /// The state of the physical switch
        get set toggle "switch" => enum SwitchState {
            "ON" => On,
            "OFF" => Off,
        },
        /// operation mode determines if the physical switch is coupled to the rocker or not
        stream set "operation_mode" => enum OperationMode {
            "control_relay" => ControlRelay,
            "decoupled" => Decoupled,
        },
        /// The actions detected by the rocker
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
/// an action from an Aqara rocker switch
pub enum Action {
    /// The button is released
    Release,
    /// The button is held
    Hold,
    /// The button is double pressed
    Double,
    /// The button is pressed
    Single,
    /// The button is released after being held
    HoldRelease
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
/// determines the behaviour of an Aqara rocker switch
pub enum OperationMode {
    /// Relay actions from the rocker to the switch
    ControlRelay,
    /// Do not relay actions, instead both can be used in automations
    Decoupled
}

zigbee_device! {
    /// Aqara Roller Shade Driver E1
    ///
    /// A motorised driver for roller based blinds/shades
    pub RollerShadeDriver {
        "https://www.zigbee2mqtt.io/devices/ZNJLBL01LM.html",
        /// The current state of the blinds
        get "state" => enum RollerShadeDriverState {
            "OPEN" => Open,
            "CLOSE" => Close
        },
        /// A command to be sent to the device
        set "command" => enum RollerShadeDriverStateCommand {
            "OPEN" => Open,
            "CLOSE" => Close,
            "STOP" => Stop
        },
        /// the current battery level of the device as a percentage
        get "battery" => u8<0, 100>,
        /// the current device temperature in celsius
        stream "device_temperature" => temperature: i32,
        /// true if the device is currently charging
        get "charging_status" => bool,
        /// the current motor state
        stream "motor_state" => enum RollerShadeDriverMotorState {
            "closing" => Closing,
            "opening" => Opening,
            "stopped" => Stopped,
            "blocked" => Blocked,
        },
        /// true if the motor is currently running
        stream "running" => bool,
        /// the motor's speed
        get set "motor_speed" => enum RollerShadeDriverMotorSpeed {
            "low" => Low,
            "medium" => Medium,
            "high" => High,
        }
    }
}


/// Represents the operational state of a roller shade driver.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum RollerShadeDriverState {
    /// The blinds are open
    Open,
    /// The blinds are closed
    Close
}

/// Represents the set of commands that can be issued to control a roller shade driver.
///
/// This enum defines the possible states or commands that can be sent to a roller shade device
/// to manipulate its position or activity
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum RollerShadeDriverStateCommand {
    /// open the blinds
    Open,
    /// close the blinds
    Close,
    /// stop the motor
    Stop
}

/// The `RollerShadeDriverMotorState` enum represents the operational states of a motor that controls a roller shade.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum RollerShadeDriverMotorState {
    /// The blinds are closing
    Closing,
    /// The blinds are opening
    Opening,
    /// The blinds are stopped
    Stopped,
    /// The blinds are stopped due to being blocked by something
    Blocked
}

/// An enumeration representing the speed settings for a roller shade driver's motor.
///
/// This enum is used to define the different speed levels at which a roller shade's motor can operate.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum RollerShadeDriverMotorSpeed {
    /// low speed
    Low,
    /// medium speed
    Medium,
    /// high speed
    High
}


zigbee_device! {
    /// Aqara Water leak sensor
    pub WaterLeakSensor {
        "https://www.zigbee2mqtt.io/devices/SJCGQ11LM.html",
        /// Current device battery level as a percentage
        stream "battery" => u8<0, 100>,
        /// battery voltage in mV
        stream "voltage" => u32,
        /// device temperature in Celsius
        stream "device_temperature" => i32,
        /// Number of power outages
        stream "power_outage_count" => u32,
        /// Number of triggers since last report
        stream "trigger_count" => u32,
        /// Indicates if a water leak was detected
        stream "water_leak" => bool,
        /// Indicates that the device battery is almost empty
        stream "battery_low" => bool,
    }
}