use std::{
    error::Error,
    fmt,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, Sender, TryRecvError},
    },
    thread::{self, JoinHandle},
};

use toniator_domain::{DocumentSession, EvaluationToken};

use crate::{
    CacheDiagnostics, CacheTransaction, DerivedCache, DerivedCacheSnapshot, EvaluationError,
    EvaluationLimits, EvaluationRequest, EvaluationResult, EvaluationRunError,
    evaluate_cancellable_cached,
};
#[cfg(test)]
use crate::{EvaluationStageGate, evaluate_cancellable_with_gate};

/// A monotonically increasing, checked identifier for one submitted evaluation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EvaluationTicket(u64);

impl EvaluationTicket {
    /// The nonzero numeric value assigned by the scheduler.
    pub const fn value(self) -> u64 {
        self.0
    }
}

/// The only terminal outcomes made visible for the most recent evaluation.
#[derive(Clone, Debug, PartialEq)]
pub enum EvaluationCompletion {
    Completed {
        ticket: EvaluationTicket,
        result: Box<EvaluationResult>,
        cache_diagnostics: CacheDiagnostics,
    },
    Failed {
        ticket: EvaluationTicket,
        token: EvaluationToken,
        error: EvaluationError,
    },
}

impl EvaluationCompletion {
    pub const fn ticket(&self) -> EvaluationTicket {
        match self {
            Self::Completed { ticket, .. } | Self::Failed { ticket, .. } => *ticket,
        }
    }

    pub fn token(&self) -> EvaluationToken {
        match self {
            Self::Completed { result, .. } => result.token(),
            Self::Failed { token, .. } => *token,
        }
    }

    pub fn result(&self) -> Option<&EvaluationResult> {
        match self {
            Self::Completed { result, .. } => Some(result),
            Self::Failed { .. } => None,
        }
    }

    pub fn error(&self) -> Option<&EvaluationError> {
        match self {
            Self::Completed { .. } => None,
            Self::Failed { error, .. } => Some(error),
        }
    }

    /// Successful completions expose immutable reuse diagnostics. Failures
    /// deliberately retain their Stage 7 shape.
    pub fn cache_diagnostics(&self) -> Option<&CacheDiagnostics> {
        match self {
            Self::Completed {
                cache_diagnostics, ..
            } => Some(cache_diagnostics),
            Self::Failed { .. } => None,
        }
    }
}

/// Lifecycle failures for the one-worker scheduler.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SchedulerError {
    WorkerSpawn,
    WorkerUnavailable,
    TicketExhausted,
    WorkerPanicked,
}

impl fmt::Display for SchedulerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WorkerSpawn => formatter.write_str("could not spawn evaluation worker"),
            Self::WorkerUnavailable => formatter.write_str("evaluation worker is unavailable"),
            Self::TicketExhausted => formatter.write_str("evaluation ticket sequence is exhausted"),
            Self::WorkerPanicked => {
                formatter.write_str("evaluation worker panicked during shutdown")
            }
        }
    }
}

impl Error for SchedulerError {}

struct Job {
    ticket: EvaluationTicket,
    request: EvaluationRequest,
    cancelled: Arc<AtomicBool>,
    cache: DerivedCacheSnapshot,
}

struct WorkerCompletion {
    completion: EvaluationCompletion,
    transaction: Option<CacheTransaction>,
}

/// One mutex serializes ticket publication, accepted-cache snapshots, staged
/// transactions, and acceptance commits. No worker mutates this state.
struct SchedulerState {
    next_ticket: Option<u64>,
    sender: Option<Sender<Job>>,
    worker: Option<JoinHandle<()>>,
    latest_cancellation: Option<Arc<AtomicBool>>,
    latest_ticket: Option<EvaluationTicket>,
    cache: DerivedCache,
    pending_transaction: Option<(EvaluationTicket, CacheTransaction)>,
    accepted_ticket: Option<EvaluationTicket>,
}

/// An engine-owned, latest-only evaluator with exactly one worker thread.
pub struct EvaluationScheduler {
    state: Mutex<SchedulerState>,
    latest_ticket: Arc<AtomicU64>,
    shutdown: Arc<AtomicBool>,
    completions: Mutex<Receiver<WorkerCompletion>>,
    #[cfg(test)]
    shutdown_notifier: Option<Sender<()>>,
}

