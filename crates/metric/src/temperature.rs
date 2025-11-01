//! Temperature related metrics


/// A commonly used Temperature unit for human readability
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum TemperatureUnit {
    /// Degrees Celsius, the normal metric unit for temperature
    Celsius,
    /// Degrees Fahrenheit, the outdated imperial unit that many devices still support because
    /// americans are not allowed to learn new things
    Fahrenheit
}