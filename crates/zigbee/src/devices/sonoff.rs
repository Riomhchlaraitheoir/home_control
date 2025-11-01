use macros::zigbee_device;
use metric::temperature::TemperatureUnit;

zigbee_device! {
    /// A Door/window contact sensor
    pub ContactSensor {
        "https://www.zigbee2mqtt.io/devices/SNZB-04.html",
        /// Battery level of the sensor as a percentage
        get "battery" => u8<0, 100>,
        /// Battery voltage in mV
        get "voltage" => u32,
        /// true if the contact sensor is in contact
        stream "contact" => bool,
        /// true if the battery is almost empty
        stream "battery_low" => bool,
    }
}

zigbee_device! {
    /// Wireless Button
    pub WirelessButton {
        "https://www.zigbee2mqtt.io/devices/SNZB-01.html",
        /// Battery level as a percentage
        get "battery" => u8<0, 100>,
        /// Battery voltage in mV
        get "voltage" => u32,
        /// detected action from the button
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
        "https://www.zigbee2mqtt.io/devices/SNZB-02D.html",
        /// Battery level as a percentage
        get "battery" => u8<0, 100>,
        /// measured temperature in Celsius
        get "temperature" => i32,
        /// measured humidity as a percentage
        get "humidity" => u8<0, 100>,
        /// minimum temperature that is considered comfortable
        get set "comfort_temperature_min" => i8<-10, 60>,
        /// maximum temperature that is considered comfortable
        get set "comfort_temperature_max" => i8<-10, 60>,
        /// minimum humidity that is considered comfortable
        get set "comfort_humidity_min" => u8<5, 95>,
        /// maximum humidity that is considered comfortable
        get set "comfort_humidity_max" => u8<5, 95>,
        /// Display unit for the temperature
        get set "temperature_units" => temp_display_unit: enum TemperatureUnit {
            "celsius" => Celsius,
            "fahrenheit" => Fahrenheit
        },
        /// Offset to calibrate the reported temperature
        get set "temperature_calibration" => i8<-50, 50>,
        /// Offset to calibrate the reported humidity
        get set "humidity_calibration" => i8<-50, 50>,
    }
}