impl EvaluationScheduler {
    /// Starts the scheduler's one engine-owned worker.
    pub fn new() -> Result<Self, SchedulerError> {
        Self::new_with_limits(EvaluationLimits::default())
    }

    /// Starts the scheduler with an immutable family candidate policy.
    pub fn new_with_limits(limits: EvaluationLimits) -> Result<Self, SchedulerError> {
        Self::new_with_next_ticket(limits, 1)
    }

    fn new_with_next_ticket(
        limits: EvaluationLimits,
        next_ticket: u64,
    ) -> Result<Self, SchedulerError> {
        let (sender, receiver) = mpsc::channel();
        let (completion_sender, completions) = mpsc::channel();
        let latest_ticket = Arc::new(AtomicU64::new(0));
        let shutdown = Arc::new(AtomicBool::new(false));
        let worker_latest_ticket = Arc::clone(&latest_ticket);
        let worker_shutdown = Arc::clone(&shutdown);
        let worker = thread::Builder::new()
            .name("toniator-evaluation".into())
            .spawn(move || {
                worker_loop(
                    receiver,
                    completion_sender,
                    worker_latest_ticket,
                    worker_shutdown,
                    limits,
                )
            })
            .map_err(|_| SchedulerError::WorkerSpawn)?;
        Ok(Self {
            state: Mutex::new(SchedulerState {
                next_ticket: Some(next_ticket),
                sender: Some(sender),
                worker: Some(worker),
                latest_cancellation: None,
                latest_ticket: None,
                cache: DerivedCache::default(),
                pending_transaction: None,
                accepted_ticket: None,
            }),
            latest_ticket,
            shutdown,
            completions: Mutex::new(completions),
            #[cfg(test)]
            shutdown_notifier: None,
        })
    }

    #[cfg(test)]
    fn new_with_stage_gate(
        gate: Arc<EvaluationStageGate>,
        shutdown_notifier: Sender<()>,
    ) -> Result<Self, SchedulerError> {
        let (sender, receiver) = mpsc::channel();
        let (completion_sender, completions) = mpsc::channel();
        let latest_ticket = Arc::new(AtomicU64::new(0));
        let shutdown = Arc::new(AtomicBool::new(false));
        let worker_latest_ticket = Arc::clone(&latest_ticket);
        let worker_shutdown = Arc::clone(&shutdown);
        let worker = thread::Builder::new()
            .name("toniator-evaluation-test".into())
            .spawn(move || {
                worker_loop_with(
                    receiver,
                    completion_sender,
                    worker_latest_ticket,
                    worker_shutdown,
                    EvaluationLimits::default(),
                    |request, cancelled, _cache, _limits| {
                        evaluate_cancellable_with_gate(request, cancelled, &gate).map(|result| {
                            crate::CachedEvaluation {
                                result,
                                diagnostics: CacheDiagnostics {
                                    decoded_source: crate::CacheDisposition::Miss,
                                    family: crate::CacheDisposition::Miss,
                                    realization: crate::CacheDisposition::Miss,
                                    scene: crate::CacheDisposition::Miss,
                                    raster: crate::CacheDisposition::Miss,
                                },
                                transaction: CacheTransaction::default(),
                            }
                        })
                    },
                )
            })
            .map_err(|_| SchedulerError::WorkerSpawn)?;
        Ok(Self {
            state: Mutex::new(SchedulerState {
                next_ticket: Some(1),
                sender: Some(sender),
                worker: Some(worker),
                latest_cancellation: None,
                latest_ticket: None,
                cache: DerivedCache::default(),
                pending_transaction: None,
                accepted_ticket: None,
            }),
            latest_ticket,
            shutdown,
            completions: Mutex::new(completions),
            shutdown_notifier: Some(shutdown_notifier),
        })
    }

