use crate::automations::{Automation, AutomationMutAction};
use futures::future::poll_immediate;
use futures::{Stream, StreamExt};

/// creates an automation of the `single` type, the behaviour is that if a trigger occurs while
/// the previous run is ongoing, then the second trigger will be ignored entirely
pub fn single<S, A>(input: S, automation: A) -> impl Automation
where
    S: Stream + Unpin + Send,
    A: AutomationMutAction<S::Item> + Send {
    Single {
        input,
        automation,
    }
}

pub struct Single<S, A> {
    input: S,
    automation: A
}

impl<S, A> Automation for Single<S, A>
where
    S: Stream + Unpin + Send,
    A: AutomationMutAction<S::Item> + Send
{
    async fn run(&mut self) {
        loop {
            let Some(trigger) = self.input.next().await else {
                return;
            };
            self.automation.run(trigger).await;
            while poll_immediate(self.input.next()).await.is_some() {
                // consume all ready input from stream
                continue
            }
        }
    }
}