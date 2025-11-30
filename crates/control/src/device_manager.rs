//! Defines the Manager and associated types

use std::any::Any;
use thiserror::Error;
use tokio_util::sync::CancellationToken;

/// A [Device] manager, this can be used to handle all devices of a certain type,
/// for example, the manager might communicate with an external server which manages the devices
/// directly
///
/// If a device does not need a manager, then it should use `()` as its manager
#[allow(private_bounds, reason = "This is a awful ext trait to use 'dyn DeviceManager' as 'dyn Any', there is no need to expose it")]
pub trait DeviceManager: Any + DeviceManagerExt + Send {
    /// Starts this manager, spawn any tasks in the tokio runtime
    fn start(self: Box<Self>, token: CancellationToken);
}

pub(crate) trait DeviceManagerExt: Any {
    fn as_any(&self) -> &dyn Any;
}

impl<T: DeviceManager + Sized> DeviceManagerExt for T {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Dummy device manager for unmanaged devices
impl DeviceManager for () {
    fn start(self: Box<Self>, _: CancellationToken) {}
}

/// This error occurs when a device manager is not found in the manager
#[derive(Debug, Error)]
#[error("Manager not registered")]
pub struct DeviceManagerNotFound;
