use macros::zigbee_device;
use metric::temperature::TemperatureUnit;

zigbee_device! {
    pub DoorSensor {
        get "battery" => u8<0, 100>,
        get "valtage" => u32,
        stream "contact" => bool,
        stream "battery_low" => bool,
    }
}

zigbee_device! {
    pub WirelessButton {
        get "battery" => u8<0, 100>,
        get "voltage" => u32,
        stream "action" => enum ButtonAction {
            "single" => Single,
            "double" => Double,
            "long" => Long,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum ButtonAction { Single, Double, Long }

// https://www.zigbee2mqtt.io/devices/SNZB-02D.html
zigbee_device! {
    pub TemperatureAndHumiditySensor {
        get "battery" => u8<0, 100>,
        get "temperature" => i32,
        get "humidity" => u8<0, 100>,
        get set "comfort_temperature_min" => i8<-10, 60>,
        get set "comfort_temperature_max" => i8<-10, 60>,
        get set "comfort_humidity_min" => u8<5, 95>,
        get set "comfort_humidity_max" => u8<5, 95>,
        get set "temperature_units" => temp_display_unit: enum TemperatureUnit {
            "celsius" => Celsius,
            "fahrenheit" => Fahrenheit
        },
        get set "temperature_calibration" => i8<-50, 50>,
        get set "humidity_calibration" => i8<-50, 50>,
    }
}
