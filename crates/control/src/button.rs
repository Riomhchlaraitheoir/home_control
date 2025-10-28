use crate::ButtonEvent;
use async_timer::new_timer;
use async_timer::timer::Platform as Timer;
use futures::{Stream, StreamExt};
use light_ranged_integers::RangedU8;
use std::ops::DerefMut;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

const PRESS_INTERVAL: Duration = Duration::from_millis(500);

pub struct ButtonPressStream<S: Stream<Item = ButtonEvent> + Unpin, const MAX: u8> {
    stream: S,
    count: RangedU8<1, MAX>,
    released: bool,
    timer: Option<Pin<Box<Timer>>>
}

impl<S: Stream<Item=ButtonEvent> + Unpin, const MAX: u8> ButtonPressStream<S, MAX> {
    pub(crate) fn new(stream: S) -> Self {
        Self {
            stream,
            count: RangedU8::new(1),
            released: false,
            timer: None,
        }
    }
}

impl<S: Stream<Item = ButtonEvent> + Unpin, const MAX: u8> Stream for ButtonPressStream<S, MAX> {
    type Item = ButtonPressEvent<MAX>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let Self { stream, count, released, timer } = self.deref_mut();
        let Some(timer_pin) = timer.as_mut() else {
            // wait for first press
            return match stream.poll_next_unpin(cx) {
                Poll::Ready(Some(ButtonEvent::Press)) => {
                    *count = RangedU8::new(1);
                    *released = false;
                    *timer = Some(Box::pin(new_timer(PRESS_INTERVAL)));
                    let timer_poll = timer.as_mut().expect("just set Some").as_mut().poll(cx);
                    if timer_poll.is_ready() {
                        panic!("Timer should not be ready immediately after starting")
                    }
                    Poll::Pending
                }
                Poll::Ready(Some(_)) | Poll::Pending => Poll::Pending,
                Poll::Ready(None) => Poll::Ready(None)
            };
        };

        if *released {
            match stream.poll_next_unpin(cx) {
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Ready(Some(ButtonEvent::Press)) => {
                    *count += 1;
                    return if *count == MAX {
                        // reached max presses
                        *timer = None;
                        Poll::Ready(Some(ButtonPressEvent::Press(*count)))
                    } else {
                        *released = false;
                        *timer = Some(Box::pin(new_timer(PRESS_INTERVAL)));
                        let timer_poll = timer.as_mut().expect("just set Some").as_mut().poll(cx);
                        if timer_poll.is_ready() {
                            panic!("Timer should not be ready immediately after starting")
                        }
                        Poll::Pending
                    }
                }
                Poll::Pending | Poll::Ready(Some(_)) => {}
            }

            match timer_pin.as_mut().poll(cx) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(()) => {
                    // timer has finished, remove timer, return event, wait for next press
                    *timer = None;
                    Poll::Ready(Some(ButtonPressEvent::Press(*count)))
                }
            }
        } else {
            match stream.poll_next_unpin(cx) {
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Ready(Some(ButtonEvent::Release)) => {
                    // button released, set released = true, poll again
                    *released = true;
                    return self.poll_next(cx)
                }
                Poll::Pending | Poll::Ready(Some(_)) => {}
            }

            match timer_pin.as_mut().poll(cx) {
                Poll::Pending => Poll::Pending,
                Poll::Ready(()) => {
                    // timer has finished, remove timer, return event, wait for next press
                    *timer = None;
                    Poll::Ready(Some(ButtonPressEvent::Hold(*count)))
                }
            }
        }
    }
}

/// An event for a counted button press, up to [MAX] presses.
/// If the last press is held, then the `Self::Hold` variant is used
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ButtonPressEvent<const MAX: u8> {
    /// Represents X presses
    Press(RangedU8<1, MAX>),
    /// Represents X presses, with the final press being held
    Hold(RangedU8<1, MAX>),
}