    /// Queues immutable work and returns its checked monotonic ticket.
    pub fn submit(&self, request: EvaluationRequest) -> Result<EvaluationTicket, SchedulerError> {
        let mut state = self
            .state
            .lock()
            .expect("evaluation scheduler state lock poisoned");
        if self.shutdown.load(Ordering::Acquire) {
            return Err(SchedulerError::WorkerUnavailable);
        }
        // One lock owns the accepted cache, latest ticket, and outstanding
        // transaction. Snapshot only after acquiring it, so a new job cannot
        // observe cache state from before a concurrent accepted completion.
        let cache = state.cache.snapshot();
        let ticket = take_next_ticket(&mut state.next_ticket)?;
        if let Some(previous) = &state.latest_cancellation {
            previous.store(true, Ordering::Release);
        }
        state.pending_transaction = None;
        state.accepted_ticket = None;
        let cancelled = Arc::new(AtomicBool::new(false));
        state.latest_cancellation = Some(Arc::clone(&cancelled));
        state.latest_ticket = Some(ticket);
        self.latest_ticket.store(ticket.value(), Ordering::Release);
        let sender = state
            .sender
            .as_ref()
            .ok_or(SchedulerError::WorkerUnavailable)?;
        sender
            .send(Job {
                ticket,
                request,
                cancelled,
                cache,
            })
            .map_err(|_| SchedulerError::WorkerUnavailable)?;
        Ok(ticket)
    }

    /// Returns the newest still-current terminal completion without blocking.
    pub fn try_receive_latest(&self) -> Result<Option<EvaluationCompletion>, SchedulerError> {
        let receiver = self
            .completions
            .lock()
            .expect("evaluation completion lock poisoned");
        let mut current = None;
        loop {
            match receiver.try_recv() {
                Ok(worker_completion) => {
                    let mut state = self
                        .state
                        .lock()
                        .expect("evaluation scheduler state lock poisoned");
                    if !self.shutdown.load(Ordering::Acquire)
                        && state.latest_ticket == Some(worker_completion.completion.ticket())
                    {
                        if let Some(transaction) = worker_completion.transaction {
                            state.pending_transaction =
                                Some((worker_completion.completion.ticket(), transaction));
                        }
                        current = Some(worker_completion.completion);
                    }
                }
                Err(TryRecvError::Empty) => return Ok(current),
                Err(TryRecvError::Disconnected) => {
                    return if self.shutdown.load(Ordering::Acquire) {
                        Ok(current)
                    } else {
                        Err(SchedulerError::WorkerUnavailable)
                    };
                }
            }
        }
    }

    /// Whether this ticket remains the scheduler's newest submission.
    pub fn is_latest(&self, ticket: EvaluationTicket) -> bool {
        !self.shutdown.load(Ordering::Acquire)
            && self
                .state
                .lock()
                .expect("evaluation scheduler state lock poisoned")
                .latest_ticket
                == Some(ticket)
    }

    /// Accepts a currently presentable completion. A successful completion
    /// atomically installs only its own staged derived values; current failures
    /// are accepted as terminal reporting without changing the cache.
    pub fn accept_completion(
        &self,
        completion: &EvaluationCompletion,
        session: &DocumentSession,
    ) -> Result<bool, SchedulerError> {
        let mut state = self
            .state
            .lock()
            .expect("evaluation scheduler state lock poisoned");
        if self.shutdown.load(Ordering::Acquire)
            || state.latest_ticket != Some(completion.ticket())
            || !session.accepts_evaluation(completion.token())
        {
            return Ok(false);
        }
        if state.accepted_ticket == Some(completion.ticket()) {
            return Ok(true);
        }
        if completion.result().is_some()
            && let Some((ticket, transaction)) = state.pending_transaction.take()
            && ticket == completion.ticket()
        {
            state.cache.commit(transaction);
        }
        state.accepted_ticket = Some(completion.ticket());
        Ok(true)
    }

    /// Cancels outstanding work, signals the worker, and joins it.
    pub fn shutdown(mut self) -> Result<(), SchedulerError> {
        self.stop_worker()
    }

    fn stop_worker(&mut self) -> Result<(), SchedulerError> {
        let worker = {
            let mut state = self
                .state
                .lock()
                .expect("evaluation scheduler state lock poisoned");
            self.shutdown.store(true, Ordering::Release);
            if let Some(cancelled) = &state.latest_cancellation {
                cancelled.store(true, Ordering::Release);
            }
            state.sender.take();
            state.worker.take()
        };
        #[cfg(test)]
        if let Some(notifier) = self.shutdown_notifier.take() {
            let _ = notifier.send(());
        }
        if let Some(worker) = worker {
            worker.join().map_err(|_| SchedulerError::WorkerPanicked)?;
        }
        Ok(())
    }
}

