//! Current integrated document-evaluation witnesses.
//!
//! Historical pre-bundle and schema-migration matrices are intentionally absent: current Stage 20
//! authority is exercised here, while focused mechanism, cache, scheduler, rendering, and
//! persistence tests own their respective detailed contracts.

use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};

use toniator_domain::{
    CanvasSpec, ChannelId, DensityMetricDelta2D, Document, DocumentCommand, DocumentHistory,
    DocumentSession, SourceReference, SourceReferenceId,
};
use toniator_engine::{
    CacheDisposition, EvaluationCompletion, EvaluationExecutionClass, EvaluationLimits,
    EvaluationPerformanceStage, EvaluationProfileCache, EvaluationRequest, EvaluationScheduler,
    EvaluationWorkloadKind, ResolvedSource, SourceFormatHint, evaluate,
    evaluate_profiled_cached_with_limits, evaluate_profiled_with_limits,
};

/// Builds one current modeled history with a caller-selected assigned source identity.
fn history(source_id: &str, width: f64, height: f64) -> DocumentHistory {
    let document = Document::new_default_document(
        CanvasSpec { width, height },
        SourceReference::Assigned(SourceReferenceId::new(source_id).expect("source ID validates")),
    )
    .expect("current default document validates");
    DocumentHistory::new(DocumentSession::new(document).expect("current session validates"))
}

/// Builds one complete request from the current document snapshot and immutable source bytes.
fn request(
    history: &DocumentHistory,
    source_id: &str,
    asset: &str,
    format: SourceFormatHint,
) -> EvaluationRequest {
    EvaluationRequest::new(
        history.session().document_evaluation_snapshot(),
        ResolvedSource::new(
            SourceReferenceId::new(source_id).expect("source ID validates"),
            fs::read(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../assets")
                    .join(asset),
            )
            .expect("immutable source reads"),
            format,
        )
        .expect("resolved source validates"),
    )
}

/// Waits for one latest scheduler completion without depending on worker timing.
fn wait_for_latest(scheduler: &EvaluationScheduler) -> EvaluationCompletion {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(completion) = scheduler
            .try_receive_latest()
            .expect("scheduler receive succeeds")
        {
            return completion;
        }
        assert!(Instant::now() < deadline, "evaluation timed out");
        std::thread::yield_now();
    }
}

/// Proves both immutable source formats produce deterministic complete current-authority results.
#[test]
fn immutable_png_and_svg_inputs_evaluate_deterministically() {
    for (source_id, asset, format) in [
        ("integrated-png", "raster-sample.png", SourceFormatHint::Png),
        ("integrated-svg", "vector-sample.svg", SourceFormatHint::Svg),
    ] {
        let history = history(source_id, 180.0, 120.0);
        let first =
            evaluate(request(&history, source_id, asset, format)).expect("first evaluation");
        let second =
            evaluate(request(&history, source_id, asset, format)).expect("second evaluation");
        assert_eq!(first, second);
        assert_eq!(first.channels().len(), 3);
        assert_eq!(
            (first.raster().width(), first.raster().height()),
            (180, 120)
        );
    }
}

/// Proves accepted warm replay reports hits without rewriting local-family diagnostic origin.
#[test]
fn accepted_warm_replay_reports_family_and_output_hits() {
    let history = history("warm-cache", 180.0, 120.0);
    let scheduler = EvaluationScheduler::new().expect("scheduler starts");
    let first_ticket = scheduler
        .submit(request(
            &history,
            "warm-cache",
            "raster-sample.png",
            SourceFormatHint::Png,
        ))
        .expect("first request submits");
    let first = wait_for_latest(&scheduler);
    assert_eq!(first.ticket(), first_ticket);
    assert!(
        scheduler
            .accept_completion(&first, history.session())
            .expect("first acceptance checks")
    );

    let replay_ticket = scheduler
        .submit(request(
            &history,
            "warm-cache",
            "raster-sample.png",
            SourceFormatHint::Png,
        ))
        .expect("replay request submits");
    let replay = wait_for_latest(&scheduler);
    assert_eq!(replay.ticket(), replay_ticket);
    let diagnostics = replay
        .cache_diagnostics()
        .expect("replay diagnostics exist");
    assert!(diagnostics.channels.iter().all(|channel| {
        channel.family == CacheDisposition::Hit
            && channel
                .outputs
                .iter()
                .all(|output| output.realization == CacheDisposition::Hit)
    }));
}

