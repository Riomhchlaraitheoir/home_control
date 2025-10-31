use crate::automations::{Automation, AutomationMutAction};
use futures::future::poll_immediate;
use futures::{Stream, StreamExt};
use tracing::warn;

/// creates an automation of the `single` type, the behaviour is that if a trigger occurs while
/// the previous run is ongoing, then the second trigger will be ignored entirely
pub fn single<'a, S, A>(input: S, automation: A) -> impl Automation<'a>
where
    S: Stream + Unpin + Send + 'a,
    A: AutomationMutAction<S::Item> + Send + 'a {
    Single {
        input,
        automation,
    }
}

struct Single<S, A> {
    input: S,
    automation: A
}

impl<'a, S, A> Automation<'a> for Single<S, A>
where
    S: Stream + Unpin + Send + 'a,
    A: AutomationMutAction<S::Item> + Send + 'a
{
    async fn run(&mut self) {
        loop {
            let Some(trigger) = self.input.next().await else {
                return;
            };
            if let Err(error) = self.automation.run(trigger).await {
                warn!("automation error: {error}");
            }
            while poll_immediate(self.input.next()).await.is_some() {
                // consume all ready input from stream
                continue
            }
        }
    }
}