//! Devices and related types

use std::fmt::Debug;
use thiserror::Error;
use crate::manager::{DeviceManager, Manager, DeviceManagerNotFound};

/// This is a set of devices which can be created together using `Manager::create_devices`
///
/// This can be derived for any struct which has a `create` function returning a `bon` builder
/// The create function must have at least two parameters: `name: &'static str` and `manager: &mut Manager`
///
/// This function is used by duck typing (The macro calls the function, resulting in a compile error if the function is not present) rather than using triats
/// This allows additional parameters to be defined in the device as needed rather than being tied to a trait definition
pub trait DeviceSet: Sized {
    /// Create a new device set from the manager
    async fn new(manager: &mut Manager) -> Result<Self, CreateDeviceError>;
}


/// A Device which can be used in the home_control system
pub trait Device: Sized {
    /// Creation args needed to create this device
    type Args;
    /// The manager type that this device needs
    type Manager: DeviceManager;

    /// creates the device
    async fn new(manager: &mut Self::Manager, args: Self::Args) -> anyhow::Result<Self>;
}

/// This error occurs when a device creation failed
#[derive(Debug, Error)]
pub enum CreateDeviceError {
    /// The appropriate device manager was not found
    #[error(transparent)]
    ManagerNotFound(#[from] DeviceManagerNotFound),
    /// The Device creation failed with a device-specific error
    #[error(transparent)]
    Device(#[from] anyhow::Error),
}
