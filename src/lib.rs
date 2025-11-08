#![feature(mpmc_channel)]

use futures::executor::block_on_stream;
use futures::stream::select_all;
use tokio::signal::unix::{SignalKind, signal};
use tokio::spawn;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

pub mod automation;

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

    pub async fn start<'a>(self, automations: impl IntoIterator<Item = Automation<'a>>) {
        let token = CancellationToken::new();
        debug!("Starting automations");
        #[cfg(feature = "zigbee")]
        {
            debug!("Initializing zigbee");
            self.zigbee.start(token.clone()).await;
            debug!("zigbee initialized");
        };

        #[cfg(feature = "arp")]
        {
            debug!("Starting ARP");
            spawn(self.arp.run(token.clone()));
            debug!("ARP started")
        }

        debug!("Starting signal listener");
        spawn(async move {
            let mut interrupt = signal(SignalKind::interrupt()).unwrap();
            let mut terminate = signal(SignalKind::terminate()).unwrap();
            let termination = futures::future::join(interrupt.recv(), terminate.recv());
            termination.await;
            token.cancel();
        });

        async_scoped::TokioScope::scope_and_block(move |scope| {
            info!("Starting main automation loop");
            for job in block_on_stream(select_all(automations.into_iter().map(|automation| automation.0))) {
                info!("Job started");
                scope.spawn(job)
            }
        });
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
