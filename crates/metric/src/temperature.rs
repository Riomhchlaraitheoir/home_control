//! Temperature related metrics

use derive_more::Display;
use control::reflect::enum_value;

/// A commonly used Temperature unit for human readability
#[derive(Debug, Clone, Copy, Eq, PartialEq, Display)]
pub enum TemperatureUnit {
    /// Degrees Celsius, the normal metric unit for temperature
    #[display("celsius")]
    Celsius,
    /// Degrees Fahrenheit, the outdated imperial unit that many devices still support because
    /// americans are not allowed to learn new things
    #[display("fahrenheit")]
    Fahrenheit
}

enum_value!(TemperatureUnit,
    "celsius" => Celsius,
    "fahrenheit" => Fahrenheit
);