#![doc = include_str!("../README.md")]

pub mod automation;
mod button;
pub mod device;
pub mod device_manager;
pub use reflect;
mod set;
mod streams;
mod values;

use crate::automation::Automation;
use crate::device::{CreateDeviceError, Device, DeviceSet};
use crate::device_manager::{DeviceManager, DeviceManagerNotFound};
use async_scoped::TokioScope;
use bon::bon;
pub use button::ButtonPressEvent;
use futures::executor::block_on_stream;
use futures::future::{BoxFuture, ready};
use futures::stream::select_all;
use futures::{FutureExt, StreamExt};
pub use set::*;
use std::any::Any;
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::panic::AssertUnwindSafe;
pub use streams::*;
use tokio::signal::unix::{SignalKind, signal};
use tokio::spawn;
use tokio_util::sync::CancellationToken;
use tracing::{Instrument, debug, error, info, info_span};
use reflect::{DeviceInfo, DeviceType};
pub use values::*;

/// Manager is the overall manager of the automation system where all devices and automations are
/// managed
pub struct Manager<'a> {
    device_managers: Vec<Box<dyn DeviceManager>>,
    services: Vec<(String, BoxFuture<'a, anyhow::Result<()>>)>,
}

/// A service to run in the background
pub trait Service<'a>: Send + 'a {
    /// The name of the service (will be used in logs)
    fn name(&self) -> String;

    /// Start the service, returning when either finished or failed
    fn start(self) -> impl Future<Output = anyhow::Result<()>> + Send + 'a;
}

#[bon]
impl<'a> Manager<'a> {
    /// Create a new manager
    #[builder]
    pub fn new(
        #[builder(field)] mut device_managers: Vec<Box<dyn DeviceManager>>,
        #[builder(field)] services: Vec<(String, BoxFuture<'a, anyhow::Result<()>>)>,
    ) -> Self {
        device_managers.insert(0, Box::new(()));
        Self {
            device_managers,
            services
        }
    }
}

impl<'a, S: manager_builder::State> ManagerBuilder<'a, S> {
    /// Add a device manager, this is required to support the devices of a particular manager
    /// If a manager of the given type is already added, then this call does nothing
    pub fn add_device_manager<M: DeviceManager>(mut self, manager: M) -> Self {
        for manager in self.device_managers.iter() {
            if manager.deref().as_any().is::<M>() {
                return self;
            }
        }
        self.device_managers.push(Box::new(manager));
        self
    }
}

impl<'a> Manager<'a> {
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
    pub async fn add_device<D: Device<Args = ()>>(&mut self, id: String, device_type: DeviceType) -> Result<D, CreateDeviceError> {
        Ok(D::new(self.device_manager()?, DeviceInfo {
            name: id.clone(),
            id,
            description: None,
            device_type,
            tags: HashMap::default(),
        }).await?)
    }

    /// Creates a single device
    pub async fn add_device_with_args<D: Device>(&mut self, id: String, device_type: DeviceType, args: D::Args) -> Result<D, CreateDeviceError> {
        Ok(D::new_with_args(self.device_manager()?, DeviceInfo {
            name: id.clone(),
            id,
            description: None,
            device_type,
            tags: HashMap::default(),
        }, args).await?)
    }

    /// Add a service that should run in the background
    pub fn add_service(&mut self, service: impl Service<'a>) {
        self.services.push((service.name(), Box::pin(service.start())));
    }

    /// Start the manager, this starts all device managers and automations.
    ///
    /// This is the main entry point for the program and should be called after all devices and
    /// automations have been set up
    pub async fn start(self, automations: impl IntoIterator<Item = Automation<'a>>) {
        async {
            let token = CancellationToken::new();
            debug!("Starting automations");
            for manager in self.device_managers {
                manager.start(token.clone());
            }

            debug!("Starting signal listener");
            #[allow(
                clippy::unwrap_used,
                reason = "signal creation is not expected to fail"
            )]
            spawn(async move {
                let mut interrupt = signal(SignalKind::interrupt()).unwrap();
                let mut terminate = signal(SignalKind::terminate()).unwrap();
                let termination = futures::future::join(interrupt.recv(), terminate.recv());
                termination.await;
                token.cancel();
            });

            TokioScope::scope_and_block(move |scope| {
                info!("Starting services");
                for (name, future) in self.services {
                    let service_span = info_span!("service", service = name.as_str());

                    scope.spawn(async move {
                        match AssertUnwindSafe(future)
                            .catch_unwind()
                            .await {
                            Err(panic) => {
                                error!(service = name, "Service panicked: {:?}", panic);
                            }
                            Ok(Err(error)) => {
                                error!(service = name, "Service error: {:?}", error);
                            }
                            Ok(Ok(())) => {
                                info!(service = name, "Service finished");
                            }
                        }
                    }.instrument(service_span))
                }

                info!("Starting main automation loop");
                let all_jobs = select_all(automations.into_iter().map(|automation| {
                    let name = automation.name;
                    AssertUnwindSafe(automation.stream)
                        .catch_unwind()
                        .filter_map(move |result| {
                            let name = name.clone();
                            let option = match result {
                                Ok(job) => Some(job),
                                Err(panic) => {
                                    error!(
                                        automation = name,
                                        "Automation trigger panicked: {panic:?}"
                                    );
                                    None
                                }
                            };
                            ready(option)
                        })
                }));
                for (name, job) in block_on_stream(all_jobs) {
                    info!("Job started");
                    scope.spawn(async move {
                        if let Err(panic) = AssertUnwindSafe(job).catch_unwind().await {
                            error!(automation = name, "Automation panicked: {:?}", panic);
                        }
                    })
                }
            });
        }
        .instrument(info_span!("automation_runner"))
        .await
    }
}
