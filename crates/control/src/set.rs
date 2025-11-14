use crate::{ToggleValue, WriteValue};
use futures::future::{join_all, BoxFuture};
use futures::FutureExt;
use anyhow::Result;

/// A set of many toggle values which can be operated as one
pub struct ToggleSet<'a, T: Clone> {
    switches: Vec<&'a (dyn ToggleValue<Item = T> + Send + Sync)>,
}

impl<'a, T: Clone> ToggleSet<'a, T> {
    /// create a new set
    pub fn new(switches: impl IntoIterator<Item = &'a (dyn ToggleValue<Item = T> + Send + Sync)>) -> Self {
        Self {
            switches: switches.into_iter().collect(),
        }
    }
}

impl<T: Clone> WriteValue for ToggleSet<'_, T> {
    type Item = T;

    fn set(&self, value: Self::Item) -> BoxFuture<'_, Result<()>> {
        Box::pin(join_all(
            self.switches.iter().map(|switch| switch.set(value.clone())),
        ).map(|_| Ok(())))
    }
}


impl<T: Clone> ToggleValue for ToggleSet<'_, T> {
    fn toggle(&self) -> BoxFuture<'_, Result<()>> {
        Box::pin(join_all(
            self.switches.iter().map(|switch| switch.toggle()),
        ).map(|results| results.into_iter().collect()))
    }
}


/// A set of many write values which can be operated as one
pub struct WriteSet<'a, T: Clone> {
    switches: Vec<&'a dyn ToggleValue<Item = T>>,
}

impl<'a, T: Clone> WriteSet<'a, T> {
    /// create a new set
    pub fn new(switches: impl IntoIterator<Item = &'a dyn ToggleValue<Item = T>>) -> Self {
        Self {
            switches: switches.into_iter().collect(),
        }
    }
}

impl<T: Clone> WriteValue for WriteSet<'_, T> {
    type Item = T;

    fn set(&self, value: Self::Item) -> BoxFuture<'_, Result<()>> {
        Box::pin(join_all(
            self.switches.iter().map(|switch| switch.set(value.clone())),
        ).map(|_| Ok(())))
    }
}