impl Drop for EvaluationScheduler {
    fn drop(&mut self) {
        let _ = self.stop_worker();
    }
}

fn worker_loop(
    receiver: Receiver<Job>,
    completion_sender: Sender<WorkerCompletion>,
    latest_ticket: Arc<AtomicU64>,
    shutdown: Arc<AtomicBool>,
    limits: EvaluationLimits,
) {
    worker_loop_with(
        receiver,
        completion_sender,
        latest_ticket,
        shutdown,
        limits,
        evaluate_cancellable_cached,
    );
}

fn worker_loop_with<F>(
    receiver: Receiver<Job>,
    completion_sender: Sender<WorkerCompletion>,
    latest_ticket: Arc<AtomicU64>,
    shutdown: Arc<AtomicBool>,
    limits: EvaluationLimits,
    mut evaluate: F,
) where
    F: FnMut(
        EvaluationRequest,
        &AtomicBool,
        DerivedCacheSnapshot,
        EvaluationLimits,
    ) -> Result<crate::CachedEvaluation, EvaluationRunError>,
{
    while !shutdown.load(Ordering::Acquire) {
        let mut job = match receiver.recv() {
            Ok(job) => job,
            Err(_) => return,
        };
        job = drain_latest(&receiver, job);
        if !is_current(job.ticket, &job.cancelled, &latest_ticket, &shutdown) {
            continue;
        }
        let token = job.request.token();
        let ticket = job.ticket;
        let cancelled = Arc::clone(&job.cancelled);
        let completion = match evaluate(job.request, &cancelled, job.cache, limits) {
            Ok(result) => WorkerCompletion {
                completion: EvaluationCompletion::Completed {
                    ticket,
                    result: Box::new(result.result),
                    cache_diagnostics: result.diagnostics,
                },
                transaction: Some(result.transaction),
            },
            Err(EvaluationRunError::Evaluation(error)) => WorkerCompletion {
                completion: EvaluationCompletion::Failed {
                    ticket,
                    token,
                    error,
                },
                transaction: None,
            },
            Err(EvaluationRunError::Cancelled) => continue,
        };
        if is_current(ticket, &cancelled, &latest_ticket, &shutdown) {
            let _ = completion_sender.send(completion);
        }
    }
}

fn is_current(
    ticket: EvaluationTicket,
    cancelled: &AtomicBool,
    latest_ticket: &AtomicU64,
    shutdown: &AtomicBool,
) -> bool {
    !shutdown.load(Ordering::Acquire)
        && !cancelled.load(Ordering::Acquire)
        && latest_ticket.load(Ordering::Acquire) == ticket.value()
}

fn take_next_ticket(next_ticket: &mut Option<u64>) -> Result<EvaluationTicket, SchedulerError> {
    let value = next_ticket.take().ok_or(SchedulerError::TicketExhausted)?;
    *next_ticket = value.checked_add(1);
    Ok(EvaluationTicket(value))
}

