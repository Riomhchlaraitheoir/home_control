pub use ::control::*;
use async_signal::{Signal, Signals};
use control::automations::AutomationSet;
use futures::executor::block_on;
use futures::future::{join_all, BoxFuture};
pub use macros::DeviceSet;
use std::time::Duration;
use futures::{FutureExt, StreamExt};

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

    #[cfg(feature = "async-executor")]
    pub fn start(self, automations: &mut impl AutomationSet) -> impl Future<Output = ()> {
        use async_executor::Executor;
        let executor = Executor::new();
        #[cfg(feature = "zigbee")]
        let (zigbee_jobs, zigbee_abort_handles) = block_on(self.zigbee.start());

        let mut termination = Signals::new([Signal::Term, Signal::Int])
            .expect("Failed to register signal handler");

        #[cfg(feature = "arp")]
        self.arp.run();

        let mut jobs = Vec::with_capacity(automations.size());
        automations.futures(&mut jobs);

        #[allow(unused_mut, reason = "mut needed only with zigbee feature")]
        let mut tasks = jobs
            .into_iter()
            .map(|job| executor.spawn(job))
            .collect::<Vec<_>>();
        #[cfg(feature = "zigbee")]
        tasks.extend(zigbee_jobs.into_iter().map(|job| executor.spawn(job)));

        async move {
            termination.next().await;
            #[cfg(feature = "zigbee")]
            for handle in zigbee_abort_handles {
                handle.abort();
            }
            let mut timeout = async_timer::new_timer(Duration::from_millis(1000)).fuse();
            let mut tasks = join_all(tasks).fuse();
            futures::select! {
                _ = tasks => {}
                _ = timeout => {}
            }
        }
    }

    #[cfg(feature = "custom-executor")]
    pub fn start(self, automations: &mut impl AutomationSet) -> impl Future<Output = ()> {
            #[cfg(feature = "zigbee")]
            let (zigbee_jobs, zigbee_abort_handles) = block_on(self.zigbee.start());

            let mut termination = Signals::new([Signal::Term, Signal::Int])
                .expect("Failed to register signal handler");

            #[cfg(feature = "arp")]
            self.arp.run();

            let mut jobs = Vec::with_capacity(automations.size());
            automations.futures(&mut jobs);

            #[allow(unused_mut, reason = "mut needed only with zigbee feature")]
            let mut tasks = jobs
                .into_iter()
                .map(|job| tokio::spawn(job))
                .collect::<Vec<_>>();
            #[cfg(feature = "zigbee")]
            tasks.extend(zigbee_jobs.into_iter().map(|job| executor.spawn(job)));

            async move {
                termination.next().await;
                #[cfg(feature = "zigbee")]
                for handle in zigbee_abort_handles {
                    handle.abort();
                }
                let mut timeout = async_timer::new_timer(Duration::from_millis(1000)).fuse();
                let mut tasks = join_all(tasks).fuse();
                futures::select! {
                    _ = tasks => {}
                    _ = timeout => {}
                }
            }
    }

    #[cfg(not(any(feature = "async-executor", feature = "custom-executor")))]
    pub fn start(self, automations: &mut impl AutomationSet) -> impl Future<Output = ()> {
        compile_error!("Please add one of the following features in order to select an execuitor: tokio, async-ececutor")
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
