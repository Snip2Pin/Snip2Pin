use super::Snapshot;
use crate::i18n::{Message, Text};
use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicBool, Ordering},
};

struct Job {
    id: u64,
    origin: Option<[f32; 2]>,
    cancelled: Arc<AtomicBool>,
}

#[derive(Default)]
struct Queue {
    pending: Option<Job>,
    active: Option<Arc<AtomicBool>>,
    stopped: bool,
}

/// One active OS capture and one replaceable pending request. Cancelling never
/// waits on the OS, and a late result cannot make selection reappear.
pub struct Worker {
    shared: Arc<(Mutex<Queue>, Condvar)>,
}

impl Worker {
    pub fn new(
        mut capture: impl FnMut(Option<[f32; 2]>, &AtomicBool) -> Result<Vec<Snapshot>, Message>
        + Send
        + 'static,
        deliver: impl Fn(u64, Result<Vec<Snapshot>, Message>) + Send + 'static,
    ) -> Result<Self, Message> {
        let shared = Arc::new((Mutex::new(Queue::default()), Condvar::new()));
        let thread_state = shared.clone();
        std::thread::Builder::new()
            .name("snip2pin-capture".into())
            .spawn(move || {
                let (queue, ready) = &*thread_state;
                loop {
                    let job = {
                        let Ok(mut state) = queue.lock() else { return };
                        while state.pending.is_none() && !state.stopped {
                            let Ok(next) = ready.wait(state) else { return };
                            state = next;
                        }
                        if state.stopped {
                            return;
                        }
                        let Some(job) = state.pending.take() else {
                            continue;
                        };
                        state.active = Some(job.cancelled.clone());
                        job
                    };
                    if !job.cancelled.load(Ordering::Acquire) {
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            capture(job.origin, &job.cancelled)
                        }))
                        .unwrap_or_else(|_| Err(Text::CaptureBackendCrashed.into()));
                        if !job.cancelled.load(Ordering::Acquire) {
                            deliver(job.id, result);
                        }
                    }
                    if let Ok(mut state) = queue.lock() {
                        state.active = None;
                    }
                }
            })
            .map_err(|error| error.to_string())?;
        Ok(Self { shared })
    }

    pub fn request(&self, id: u64, origin: Option<[f32; 2]>) -> Result<(), Message> {
        let (queue, ready) = &*self.shared;
        let mut state = queue.lock().map_err(|_| Text::CaptureQueueError)?;
        state.pending = Some(Job {
            id,
            origin,
            cancelled: Arc::new(AtomicBool::new(false)),
        });
        ready.notify_one();
        Ok(())
    }

    pub fn cancel(&self) {
        let (queue, _) = &*self.shared;
        if let Ok(mut state) = queue.lock() {
            state.pending = None;
            if let Some(active) = &state.active {
                active.store(true, Ordering::Release);
            }
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.cancel();
        let (queue, ready) = &*self.shared;
        if let Ok(mut state) = queue.lock() {
            state.stopped = true;
        }
        ready.notify_one();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{atomic::AtomicUsize, mpsc};
    use std::time::Duration;

    #[test]
    fn cancellation_drops_late_results_and_only_the_latest_pending_request_runs() {
        let (started, running) = mpsc::channel();
        let (release, wait) = mpsc::channel();
        let (deliver, results) = mpsc::channel();
        let calls = Arc::new(AtomicUsize::new(0));
        let count = calls.clone();
        let worker = Worker::new(
            move |_, _| {
                if count.fetch_add(1, Ordering::SeqCst) == 0 {
                    started.send(()).expect("started");
                    wait.recv().expect("release");
                }
                Ok(Vec::new())
            },
            move |id, _| {
                deliver.send(id).expect("result");
            },
        )
        .expect("worker");
        worker.request(1, None).expect("request");
        running
            .recv_timeout(Duration::from_secs(5))
            .expect("capture started");
        worker.cancel();
        worker.request(2, None).expect("queued");
        worker.request(3, None).expect("replaces queued request");
        release.send(()).expect("release");
        assert_eq!(
            results
                .recv_timeout(Duration::from_secs(5))
                .expect("latest result"),
            3
        );
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert!(results.try_recv().is_err());
    }
}
