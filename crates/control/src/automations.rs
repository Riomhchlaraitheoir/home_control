//! This module defines the `Automation` trait and several implementations, these can be created
//! using the functions: `single`, `cancel`, `queued`, `parallel`, these functions create
//! automations with differing behaviours,
//!
//! In particular, the difference lies in the answer to the question:
//! "What happens when an automation is triggered when it hasn't yet completed its last run"

mod cancel;
mod parallel;
mod queued;
mod single;

use async_executor::{Executor};
pub use cancel::cancel;
use futures::future::BoxFuture;
pub use parallel::parallel;
pub use queued::queued;
pub use single::single;
use macros::automation_sets;

/// Run all given automations, will only return when all automations have ended (likely due to
/// closed input), this should only be expected to happen when the program is exiting
pub fn run_automations(mut automations: impl AutomationSet) {
    let mut futures = Vec::with_capacity(automations.size());
    automations.futures(&mut futures);

    let executor = Executor::new();
    let mut tasks = vec![];

    executor.spawn_many(futures, &mut tasks);

    futures::executor::block_on(
        executor.run(async {
            for task in tasks {
                task.await
            }
        })
    )
}

/// A set of one or more automations
///
/// Since the [Automation] trait is not dyn compatible, collections like [Vec] are not suitable
/// This trait is implemented for tuples containing automations, up to a tuple size of 21
/// If you need more automations in a set, then you can nest the tuples
pub trait AutomationSet {
    /// Returns all futures from automations in this set
    fn futures<'a>(&'a mut self, futures: &mut Vec<BoxFuture<'a, ()>>);

    /// returns the total number of automations in this set
    fn size(&self) -> usize;
}

automation_sets!(21);

impl<A: Automation> AutomationSet for A {
    fn futures<'a>(&'a mut self, futures: &mut Vec<BoxFuture<'a, ()>>) {
        futures.push(Box::pin(self.run()))
    }

    fn size(&self) -> usize {
        1
    }
}

/// A complete self-contained automation which is ready to run indefinitely
pub trait Automation {
    /// run this automation.
    /// the returned future should only finish when the input stream for the automation is closed
    fn run(&mut self) -> impl Future<Output=()> + Send;
}

/// An automation action which accepts a shared reference to self,
///
/// An async closure should implement this trait
/// `AsyncFn(T)` with feature="nightly"
/// `Fn(T) -> impl Future<Output=()>` otherwise
pub trait AutomationAction<T> {
    /// run the action in response to a trigger
    fn run(&self, trigger: T) -> impl Future<Output=()> + Send;
}

/// An automation action which accepts an exclusive reference to self, allowing this action to
/// have mutable state.
///
/// NOTE: Mutable actions **cannot** be used in the [parallel] automation type
///
/// An async closure should implement this trait
/// `AsyncFnMut(T)` with feature="nightly"
/// `FnMut(T) -> impl Future<Output=()>` otherwise
pub trait AutomationMutAction<T> {
    /// run the action in response to a trigger
    fn run(&mut self, trigger: T) -> impl Future<Output=()> + Send;
}

impl<F: Fn(T) -> Fut, Fut, T> AutomationAction<T> for F
where
    Fut: Future<Output=()> + Send,
{
    fn run(&self, trigger: T) -> impl Future<Output=()> {
        self(trigger)
    }
}

impl<F: FnMut(T) -> Fut, Fut, T> AutomationMutAction<T> for F
where
    Fut: Future<Output=()> + Send,
{
    fn run(&mut self, trigger: T) -> impl Future<Output=()> {
        self(trigger)
    }
}
