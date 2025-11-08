#![feature(async_iterator)]
#![deny(missing_docs)]
#![doc = include_str!("../README.md")]

mod button;
mod set;

use crate::button::ButtonPressStream;
use futures::{Stream, StreamExt};
use light_ranged_integers::RangedU8;
use std::future::ready;
use std::ops::Not;
use thiserror::Error;
pub use button::ButtonPressEvent;
pub use set::*;

/// Sensor is an entity which streams data to the controller eg: thermostat
pub trait Sensor {
    /// Item is the type of the data streamed from this sensor
    type Item;
    /// subscribe returns a stream of data, it should be read from regularly to prevent the
    /// lagging receiver from slowing down other receivers, this stream can be safely dropped
    /// if it is no longer needed
    fn subscribe(&self) -> Box<dyn Stream<Item = Self::Item> + Unpin + Send + '_>;
}

impl<T> dyn Sensor<Item =T> {}

/// ReadValue represents an entity that accepts 'GET' requests to fetch this value's data
pub trait ReadValue {
    /// Item is the type of the data fetched from this sensor
    type Item;
    /// get issues a get request and waits for a response, this response may also be observed in the `Sensor::subscribe` stream in some implementations
    fn get(&self) -> Box<dyn Future<Output = Result<Self::Item, Error>> + Unpin + Send + '_>;
}

impl<T> dyn ReadValue<Item =T> {}

/// WriteValue represents an entity that can be written, this might range from a generic device
/// configuration option to an actual switch
pub trait WriteValue {
    /// Item is the type of data that can be written to this entity
    type Item;
    /// set writes the given data immediately to the entity
    fn set(&self, value: Self::Item) -> Box<dyn Future<Output = Result<(), Error>> + Unpin + Send + '_>;
}

impl<T> dyn WriteValue<Item =T> {}

/// ToggleValue is a `WriteValue` that also allows writing a special value to 'toggle' the entity,
/// whatever that may mean will depend on the device
pub trait ToggleValue: WriteValue {
    /// toggles the value
    fn toggle(&self) -> Box<dyn Future<Output = Result<(), Error>> + Unpin + Send + '_>;
}

impl<T> dyn ToggleValue<Item =T> {}

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

/// SwitchState represents a switch entity which can be on or off
#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum SwitchState {
    /// on
    On,
    /// off
    Off,
}

impl Not for SwitchState {
    type Output = Self;

    fn not(self) -> Self::Output {
        match self {
            Self::On => Self::Off,
            Self::Off => Self::On,
        }
    }
}

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
    <V as ReadValue>::Item: Not<Output = <V as WriteValue>::Item>
{
    type Item = <V as WriteValue>::Item;

    fn set(&self, value: Self::Item) -> Box<dyn Future<Output=Result<(), Error>> + Unpin + Send + '_> {
        self.0.set(value)
    }
}

impl<V> ToggleValue for FakeToggle<V>
where
    V: ReadValue + Sync,
    V: WriteValue,
    <V as ReadValue>::Item: Not<Output = <V as WriteValue>::Item>
{
    fn toggle(&self) -> Box<dyn Future<Output=Result<(), Error>> + Unpin + Send + '_> {
        Box::new(Box::pin(
            async {
                let value = self.0.get().await?;
                self.0.set(!value).await?;
                Ok(())
            }
        ))
    }
}

/// Percentage is useful for sensors that output a percentage between 0 and 100 inclusive
pub type Percentage = RangedU8<0, 100>;

/// some helpers provided as extensions to stream since streams are quite useful as input for
/// automations
pub trait StreamCustomExt: Stream + Sized {
    /// filter out any values not equal to the given value, eg:
    /// ```
    /// use futures::StreamExt;
    /// use control::{ButtonEvent, Sensor};
    ///
    /// async fn example(button: Sensor<ButtonEvent>) {
    ///     let events = button.subscribe();
    ///     let presses = events.filter_eq(ButtonEvent::Press);
    ///     while presses.next().await.is_some() {
    ///         println!("Button was pressed")
    ///     }
    /// }
    fn filter_eq(self, value: Self::Item) -> impl Stream<Item = Self::Item>
    where
        Self::Item: PartialEq + 'static,
    {
        self.filter(move |v| ready(value.eq(v)))
    }

    /// next_eq wait for the next value in the stream which equals the given value,
    /// eg: waiting for a certain value from an enum sensor like a button
    fn next_eq(&mut self, value: Self::Item) -> impl Future<Output = Option<Self::Item>>
    where
        Self::Item: PartialEq,
        Self: Unpin,
    {
        async move {
            loop {
                let v = self.next().await?;
                if value == v {
                    return Some(v);
                }
            }
        }
    }

    /// Counts the number of times a button is pressed, and whether it ends in a held
    /// press or not, up to a const MAX
    /// This allows triggering automations when a button is double, triple, etc pressed, or
    /// combining with filter_eq to wait for a particular input,
    /// returns [None] if the stream has ended, otherwise returns a [ButtonPressEvent]\<[MAX]>
    fn count_presses<const MAX: u8>(self) -> ButtonPressStream<Self, MAX>
    where
        Self: Stream<Item = ButtonEvent> + Unpin,
    {
        ButtonPressStream::new(self)
    }
}

impl<S: Stream> StreamCustomExt for S {}

/// An error enum for integrations to use
#[derive(Debug, Error)]
pub enum Error {
    /// This error can be returned from any function reading a stream from a device, and indicates that the stream closed
    #[error("Input stream closed")]
    InputStreamClosed,
    /// This indicates that there was an error while communicating with a device
    #[error("Communication error: {0}")]
    Communication(String),
}

#[doc(hidden)]
pub trait ExposesSubManager<M> {
    fn shared(&self) -> &M;
    fn exclusive(&mut self) -> &mut M;
}

/// A Device which can be used in the home_control system
pub trait Device: Sized {
    /// Creation args needed to create this device
    type Args;
    /// The manager type that this device needs
    type Manager;
    /// The error that can occur when creating a device
    type Error;

    /// creates the device
    fn new(manager: &mut Self::Manager, args: Self::Args) -> Result<Self, Self::Error>;
}
