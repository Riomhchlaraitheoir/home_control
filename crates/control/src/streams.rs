use crate::button::ButtonPressStream;
use crate::ButtonEvent;
use futures::{Stream, StreamExt};
use pin_project::pin_project;
use std::future::ready;
use std::pin::Pin;
use std::task::{Context, Poll};
use thiserror::Error;

/// some helpers provided as extensions to stream since streams are quite useful as input for
/// automations
pub trait StreamCustomExt: Stream + Sized {
    /// filter out any values not equal to the given value, eg:
    /// ```
    /// use futures::StreamExt;
    /// use control::{ButtonEvent, Sensor, StreamCustomExt};
    ///
    /// async fn example(button: impl Sensor<Item=ButtonEvent>) {
    ///     let events = button.subscribe();
    ///     let mut presses = events.filter_eq(ButtonEvent::Press);
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

    /// Filters out blocks of equal items so that this stream only yields a value when the value has changed
    fn filter_changes(self) -> impl Stream<Item = Self::Item>
    where Self::Item: PartialEq + Clone
    {
        Changes {
            stream: self,
            last_item: None,
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

#[pin_project]
struct Changes<S: Stream> {
    #[pin]
    stream: S,
    last_item: Option<S::Item>,
}

impl<S: Stream> Stream for Changes<S>
where
    S::Item: PartialEq + Clone,
{
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();
        this.stream.poll_next_unpin(cx).map(|next| next.filter(|item| {
            if this.last_item.as_ref().is_none_or(|last| item != last) {
                *this.last_item = Some(item.clone());
                true
            } else {
                false
            }
        }))
    }
}

impl<S: Stream> StreamCustomExt for S {}

/// This error indicates that an action failed due to a closed input stream
#[derive(Debug, Error)]
#[error("The input stream has closed")]
pub struct InputStreamClosed;
