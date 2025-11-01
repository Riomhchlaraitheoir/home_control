use crate::automations::{Automation, AutomationMutAction};
use futures::future::poll_immediate;
use futures::{Stream, StreamExt};
use tracing::{info, info_span, warn, Instrument};

/// creates an automation of the `single` type, the behaviour is that if a trigger occurs while
/// the previous run is ongoing, then the second trigger will be ignored entirely
pub fn single<S, A>(name: String, input: S, automation: A) -> impl Automation
where
    S: Stream + Unpin + Send,
    A: AutomationMutAction<S::Item> + Send,
{
    Single {
        name,
        input,
        automation,
    }
}

struct Single<S, A> {
    name: String,
    input: S,
    automation: A,
}

impl<S, A> Automation for Single<S, A>
where
    S: Stream + Unpin + Send,
    A: AutomationMutAction<S::Item> + Send,
{
    async fn run(&mut self) {
        let span = info_span!("Automation {} ready", self.name);
        async {
            loop {
                let Some(trigger) = self.input.next().await else {
                    return;
                };
                info!("automation running");
                if let Err(error) = self.automation.run(trigger).await {
                    warn!("automation error: {error}");
                } else {
                    info!("automation complete");
                }
                while poll_immediate(self.input.next()).await.is_some() {
                    // consume all ready input from stream
                    continue;
                }
            }
        }.instrument(span).await
    }
}