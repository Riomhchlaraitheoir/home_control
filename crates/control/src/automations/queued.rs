use crate::automations::{Automation, AutomationMutAction};
use futures::{Stream, StreamExt};
use tracing::warn;

/// creates an automation of the `queued` type, the behaviour is that if a trigger occurs while
/// the previous run is ongoing, then a second run will begin after the first completes
///
/// May drop queued runs if the queue becomes too large
pub fn queued<'a, S, A>(input: S, automation: A) -> impl Automation<'a>
where
    S: Stream + Unpin + Send + 'a,
    A: AutomationMutAction<S::Item> + Send + 'a,
    S::Item: Send {
    Queued {
        input,
        automation,
    }
}

struct Queued<S, A> {
    input: S,
    automation: A
}

impl<'a, S, A> Automation<'a> for Queued<S, A>
where
    S: Stream + Unpin + Send + 'a,
    A: AutomationMutAction<S::Item> + Send + 'a,
    S::Item: Send
{
    async fn run(&mut self) {
        while let Some(trigger) = self.input.next().await {
            if let Err(error) = self.automation.run(trigger).await {
                warn!("automation error: {error}");
            }
        }
    }
}
