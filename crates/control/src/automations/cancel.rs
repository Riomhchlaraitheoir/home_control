use std::marker::PhantomData;
use crate::automations::{Automation, AutomationMutAction};
use futures::stream::FusedStream;
use futures::{select_biased, FutureExt, StreamExt};
use tracing::warn;

/// creates an automation of the `cancel` type, the behaviour is that if a trigger occurs while
/// the previous run is ongoing, then the previous run is canceled (at the next [poll](Future::poll) call)
///
/// NOTE: actions used with this type of automation must be cancel-safe, this means that dropping the
/// future before it has completed must not create an invalid internal state
pub fn cancel<'a, S, A>(input: S, automation: A) -> impl Automation<'a>
where
    S: FusedStream + Unpin + Send + 'a,
    A: AutomationMutAction<S::Item> + Send + 'a,
    S::Item: Send {
    Cancel {
        input,
        automation,
        _a: PhantomData
    }
}

struct Cancel<'a, S, A> {
    input: S,
    automation: A,
    _a: PhantomData<&'a ()>
}

impl<'a, S, A> Automation<'a> for Cancel<'a, S, A>
where
    S: FusedStream + Unpin + Send + 'a,
    A: AutomationMutAction<S::Item> + Send + 'a,
    S::Item: Send
{
    async fn run(&mut self) {
        let mut next = self.input.next().await;
        loop {
            let Some(trigger) = next else {
                return;
            };
            let mut current = Box::pin(self.automation.run(trigger).fuse());
            next = select_biased! {
                result = current => {
                    if let Err(error) = result {
                        warn!("automation error: {error}");
                    }
                    self.input.next().await
                },
                next = self.input.next() => next
            };
        };
    }
}