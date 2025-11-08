#![feature(mpmc_channel)]

use crate::threads::new_thread_pool;
#[cfg(feature = "zigbee")]
use futures::executor::block_on;
use futures::executor::block_on_stream;
use signal_hook::consts::{SIGINT, SIGTERM};
use signal_hook::iterator::Signals;
use std::thread;
use std::time::Duration;
use tracing::debug;

pub mod automation;
mod threads;

use automation::Automation;
pub use control::*;
pub use macros::DeviceSet;

#[cfg(feature = "zigbee")]
pub mod zigbee {
    pub use ::zigbee::*;
}
#[cfg(feature = "wiz")]
pub mod wiz {
    pub use ::wiz::*;
}
#[cfg(feature = "arp")]
pub mod arp {
    pub use ::arp::*;
}

#[derive(Default)]
#[allow(
    clippy::manual_non_exhaustive,
    reason = "The dummy field is needed to satisfy some traits for integrations without a manager type"
)]
pub struct Manager {
    #[cfg(feature = "zigbee")]
    pub zigbee: zigbee::Manager,
    #[cfg(feature = "arp")]
    pub arp: arp::ArpManager,
    // a dummy manager for platforms without any manager
    dummy: (),
}

impl Manager {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn create<D: DeviceSet>(&mut self) -> Result<D, Box<dyn std::error::Error>> {
        D::new(self)
    }

    pub fn start<'a>(self, automations: impl IntoIterator<Item = Automation<'a>>) {
        #[cfg(feature = "zigbee")]
        let (zigbee_jobs, zigbee_abort_handles) = block_on(self.zigbee.start());

        let mut termination =
            Signals::new([SIGTERM, SIGINT]).expect("Failed to register signal handler");

        #[cfg(feature = "arp")]
        self.arp.run();

        thread::scope(|scope| {
            let pool = new_thread_pool(scope, 10, "home-control");
            let all_jobs =
                futures::stream::select_all(automations.into_iter()
                    .map(|automation| automation.0));
            let (all_jobs, abort) = futures::stream::abortable(all_jobs);

            #[cfg(feature = "zigbee")]
            for (name, future) in zigbee_jobs {
                debug!("starting thread {}", name);
                thread::Builder::new()
                    .name(name.to_string())
                    .spawn_scoped(scope, move || block_on(future))
                    .expect("Failed to spawn thread");
            }

            scope.spawn(move || {
                if termination.forever().next().is_some() {
                    abort.abort();
                    #[cfg(feature = "zigbee")]
                    for handle in zigbee_abort_handles {
                        handle.abort();
                    }
                }
            });

            for job in block_on_stream(all_jobs) {
                pool.execute(job);
            }
            pool.cancel(Duration::from_secs(5));
        })
    }

    pub fn add_device<D: Device>(&mut self, args: D::Args) -> Result<D, D::Error>
    where
        Self: ExposesSubManager<D::Manager>,
    {
        D::new(self.exclusive(), args)
    }
}

/// This is a set of devices which can be created together using `Manager::create_devices`
///
/// This can be derived for any struct which has a create function returning a `bon` builder
/// The create function must have at least two parameters: `name: &'static str` and `manager: &mut Manager`
///
/// This function is used by duck typing (The macro calls the function, resulting in a compile error if the function is not present) rather than using triats
/// This allows additional parameters to be defined in the device as needed rather than being tied to a trait definition
pub trait DeviceSet: Sized {
    fn new(manager: &mut Manager) -> Result<Self, Box<dyn std::error::Error>>;
}

#[cfg(feature = "zigbee")]
impl ExposesSubManager<zigbee::Manager> for Manager {
    fn shared(&self) -> &::zigbee::Manager {
        &self.zigbee
    }

    fn exclusive(&mut self) -> &mut ::zigbee::Manager {
        &mut self.zigbee
    }
}

#[cfg(feature = "arp")]
impl ExposesSubManager<arp::ArpManager> for Manager {
    fn shared(&self) -> &::arp::ArpManager {
        &self.arp
    }

    fn exclusive(&mut self) -> &mut ::arp::ArpManager {
        &mut self.arp
    }
}

impl ExposesSubManager<()> for Manager {
    fn shared(&self) -> &() {
        &self.dummy
    }

    fn exclusive(&mut self) -> &mut () {
        &mut self.dummy
    }
}
