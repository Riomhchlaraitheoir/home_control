use crate::automations::{Automation, AutomationMutAction};
use futures::{Stream, StreamExt};
use tracing::{info_span, warn, Instrument};

/// creates an automation of the `queued` type, the behaviour is that if a trigger occurs while
/// the previous run is ongoing, then a second run will begin after the first completes
///
/// May drop queued runs if the queue becomes too large
pub fn queued<S, A>(name: String, input: S, automation: A) -> impl Automation
where
    S: Stream + Unpin + Send,
    A: AutomationMutAction<S::Item> + Send,
    S::Item: Send,
{
    Queued {
        name,
        input,
        automation,
    }
}

struct Queued<S, A> {
    name: String,
    input: S,
    automation: A,
}

impl<S, A> Automation for Queued<S, A>
where
    S: Stream + Unpin + Send,
    A: AutomationMutAction<S::Item> + Send,
    S::Item: Send,
{
    async fn run(&mut self) {
        let span = info_span!("Automation {} ready", self.name);
        async {
            while let Some(trigger) = self.input.next().await {
                if let Err(error) = self.automation.run(trigger).await {
                    warn!("automation error: {error}");
                }
            }
        }.instrument(span).await
    }
}
