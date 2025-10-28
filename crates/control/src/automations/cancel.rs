use crate::automations::{Automation, AutomationMutAction};
use futures::stream::FusedStream;
use futures::{select_biased, FutureExt, StreamExt};

/// creates an automation of the `cancel` type, the behaviour is that if a trigger occurs while
/// the previous run is ongoing, then the previous run is canceled (at the next [poll](Future::poll) call)
///
/// NOTE: actions used with this type of automation must be cancel-safe, this means that dropping the
/// future before it has completed must not create an invalid internal state
pub fn cancel<S, A>(input: S, automation: A) -> impl Automation
where
    S: FusedStream + Unpin + Send,
    A: AutomationMutAction<S::Item> + Send,
    S::Item: Send {
    Cancel {
        input,
        automation,
    }
}

pub struct Cancel<S, A> {
    input: S,
    automation: A
}

impl<S, A> Automation for Cancel<S, A>
where
    S: FusedStream + Unpin + Send,
    A: AutomationMutAction<S::Item> + Send,
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
                _ = current => self.input.next().await,
                next = self.input.next() => next
            };
        };
    }
}