/// Proves profiling observes the ordinary evaluator without changing result identity or semantics.
#[test]
fn profiled_evaluation_matches_ordinary_result_and_reports_workloads() {
    let history = history("profiled", 240.0, 160.0);
    let ordinary = evaluate(request(
        &history,
        "profiled",
        "raster-sample.png",
        SourceFormatHint::Png,
    ))
    .expect("ordinary evaluation succeeds");
    let profiled = evaluate_profiled_with_limits(
        request(
            &history,
            "profiled",
            "raster-sample.png",
            SourceFormatHint::Png,
        ),
        EvaluationLimits::default(),
    )
    .expect("profiled evaluation succeeds");
    assert_eq!(profiled.result, ordinary);
    assert!(profiled.performance.configured_worker_count >= 1);
    assert!(
        profiled
            .performance
            .records
            .iter()
            .any(|record| record.stage == EvaluationPerformanceStage::Total)
    );
    assert!(profiled.performance.records.iter().any(|record| {
        record.workloads.iter().any(|workload| {
            workload.kind == EvaluationWorkloadKind::RasterPixels && workload.count == 240 * 160
        })
    }));
}

/// Proves profiled cold, warm, and one-channel edits expose transactional cache and reuse classes.
#[test]
fn profiled_cache_distinguishes_computation_accepted_hits_and_local_reuse() {
    let mut history = history("profile-cache", 240.0, 160.0);
    let mut cache = EvaluationProfileCache::default();
    let run = |history: &DocumentHistory, cache: &mut EvaluationProfileCache| {
        evaluate_profiled_cached_with_limits(
            request(
                history,
                "profile-cache",
                "raster-sample.png",
                SourceFormatHint::Png,
            ),
            EvaluationLimits::default(),
            cache,
        )
        .expect("profiled cached evaluation succeeds")
    };
    let cold = run(&history, &mut cache);
    assert_eq!(
        cold.diagnostics.aggregate.decoded_source,
        CacheDisposition::Miss
    );
    let cold_family = cold
        .performance
        .records
        .iter()
        .filter(|record| record.stage == EvaluationPerformanceStage::Family)
        .map(|record| record.execution)
        .collect::<Vec<_>>();
    assert_eq!(
        cold_family,
        vec![
            EvaluationExecutionClass::Computed,
            EvaluationExecutionClass::LocalReuse,
            EvaluationExecutionClass::LocalReuse,
        ]
    );

    let warm = run(&history, &mut cache);
    assert_eq!(warm.result, cold.result);
    assert_eq!(
        warm.diagnostics.aggregate.decoded_source,
        CacheDisposition::Hit
    );
    assert_eq!(warm.diagnostics.aggregate.family, CacheDisposition::Hit);
    assert_eq!(
        warm.diagnostics.aggregate.realization,
        CacheDisposition::Hit
    );
    assert_eq!(warm.diagnostics.aggregate.scene, CacheDisposition::Hit);
    assert_eq!(warm.diagnostics.aggregate.raster, CacheDisposition::Hit);

    history
        .apply(&DocumentCommand::SetChannelDensityDelta {
            base: history.document().pattern_settings().clone(),
            channel_id: ChannelId(1),
            density: DensityMetricDelta2D {
                across_x_delta: 1.0,
                across_y_delta: 0.0,
            },
        })
        .expect("red density edit publishes");
    let edited = run(&history, &mut cache);
    assert_eq!(
        edited.diagnostics.aggregate.decoded_source,
        CacheDisposition::Hit
    );
    assert_eq!(
        edited.diagnostics.channels[0].family,
        CacheDisposition::Miss
    );
    assert_eq!(edited.diagnostics.channels[1].family, CacheDisposition::Hit);
    assert_eq!(edited.diagnostics.channels[2].family, CacheDisposition::Hit);
    assert_eq!(
        edited.diagnostics.channels[0].realization,
        CacheDisposition::Miss
    );
    assert_eq!(
        edited.diagnostics.channels[1].realization,
        CacheDisposition::Hit
    );
    assert_eq!(
        edited.diagnostics.channels[2].realization,
        CacheDisposition::Hit
    );
}
