use crate::automations::{Automation, AutomationMutAction};
use futures::{Stream, StreamExt};

/// creates an automation of the `queued` type, the behaviour is that if a trigger occurs while
/// the previous run is ongoing, then a second run will begin after the first completes
///
/// May drop queued runs if the queue becomes too large
pub fn queued<S, A>(input: S, automation: A) -> impl Automation
where
    S: Stream + Unpin + Send,
    A: AutomationMutAction<S::Item> + Send,
    S::Item: Send {
    Queued {
        input,
        automation,
    }
}

pub struct Queued<S, A> {
    input: S,
    automation: A
}

impl<S, A> Automation for Queued<S, A>
where
    S: Stream + Unpin + Send,
    A: AutomationMutAction<S::Item> + Send,
    S::Item: Send
{
    async fn run(&mut self) {
        while let Some(trigger) = self.input.next().await {
            self.automation.run(trigger).await;
        }
    }
}
