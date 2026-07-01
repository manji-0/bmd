//! Bounded thread pool shared by background read/render workers.

use std::sync::{Arc, mpsc};
use std::thread;

const MAX_BACKGROUND_WORKERS: usize = 4;

/// Runs submitted jobs on a fixed set of worker threads.
pub(crate) struct WorkerPool {
    sender: mpsc::Sender<Box<dyn FnOnce() + Send>>,
}

impl WorkerPool {
    pub(crate) fn new(workers: usize) -> Arc<Self> {
        let workers = workers.max(1);
        let (sender, receiver) = mpsc::channel::<Box<dyn FnOnce() + Send>>();
        let receiver = Arc::new(std::sync::Mutex::new(receiver));
        for _ in 0..workers {
            let receiver = Arc::clone(&receiver);
            thread::spawn(move || {
                loop {
                    let job = receiver
                        .lock()
                        .expect("worker pool receiver poisoned")
                        .recv();
                    match job {
                        Ok(job) => job(),
                        Err(_) => break,
                    }
                }
            });
        }
        Arc::new(Self { sender })
    }

    pub(crate) fn shared() -> Arc<Self> {
        Self::new(MAX_BACKGROUND_WORKERS)
    }

    pub(crate) fn spawn<F>(&self, job: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let _ = self.sender.send(Box::new(job));
    }
}
