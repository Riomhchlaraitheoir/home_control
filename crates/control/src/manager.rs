//! Defines the Manager and associated types

use crate::automation::Automation;
use crate::device::{CreateDeviceError, Device, DeviceSet};
use async_scoped::TokioScope;
use bon::bon;
use futures::executor::block_on_stream;
use futures::future::ready;
use futures::stream::select_all;
use futures::{FutureExt, StreamExt};
use std::any::Any;
use std::ops::{Deref, DerefMut};
use std::panic::AssertUnwindSafe;
use thiserror::Error;
use tokio::signal::unix::{signal, SignalKind};
use tokio::spawn;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, info_span, Instrument};

/// Manager is the overall manager of the automation system where all devices and automations are
/// managed
pub struct Manager {
    device_managers: Vec<Box<dyn DeviceManager>>,
}

#[bon]
impl Manager {
    /// Create a new manager
    #[builder]
    pub fn new(

        #[builder(field)]
        mut device_managers: Vec<Box<dyn DeviceManager>>,
    ) -> Self {
        device_managers.insert(0, Box::new(()));
        Self { device_managers }
    }
}

impl<S: manager_builder::State> ManagerBuilder<S> {
    /// Add a device manager, this is required to support the devices of a particular manager
    /// If a manager of the given type is already added, then this call does nothing
    pub fn add_device_manager<M: DeviceManager>(mut self, manager: M) -> Self {
        for manager in self.device_managers.iter() {
            if manager.deref().as_any().is::<M>() {
                return self
            }
        }
        self.device_managers.push(Box::new(manager));
        self
    }
}

impl Manager {
    /// Fetch the given device manager
    ///
    /// # Errors
    /// Will return an error only if the device manager was not added when this manager was built
    pub fn device_manager<M: DeviceManager>(&mut self) -> Result<&mut M, DeviceManagerNotFound> {
        self.device_managers
            .iter_mut()
            .find_map(|any| <dyn Any>::downcast_mut(any.deref_mut()))
            .ok_or(DeviceManagerNotFound)
    }

    /// Creates each device in this set
    ///
    /// # Errors
    /// Returns an error if the appropriate managers were not added or if a device creation
    /// failed, see `Device::new` of the devices in this set for details on device creation
    pub async fn create<D: DeviceSet>(&mut self) -> Result<D, CreateDeviceError> {
        D::new(self).await
    }

    /// Creates a single device
    pub async fn add_device<D: Device>(&mut self, args: D::Args) -> Result<D, CreateDeviceError> {
        Ok(D::new(self.device_manager()?, args).await?)
    }

    /// Start the manager, this starts all device managers and automations.
    ///
    /// This is the main entry point for the program and should be called after all devices and
    /// automations have been set up
    pub async fn start<'a>(self, automations: impl IntoIterator<Item = Automation<'a>>) {
        async {
            let token = CancellationToken::new();
            debug!("Starting automations");
            for manager in self.device_managers {
                manager.start(token.clone());
            }

            debug!("Starting signal listener");
            #[allow(clippy::unwrap_used, reason = "signal creation is not expected to fail")]
            spawn(async move {
                let mut interrupt = signal(SignalKind::interrupt()).unwrap();
                let mut terminate = signal(SignalKind::terminate()).unwrap();
                let termination = futures::future::join(interrupt.recv(), terminate.recv());
                termination.await;
                token.cancel();
            });

            TokioScope::scope_and_block(move |scope| {
                info!("Starting main automation loop");
                let all_jobs = select_all(
                    automations.into_iter().map(|automation| {
                        let name = automation.name;
                        AssertUnwindSafe(automation.stream).catch_unwind().filter_map(move |result| {
                            let name = name.clone();
                            let option = match result {
                                Ok(job) => Some(job),
                                Err(panic) => {
                                    error!(automation = name, "Automation trigger panicked: {panic:?}");
                                    None
                                }
                            };
                            ready(option)
                        })
                    }),
                );
                for (name, job) in block_on_stream(all_jobs) {
                    info!("Job started");
                    scope.spawn(async move {
                        if let Err(panic) = AssertUnwindSafe(job).catch_unwind().await {
                            error!(automation = name, "Automation panicked: {:?}", panic);
                        }
                    })
                }
            });
        }.instrument(info_span!("automation_runner")).await
    }
}

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

trait DeviceManagerExt: Any {
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
