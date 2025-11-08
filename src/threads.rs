use async_task::{Runnable};
use flume::{Receiver, SendError, bounded};
use flume::{Sender, TryRecvError};
use futures::executor::block_on;
use futures::future::{BoxFuture, poll_immediate};
use std::ops::Add;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use thread::ScopedJoinHandle;
use tracing::warn;

type Job<'a> = BoxFuture<'a, Result<(), String>>;

pub fn new_thread_pool<'scope, 'env>(
    scope: &'scope thread::Scope<'scope, 'env>,
    thread_count: u8,
    thread_name: &str,
) -> ThreadPool<'scope> {
    let requests = bounded::<Job>(10);
    let abort = Arc::new(AtomicBool::new(false));
    let handles = {
        let requests = requests.1;
        (0..thread_count)
            .map(|i| {
                let requests = requests.clone();
                thread::Builder::new()
                    .name(format!("{thread_name}-{i}"))
                    .spawn_scoped(scope, worker(requests, abort.clone()))
                    .expect("Failed to spawn thread")
            })
            .collect()
    };
    let requests = requests.0;
    ThreadPool { handles, requests, abort }
}

fn worker<'scope>(requests: Receiver<Job<'scope>>, abort: Arc<AtomicBool>) -> impl FnOnce() + Send + 'scope {
    move || {
        let (sender, receiver) = flume::unbounded();
        let mut tasks = Vec::new();
        let schedule = move |runnable: Runnable| match sender.send(runnable) {
            Ok(()) => {}
            Err(SendError(_)) => {
                panic!("executor stopped unexpectedly")
            }
        };
        loop {
            if abort.load(Ordering::Relaxed) {
                return
            }
            tasks = tasks
                .into_iter()
                .filter_map(|mut task| {
                    if let Some(result) = block_on(poll_immediate(&mut task)) {
                        if let Err(error) = result {
                            warn!("automation returned error: {error}")
                        }
                        None
                    } else {
                        Some(task)
                    }
                })
                .collect();
            match receiver.try_recv() {
                Ok(runnable) => {
                    // run the task
                    runnable.run();
                    // continue to the next task
                    continue;
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => break,
            }
            // only if no task is ready do we try to add more to the workload
            match requests.try_recv() {
                Ok(future) => {
                    let (runnable, task) = unsafe {
                        // SAFETY: The Future here and the schedule function do not borrow anything beyond 'scope
                        // this task will be run within a scoped thread and can safely trust 'scope to remain valid
                        async_task::spawn_unchecked(future, schedule.clone())
                    };
                    tasks.push(task);
                    runnable.schedule();
                }
                Err(TryRecvError::Empty) => continue,
                Err(TryRecvError::Disconnected) => {
                    continue
                }
            };
        }
    }
}

pub struct ThreadPool<'a> {
    handles: Vec<ScopedJoinHandle<'a, ()>>,
    abort: Arc<AtomicBool>,
    requests: Sender<Job<'a>>,
}

impl<'a> ThreadPool<'a> {
    pub fn cancel(self, timeout: Duration) {
        drop(self.requests);
        let deadline = Instant::now().add(timeout);
        let mut handles = self.handles;
        while Instant::now() < deadline {
            handles = handles.into_iter().filter_map(|handle| {
                if handle.is_finished() {
                    handle.join().expect("failed to join thread");
                    None
                } else {
                    Some(handle)
                }
            }).collect();
        }
        self.abort.store(true, Ordering::SeqCst);
        for handle in handles {
            handle.join().expect("failed to join thread");
        }
    }
}

impl<'a> ThreadPool<'a> {
    pub fn execute(&self, future: BoxFuture<'a, Result<(), String>>) {
        self.requests
            .send(future)
            .expect("Failed to add future to thread pool");
    }
}
