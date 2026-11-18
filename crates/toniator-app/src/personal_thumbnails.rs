//! Bounded background canonical thumbnail work; GTK owns catalog identity and texture publication.

use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Sender, SyncSender},
    },
    thread,
};
use toniator_engine::{EvaluationLimits, EvaluationRequest, RasterSurface};

/// Retains one immutable preparation snapshot and its caller-owned cancellation/result boundary.
pub(crate) struct Job {
    prepare: Box<dyn FnOnce() -> Option<EvaluationRequest> + Send>,
    cancelled: Arc<AtomicBool>,
    result: Sender<Result<RasterSurface, String>>,
}

impl Job {
    /// Captures data-only preparation without doing canonical work on the caller's thread.
    pub(crate) fn new(
        prepare: impl FnOnce() -> Option<EvaluationRequest> + Send + 'static,
        cancelled: Arc<AtomicBool>,
        result: Sender<Result<RasterSurface, String>>,
    ) -> Self {
        Self {
            prepare: Box::new(prepare),
            cancelled,
            result,
        }
    }
}

/// Runs one canonical evaluation at a time with at most one additional queued job.
/// Additional callers retain their own small immutable snapshots until backpressure clears.
pub(crate) struct Worker {
    sender: SyncSender<Job>,
    stopped: Arc<AtomicBool>,
    active: Arc<Mutex<Option<Arc<AtomicBool>>>>,
}

impl Worker {
    /// Starts the sole worker; shutdown cancels the current request and rejects queued work.
    ///
    /// # Errors
    /// Returns the operating system thread-start error without exposing a partial worker.
    pub(crate) fn new() -> std::io::Result<Self> {
        let (sender, receiver) = mpsc::sync_channel::<Job>(1);
        let stopped = Arc::new(AtomicBool::new(false));
        let active = Arc::new(Mutex::new(None));
        let worker_stopped = stopped.clone();
        let worker_active = active.clone();
        thread::Builder::new()
            .name("personal-thumbnails".into())
            .spawn(move || {
                while let Ok(job) = receiver.recv() {
                    {
                        let mut active = worker_active
                            .lock()
                            .unwrap_or_else(|error| error.into_inner());
                        if worker_stopped.load(Ordering::Acquire) {
                            break;
                        }
                        *active = Some(job.cancelled.clone());
                    }
                    if !job.cancelled.load(Ordering::Acquire) {
                        let result = (job.prepare)()
                            .ok_or_else(|| "Thumbnail request is unavailable.".into())
                            .and_then(|request| {
                                toniator_engine::evaluate_cancellable_with_limits(
                                    request,
                                    EvaluationLimits::default(),
                                    &job.cancelled,
                                )
                                .map(|result| result.raster().clone())
                                .map_err(|error| format!("{error:?}"))
                            });
                        if !job.cancelled.load(Ordering::Acquire) {
                            let _ = job.result.send(result);
                        }
                    }
                    *worker_active
                        .lock()
                        .unwrap_or_else(|error| error.into_inner()) = None;
                }
            })?;
        Ok(Self {
            sender,
            stopped,
            active,
        })
    }

    /// Returns a bounded submission endpoint; a full queue leaves the caller's job unconsumed.
    pub(crate) fn sender(&self) -> SyncSender<Job> {
        self.sender.clone()
    }
}

impl Drop for Worker {
    /// Cancels the active canonical evaluation without blocking GTK on a thread join.
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Release);
        if let Some(active) = self
            .active
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_ref()
        {
            active.store(true, Ordering::Release);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Checks canonical preparation runs off-thread, queue capacity stays one, and cancelled queued work never prepares.
    /// # Panics
    /// Panics on unbounded admission, wrong thread ownership, lost cancellation, or worker timeout.
    #[test]
    fn bounded_worker_defers_preparation_and_skips_cancelled_queue() {
        let worker = Worker::new().unwrap();
        let sender = worker.sender();
        let (entered, entered_rx) = mpsc::channel();
        let (release, release_rx) = mpsc::channel();
        let (result, result_rx) = mpsc::channel();
        let gtk_thread = thread::current().id();
        let first = Job::new(
            move || {
                entered.send(thread::current().id()).unwrap();
                release_rx.recv_timeout(Duration::from_secs(3)).unwrap();
                None
            },
            Arc::new(AtomicBool::new(false)),
            result.clone(),
        );
        assert!(sender.try_send(first).is_ok());
        assert_ne!(
            entered_rx.recv_timeout(Duration::from_secs(3)).unwrap(),
            gtk_thread
        );
        let cancelled = Arc::new(AtomicBool::new(true));
        let prepared_cancelled = Arc::new(AtomicBool::new(false));
        let prepared_flag = prepared_cancelled.clone();
        let second = Job::new(
            move || {
                prepared_flag.store(true, Ordering::Release);
                None
            },
            cancelled,
            result.clone(),
        );
        assert!(sender.try_send(second).is_ok());
        let third = Job::new(|| None, Arc::new(AtomicBool::new(false)), result.clone());
        assert!(matches!(
            sender.try_send(third),
            Err(mpsc::TrySendError::Full(_))
        ));
        release.send(()).unwrap();
        assert!(
            result_rx
                .recv_timeout(Duration::from_secs(3))
                .unwrap()
                .is_err()
        );
        assert!(
            sender
                .send(Job::new(|| None, Arc::new(AtomicBool::new(false)), result))
                .is_ok()
        );
        assert!(
            result_rx
                .recv_timeout(Duration::from_secs(3))
                .unwrap()
                .is_err()
        );
        assert!(!prepared_cancelled.load(Ordering::Acquire));
        drop(worker);
    }
}
