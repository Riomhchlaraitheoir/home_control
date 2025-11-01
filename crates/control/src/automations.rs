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
pub fn run_automations<'a>(mut automations: impl AutomationSet<'a>) {
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
/// This trait is implemented for the following types:
/// - any [Automation] implementor
/// - any tuple up to 21 elements where each element implements [AutomationSet]
/// - A `Vec<Box<dyn AutomationSet>>`
pub trait AutomationSet<'a>: 'a {
    /// Returns all futures from automations in this set
    fn futures<'b>(&'b mut self, futures: &mut Vec<BoxFuture<'b, ()>>) where 'a:'b;

    /// returns the total number of automations in this set
    fn size(&self) -> usize;
}

automation_sets!(21);

impl<'a, A: Automation<'a>> AutomationSet<'a> for A {
    fn futures<'b>(&'b mut self, futures: &mut Vec<BoxFuture<'b, ()>>)
    where
        'a: 'b,
    {
        futures.push(Box::pin(self.run()))
    }

    fn size(&self) -> usize {
        1
    }
}

impl<'a> AutomationSet<'a> for Vec<Box<dyn AutomationSet<'a>>> {
    fn futures<'b>(&'b mut self, futures: &mut Vec<BoxFuture<'b, ()>>) where 'a: 'b {
        for set in self {
            set.futures(futures);
        }
    }

    fn size(&self) -> usize {
        self.iter().map(|set| set.size()).sum()
    }
}

/// A complete self-contained automation which is ready to run indefinitely
pub trait Automation<'a>: 'a {
    /// run this automation.
    /// the returned future should only finish when the input stream for the automation is closed
    fn run(&mut self) -> impl Future<Output=()> + Send;
}

/// An automation action which accepts a shared reference to self,
pub trait AutomationAction<T> {
    /// run the action in response to a trigger
    fn run(&self, trigger: T) -> impl Future<Output=Result<(), String>> + Send;
}

/// An automation action which accepts an exclusive reference to self, allowing this action to
/// have mutable state.
///
/// NOTE: Mutable actions **cannot** be used in the [parallel] automation type
pub trait AutomationMutAction<T> {
    /// run the action in response to a trigger
    fn run(&mut self, trigger: T) -> impl Future<Output=Result<(), String>> + Send;
}

impl<F: Fn(T) -> Fut, Fut, T> AutomationAction<T> for F
where
    Fut: Future<Output=Result<(), String>> + Send,
{
    fn run(&self, trigger: T) -> impl Future<Output=Result<(), String>> {
        self(trigger)
    }
}

impl<F: FnMut(T) -> Fut, Fut, T> AutomationMutAction<T> for F
where
    Fut: Future<Output=Result<(), String>> + Send,
{
    fn run(&mut self, trigger: T) -> impl Future<Output=Result<(), String>> {
        self(trigger)
    }
}
