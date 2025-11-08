//! Automations run when a trigger fires and executes some action

use std::marker::PhantomData;
use std::pin::Pin;
use std::task::{Context, Poll};
use futures::future::BoxFuture;
use futures::{Stream};
use futures::stream::BoxStream;
use pin_project::pin_project;

#[must_use = "An automation does nothing unless it is passed into Manager::start"]
/// An Automation definition, with a trigger stream and an action
pub struct Automation<'a>(pub(crate) BoxStream<'a, BoxFuture<'a, Result<(), String>>>);

pub trait Action<Trigger>: Send + Sync + Copy {
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
    pub fn parallel<S, A>(input: S, action: A) -> Self
    where
        S: Stream + Send + 'a,
        A: Action<S::Item> + 'a,
    {
        let futures = JobStream::new(input, action);
        Automation(Box::pin(futures))
    }
}

#[pin_project]
struct JobStream<'a, S, A> {
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
    pub fn new(input: S, action: A) -> Self {
        JobStream {
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
    type Item = BoxFuture<'a, Result<(), String>>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        let action = this.action;
        this.input.poll_next(cx).map(move |option| {
            option.map(move |trigger| {
                Box::pin(action.run(trigger)) as BoxFuture<'a, Result<(), String>>
            })
        })
    }
}
