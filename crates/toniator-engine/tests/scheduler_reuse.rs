//! Current private-editor worker reuse and cancellation boundary regression.

use std::{
    fs,
    path::Path,
    thread,
    time::{Duration, Instant},
};
use toniator_domain::{CanvasSpec, Document, DocumentSession, SourceReference, SourceReferenceId};
use toniator_engine::{
    CacheDisposition, EvaluationCompletion, EvaluationProgressStage, EvaluationRequest,
    EvaluationScheduler, ResolvedSource, SourceFormatHint,
};

/// Waits for an actual current completion with a bounded deadline.
///
/// # Panics
/// Panics if the worker disconnects or fails to complete within ten seconds.
fn completion(scheduler: &EvaluationScheduler) -> EvaluationCompletion {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(value) = scheduler
            .try_receive_latest()
            .expect("worker remains connected")
        {
            return value;
        }
        assert!(Instant::now() < deadline, "current evaluation completes");
        thread::sleep(Duration::from_millis(2));
    }
}

/// Proves reusable workers invalidate old tickets and caches without preventing fresh publication.
///
/// # Panics
/// Panics on fixture, scheduler, publication, cache-reset, or bounded completion failure.
#[test]
fn cleared_private_scheduler_rejects_old_work_and_reuses_worker_with_cold_cache() {
    let id = SourceReferenceId::new("private-worker-reuse").expect("valid source ID");
    let session = DocumentSession::new(
        Document::new_default_document(
            CanvasSpec {
                width: 80.0,
                height: 80.0,
            },
            SourceReference::Assigned(id.clone()),
        )
        .expect("document validates"),
    )
    .expect("session validates");
    let source = ResolvedSource::new(
        id,
        fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/raster-sample.png"))
            .expect("immutable source reads"),
        SourceFormatHint::Png,
    )
    .expect("source resolves");
    let request = || EvaluationRequest::new(session.document_evaluation_snapshot(), source.clone());
    let scheduler = EvaluationScheduler::new().expect("worker starts");
    let cancelled = scheduler
        .submit(request())
        .expect("first submission succeeds");
    scheduler.cancel_and_clear();
    assert!(!scheduler.is_latest(cancelled));
    assert!(
        scheduler
            .try_receive_latest()
            .expect("worker stays connected")
            .is_none()
    );
    let next = scheduler
        .submit(request())
        .expect("same worker accepts another submission");
    assert!(next.value() > cancelled.value());
    let accepted = completion(&scheduler);
    assert_eq!(accepted.ticket(), next);
    assert!(accepted.result().is_some(), "{:?}", accepted.error());
    assert!(
        scheduler
            .accept_completion(&accepted, &session)
            .expect("publication validates")
    );
    scheduler.cancel_and_clear();
    assert!(
        !scheduler
            .accept_completion(&accepted, &session)
            .expect("stale publication rejects")
    );
    let fresh = scheduler
        .submit(request())
        .expect("worker remains reusable");
    let cold = completion(&scheduler);
    assert_eq!(cold.ticket(), fresh);
    assert_eq!(
        cold.cache_diagnostics()
            .expect("successful evaluation has diagnostics")
            .aggregate
            .decoded_source,
        CacheDisposition::Miss
    );
    assert!(
        scheduler
            .accept_completion(&cold, &session)
            .expect("fresh publication validates")
    );
    let queued = scheduler
        .submit(request())
        .expect("queued completion case submits");
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if scheduler
            .try_receive_latest_progress()
            .expect("progress remains available")
            .is_some_and(|progress| {
                progress.ticket() == queued && progress.stage() == EvaluationProgressStage::Complete
            })
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "worker reaches terminal publication"
        );
        thread::sleep(Duration::from_millis(2));
    }
    // Complete progress and the result send share the publication gate. Cancellation waits
    // for that send, then drains the unconsumed result instead of retaining its cache payload.
    scheduler.cancel_and_clear();
    assert!(
        scheduler
            .try_receive_latest()
            .expect("completion queue remains usable")
            .is_none()
    );
    assert!(
        scheduler
            .try_receive_latest_progress()
            .expect("progress queue remains usable")
            .is_none()
    );
    scheduler.shutdown().expect("worker stops and joins");
}
