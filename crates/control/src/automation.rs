//! Automations run when a trigger fires and executes some action

use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};
use futures::future::{BoxFuture};
use futures::{Stream};
use futures::stream::BoxStream;
use pin_project::pin_project;
use tracing::{debug, warn, Instrument};

#[must_use = "An automation does nothing unless it is passed into Manager::start"]
/// An Automation definition, with a trigger stream and an action
pub struct Automation<'a>(pub(crate) BoxStream<'a, BoxFuture<'a, ()>>);

/// An Automation action, to be run each time the automation triggers, is already implemented for:
///
/// `Fn(Trigger) -> impl Future<Output=Result<(), String>> + Send`
pub trait Action<Trigger>: Send + Sync + Copy {
    /// Run this action
    fn run(self, trigger: Trigger) -> impl Future<Output = Result<(), String>> + Send;
}


// pub trait MutableAction<Trigger>: Send + Sync {
//     fn run(&mut self, trigger: Trigger) -> impl Future<Output = Result<(), String>> + Send;
// }

impl<F: Fn(T) -> Fut, Fut, T> Action<T> for F
where
    Fut: Future<Output=Result<(), String>> + Send,
    F: Send + Sync + Copy
{
    fn run(self, trigger: T) -> impl Future<Output=Result<(), String>> {
        self(trigger)
    }
}

impl<'a> Automation<'a> {
    /// Create a new automation
    ///
    /// * `name` is the automation's name, used mostly for tracing
    /// * `input` is the input stream to trigger this automation
    /// * `action` is the action to run for each trigger
    pub fn new<S, A>(name: impl AsRef<str>, input: S, action: A) -> Self
    where
        S: Stream + Send + 'a,
        A: Action<S::Item> + 'a,
    {
        let futures = JobStream::new(name.as_ref().to_string(), input, action);
        Automation(Box::pin(futures))
    }
}

#[pin_project]
struct JobStream<'a, S, A> {
    name: String,
    #[pin]
    input: S,
    action: A,
    _a: PhantomData<&'a ()>,
}

impl<'a, S, A> JobStream<'a, S, A>
where
    S: Stream + 'a,
    A: Action<S::Item> + 'a,
{
    pub fn new(name: String, input: S, action: A) -> Self {
        JobStream {
            name,
            input,
            action,
            _a: PhantomData,
        }
    }
}

impl<'a, S, A> Stream for JobStream<'a, S, A>
where
    S: Stream + 'a,
    A: Action<S::Item> + 'a,
{
    type Item = BoxFuture<'a, ()>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        let action = this.action;
        this.input.poll_next(cx).map(move |option| {
            option.map(move |trigger| {
                let run  = action.run(trigger);
                let name = this.name.clone();
                let future = async move {
                    debug!("Automation {name} triggered");
                    if let Err(error) = run.await {
                        warn!("automation {name} failed: {error}");
                    } else {
                        debug!("Automation {name} completed");
                    }
                };
                Box::pin(future.instrument(tracing::info_span!("automation_run", name = this.name.clone()))) as BoxFuture<'a, ()>
            })
        })
    }
}
