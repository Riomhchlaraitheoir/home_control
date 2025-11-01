use crate::automations::{Automation, AutomationAction};
use futures::future::{BoxFuture};
use futures::stream::{FusedStream};
use std::ops::DerefMut;
use std::pin::Pin;
use std::task::{Context, Poll};
use tracing::{info, info_span, warn, Instrument};

/// creates an automation of the `parallel` type, the behaviour is that if a trigger occurs while
/// the previous run is ongoing, then a second run will be started in parallel to the first, for
/// this reason [AutomationMutAction](super::AutomationMutAction) is not permitted here since it
/// requires an exclusive reference to run it
pub fn parallel<S, A>(name: String, max_parallel_runs: usize, input: S, automation: A) -> impl Automation
where
    S: FusedStream + Unpin + Send,
    A: AutomationAction<S::Item> + Send + Sync,
    S::Item: Send {
    Parallel {
        name,
        max_parallel: max_parallel_runs,
        input,
        automation,
    }
}

struct Parallel<S, A> {
    name: String,
    max_parallel: usize,
    input: S,
    automation: A,
}

impl<S, A> Automation for Parallel<S, A>
where
    S: FusedStream + Unpin + Send,
    A: AutomationAction<S::Item> + Send + Sync,
    S::Item: Send
{
    fn run<'a>(&'a mut self) -> impl Future<Output=()> + Send + 'a {
        let span = info_span!("Automation {} ready", self.name);
        ParallelFuture {
            input: Pin::new(&mut self.input),
            automation: &self.automation,
            running: Vec::with_capacity(self.max_parallel),
            count: 0,
        }.instrument(span)
    }
}

struct ParallelFuture<'a, S, A> {
    input: Pin<&'a mut S>,
    automation: &'a A,
    count: usize,
    running: Vec<Option<BoxFuture<'a, Result<(), String>>>>,
}

impl<'a, S, A> Future
    for ParallelFuture<'a, S, A>
where
    S: FusedStream + Unpin + Send,
    A: AutomationAction<S::Item> + Send + Sync,
    S::Item: Send
{
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self {
            input,
            automation,
            running,
            count,
        } = self.deref_mut();
        if !input.is_terminated()
            && *count < running.len()
            && let Poll::Ready(Some(trigger)) = input.as_mut().poll_next(cx)
        {
            info!("Automation triggered");
            let future = automation.run(trigger);
            running.first_mut().unwrap().replace(Box::pin(future));
            *count += 1;
        }

        for slot in running {
            if let Some(future) = slot.as_mut() {
                match future.as_mut().poll(cx) {
                    Poll::Ready(result) => {
                        // remove future
                        let _ = slot.take();
                        *count -= 1;
                        if let Err(error) = result {
                            warn!("automation error: {error}");
                        } else {
                            info!("automation complete");
                        }
                    }
                    Poll::Pending => {}
                }
            }
        }
        if input.is_terminated() && *count == 0 {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}
