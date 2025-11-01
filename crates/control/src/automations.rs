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

pub use cancel::cancel;
use futures::future::BoxFuture;
use macros::automation_sets;
pub use parallel::parallel;
pub use queued::queued;
pub use single::single;

/// A set of one or more automations
///
/// This trait is implemented for the following types:
/// - any [Automation] implementor
/// - any tuple up to 21 elements where each element implements [AutomationSet]
/// - A `Vec<Box<dyn AutomationSet>>`
pub trait AutomationSet {
    /// Returns all futures from automations in this set
    fn futures<'a>(&'a mut self, futures: &mut Vec<BoxFuture<'a, ()>>);

    /// returns the total number of automations in this set
    fn size(&self) -> usize;
}

automation_sets!(21);

impl<A: Automation> AutomationSet for A {
    fn futures<'a>(&'a mut self, futures: &mut Vec<BoxFuture<'a, ()>>)
    {
        futures.push(Box::pin(self.run()));
    }

    fn size(&self) -> usize {
        1
    }
}

/// A complete self-contained automation which is ready to run indefinitely
pub trait Automation {
    /// run this automation.
    /// the returned future should only finish when the input stream for the automation is closed
    fn run<'a>(&'a mut self) -> impl Future<Output=()> + Send + 'a;
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
