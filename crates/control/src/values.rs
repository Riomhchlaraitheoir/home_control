use futures::future::{join_all, BoxFuture};
use futures::stream::BoxStream;
use futures::FutureExt;
use light_ranged_integers::RangedU8;
use std::ops::Not;
use crate::enum_value;

/// Sensor is an entity which streams data to the controller eg: thermostat
pub trait Sensor {
    /// Item is the type of the data streamed from this sensor
    type Item;
    /// subscribe returns a stream of data, it should be read from regularly to prevent the
    /// lagging receiver from slowing down other receivers, this stream can be safely dropped
    /// if it is no longer needed
    fn subscribe(&self) -> BoxStream<'_, Self::Item>;
}

impl<T> dyn Sensor<Item = T> {}

/// ReadValue represents an entity that accepts 'GET' requests to fetch this value's data
pub trait ReadValue {
    /// Item is the type of the data fetched from this sensor
    type Item;
    /// get issues a get request and waits for a response, this response may also be observed in the `Sensor::subscribe` stream in some implementations
    fn get(&self) -> BoxFuture<'_, anyhow::Result<Self::Item>>;
}

impl<T> dyn ReadValue<Item = T> {}

/// WriteValue represents an entity that can be written, this might range from a generic device
/// configuration option to an actual switch
pub trait WriteValue {
    /// Item is the type of data that can be written to this entity
    type Item;
    /// set writes the given data immediately to the entity
    fn set(
        &self,
        value: Self::Item,
    ) -> BoxFuture<'_, anyhow::Result<()>>;
}

impl<T> dyn WriteValue<Item = T> {}

/// ToggleValue is a `WriteValue` that also allows writing a special value to 'toggle' the entity,
/// whatever that may mean will depend on the device
pub trait ToggleValue: WriteValue {
    /// toggles the value
    fn toggle(&self) -> BoxFuture<'_, anyhow::Result<()>>;
}

impl<T> dyn ToggleValue<Item = T> {}

/// Group can be used to group multiple writable values together to write to each in a single call
pub struct Group<'a, T>(Vec<&'a T>);

impl<'a, T> Group<'a, T> {
    /// Create a new group
    pub fn new(values: impl IntoIterator<Item = &'a T>) -> Self {
        Self(values.into_iter().collect())
    }
}

impl<T: WriteValue> WriteValue for Group<'_, T> where T::Item: Clone {
    type Item = T::Item;
    fn set(
        &self,
        value: Self::Item,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        Box::pin(
            join_all(self.0.iter().map(|item| item.set(value.clone())))
                .map(|results| results.into_iter().collect())
        )
    }
}

impl<T: ToggleValue> ToggleValue for Group<'_, T> where T::Item: Clone {
    fn toggle(&self) -> BoxFuture<'_, anyhow::Result<()>> {
        Box::pin(
            join_all(self.0.iter().map(|item| item.toggle()))
                .map(|results| results.into_iter().collect())
        )
    }
}

/// ButtonEvent is an event from a button or other button-like device
#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum ButtonEvent {
    /// Press occurs when the button is pressed
    Press,
    /// Hold occurs after the button has been held for some time. Note: the actual length of time
    /// may vary by device, some devices may not issue this at all, for a more predictable
    /// behaviour holding can be calculated based in the intervale between Press and Release,
    /// or you could use the provided helper: [`count_presses`](StreamCustomExt::count-presses)
    Hold,
    /// Release occurs when the button is released
    Release,
}

enum_value!(ButtonEvent,
    "press" => Press,
    "hold" => Hold,
    "release" => Release
);

/// Some switch like values can emulate a toggle even when they don't natively support one, wrapping the value in this struct can achieve that
pub struct FakeToggle<V>(V)
where
    V: ReadValue + Sync,
    V: WriteValue,
    <V as ReadValue>::Item: Not<Output = <V as WriteValue>::Item>;

impl<V> WriteValue for FakeToggle<V>
where
    V: ReadValue + Sync,
    V: WriteValue,
    <V as ReadValue>::Item: Not<Output = <V as WriteValue>::Item>,
{
    type Item = <V as WriteValue>::Item;

    fn set(
        &self,
        value: Self::Item,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        self.0.set(value)
    }
}

impl<V> ToggleValue for FakeToggle<V>
where
    V: ReadValue + Sync,
    V: WriteValue,
    <V as ReadValue>::Item: Not<Output = <V as WriteValue>::Item>,
{
    fn toggle(&self) -> BoxFuture<'_, anyhow::Result<()>> {
        Box::pin(async {
            let value = self.0.get().await?;
            self.0.set(!value).await?;
            Ok(())
        })
    }
}

/// Percentage is useful for sensors that output a percentage between 0 and 100 inclusive
pub type Percentage = RangedU8<0, 100>;