fn drain_latest<T>(receiver: &Receiver<T>, mut newest: T) -> T {
    while let Ok(next) = receiver.try_recv() {
        newest = next;
    }
    newest
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{Arc, Barrier, Condvar, Mutex, mpsc},
        thread,
        time::{Duration, Instant},
    };

    use super::*;
    use crate::{EvaluationCheckpoint, EvaluationStage, EvaluationStageGate, test_support};

    const GUARD: Duration = Duration::from_secs(15);

    fn wait_for_latest(scheduler: &EvaluationScheduler) -> EvaluationCompletion {
        let deadline = Instant::now() + GUARD;
        loop {
            match scheduler.try_receive_latest().unwrap() {
                Some(completion) => return completion,
                None if Instant::now() < deadline => thread::yield_now(),
                None => panic!("evaluation worker did not complete before hang guard"),
            }
        }
    }

    #[test]
    fn checked_ticket_exhaustion_is_reported_without_panicking() {
        let mut next_ticket = Some(u64::MAX);
        assert_eq!(
            take_next_ticket(&mut next_ticket).unwrap().value(),
            u64::MAX
        );
        assert_eq!(
            take_next_ticket(&mut next_ticket),
            Err(SchedulerError::TicketExhausted)
        );
    }

    #[test]
    fn drain_coalesces_queued_jobs_to_the_newest_ticket_without_sleeping() {
        let barrier = Arc::new(Barrier::new(2));
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        let (sender, receiver) = mpsc::channel();
        let (result_sender, result_receiver) = mpsc::channel();
        let worker_barrier = Arc::clone(&barrier);
        let worker_gate = Arc::clone(&gate);
        let worker = thread::spawn(move || {
            let active = receiver.recv().unwrap();
            worker_barrier.wait();
            let (lock, wake) = &*worker_gate;
            let mut released = lock.lock().unwrap();
            while !*released {
                released = wake.wait(released).unwrap();
            }
            result_sender.send(drain_latest(&receiver, active)).unwrap();
        });
        sender.send(1_u64).unwrap();
        barrier.wait();
        sender.send(2_u64).unwrap();
        sender.send(3_u64).unwrap();
        let (lock, wake) = &*gate;
        *lock.lock().unwrap() = true;
        wake.notify_one();
        assert_eq!(
            result_receiver
                .recv_timeout(std::time::Duration::from_secs(1))
                .unwrap(),
            3
        );
        worker.join().unwrap();
    }

    #[test]
    fn active_evaluation_is_cancelled_and_only_the_newest_queued_ticket_completes() {
        let (gate, entered) =
            EvaluationStageGate::new(EvaluationStage::Family, EvaluationCheckpoint::Before);
        let (shutdown_sender, _shutdown_receiver) = mpsc::channel();
        let scheduler =
            EvaluationScheduler::new_with_stage_gate(Arc::clone(&gate), shutdown_sender).unwrap();
        let first = scheduler.submit(test_support::request()).unwrap();
        entered.recv_timeout(GUARD).unwrap();
        let second = scheduler.submit(test_support::request()).unwrap();
        let newest = scheduler.submit(test_support::request()).unwrap();
        assert!(first < second && second < newest);
        gate.release();

        let completion = wait_for_latest(&scheduler);
        assert_eq!(completion.ticket(), newest);
        assert_eq!(scheduler.try_receive_latest().unwrap(), None);
        scheduler.shutdown().unwrap();
    }

    #[test]
    fn explicit_shutdown_cancels_gated_active_work_before_joining() {
        let (gate, entered) =
            EvaluationStageGate::new(EvaluationStage::Family, EvaluationCheckpoint::Before);
        let (shutdown_sender, shutdown_receiver) = mpsc::channel();
        let scheduler =
            EvaluationScheduler::new_with_stage_gate(Arc::clone(&gate), shutdown_sender).unwrap();
        scheduler.submit(test_support::request()).unwrap();
        entered.recv_timeout(GUARD).unwrap();

        let (done_sender, done_receiver) = mpsc::channel();
        thread::spawn(move || done_sender.send(scheduler.shutdown()).unwrap());
        shutdown_receiver.recv_timeout(GUARD).unwrap();
        gate.release();
        assert_eq!(done_receiver.recv_timeout(GUARD).unwrap(), Ok(()));
    }

    #[test]
    fn drop_cancels_gated_active_work_before_joining() {
        let (gate, entered) =
            EvaluationStageGate::new(EvaluationStage::Family, EvaluationCheckpoint::Before);
        let (shutdown_sender, shutdown_receiver) = mpsc::channel();
        let scheduler =
            EvaluationScheduler::new_with_stage_gate(Arc::clone(&gate), shutdown_sender).unwrap();
        scheduler.submit(test_support::request()).unwrap();
        entered.recv_timeout(GUARD).unwrap();

        let (done_sender, done_receiver) = mpsc::channel();
        thread::spawn(move || {
            drop(scheduler);
            done_sender.send(()).unwrap();
        });
        shutdown_receiver.recv_timeout(GUARD).unwrap();
        gate.release();
        done_receiver.recv_timeout(GUARD).unwrap();
    }
}
