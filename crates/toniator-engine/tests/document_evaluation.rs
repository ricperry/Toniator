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
    CanvasSpec, ChannelId, DensityEditedField, DensityMetricDelta2D, Document, DocumentCommand,
    DocumentHistory, DocumentSession, SourceReference, SourceReferenceId,
};
use toniator_engine::{
    CacheDisposition, EvaluationCompletion, EvaluationExecutionClass, EvaluationLimits,
    EvaluationPerformanceStage, EvaluationProfileCache, EvaluationRequest, EvaluationScheduler,
    EvaluationWorkloadKind, ResolvedSource, SourceFormatHint, evaluate,
    evaluate_profiled_cached_with_limits, evaluate_profiled_with_limits,
};
use toniator_patterns::PresetRegistry;
use toniator_render::GeometryOutput;

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

/// Returns the bounds aspect of the retained region whose bounds center is nearest the canvas center.
///
/// # Panics
///
/// Panics when an expected region recipe publishes no positive finite canonical region.
fn center_region_bounds_aspect(result: &toniator_engine::EvaluationResult) -> f64 {
    let center = (
        result.scene().canvas().width / 2.0,
        result.scene().canvas().height / 2.0,
    );
    result
        .scene()
        .layers()
        .iter()
        .flat_map(|layer| layer.outputs())
        .filter_map(|output| match output.geometry() {
            GeometryOutput::CanonicalRegions(regions) => Some(regions.regions()),
            _ => None,
        })
        .flatten()
        .min_by(|left, right| {
            let left_x = (left.bounds.min.x + left.bounds.max.x) / 2.0 - center.0;
            let left_y = (left.bounds.min.y + left.bounds.max.y) / 2.0 - center.1;
            let right_x = (right.bounds.min.x + right.bounds.max.x) / 2.0 - center.0;
            let right_y = (right.bounds.min.y + right.bounds.max.y) / 2.0 - center.1;
            left_x
                .mul_add(left_x, left_y * left_y)
                .total_cmp(&right_x.mul_add(right_x, right_y * right_y))
        })
        .map(|region| {
            let width = region.bounds.max.x - region.bounds.min.x;
            let height = region.bounds.max.y - region.bounds.min.y;
            assert!(width.is_finite() && width > 0.0);
            assert!(height.is_finite() && height > 0.0);
            width / height
        })
        .expect("region recipe publishes a center-near canonical region")
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

/// Proves the bundled connection recipe selected by the Stage 21A dropdown
/// evaluates at the immutable PNG's intrinsic default density after applying to ALL.
#[test]
fn clustered_connections_evaluate_at_intrinsic_png_default_density() {
    let mut history = history("stage21a-clustered", 1024.0, 1024.0);
    PresetRegistry::bundled()
        .apply_to_document_base(&mut history, "clustered-dispersion-random-links")
        .expect("clustered connection recipe applies to document base");
    let result = evaluate(request(
        &history,
        "stage21a-clustered",
        "raster-sample.png",
        SourceFormatHint::Png,
    ))
    .expect("clustered connections evaluate at the intrinsic PNG default density");
    assert_eq!(result.channels().len(), 3);
    assert!(result.scene().layers().iter().any(|layer| {
        layer
            .outputs()
            .iter()
            .any(|output| matches!(output.geometry(), GeometryOutput::CanonicalStrokes(_)))
    }));
}

/// Applies one bundled recipe at density 128 and requires complete canonical evaluation.
///
/// The helper uses the immutable intrinsic PNG and ordinary default limits so
/// failures reproduce the main-window recomputation boundary without a
/// frontend runtime, custom limits, cache reuse, or partial output publication.
fn assert_bundled_recipe_evaluates_after_density_increase(record: &toniator_domain::PresetRecord) {
    let registry = PresetRegistry::bundled();
    let source_id = format!("stage21a-density-{}", record.metadata.id);
    let mut history = history(&source_id, 1024.0, 1024.0);
    registry
        .apply_to_document_base(&mut history, &record.metadata.id)
        .unwrap_or_else(|error| {
            panic!(
                "{} applies before density editing: {error}",
                record.metadata.id
            )
        });
    let command = history
        .document()
        .set_document_density_field(DensityEditedField::Density, 128.0)
        .unwrap_or_else(|error| {
            panic!("{} accepts increased density: {error}", record.metadata.id)
        });
    history.apply(&command).unwrap_or_else(|error| {
        panic!(
            "{} publishes increased density: {error}",
            record.metadata.id
        )
    });
    let result = evaluate(request(
        &history,
        &source_id,
        "raster-sample.png",
        SourceFormatHint::Png,
    ))
    .unwrap_or_else(|error| {
        panic!(
            "{} evaluates after density increase: {error}",
            record.metadata.id
        )
    });
    assert_eq!(result.channels().len(), 3, "{}", record.metadata.id);
    assert!(
        result
            .scene()
            .layers()
            .iter()
            .any(|layer| !layer.outputs().is_empty()),
        "{} publishes canonical output",
        record.metadata.id
    );
}

/// Proves every bundled Stage 21B registry recipe recomputes canonical output after a
/// document-base density increase on the immutable intrinsic PNG.
#[test]
fn every_bundled_recipe_evaluates_after_density_increase() {
    let registry = PresetRegistry::bundled();
    assert_eq!(registry.entries().len(), 17);
    for record in registry.entries() {
        assert_bundled_recipe_evaluates_after_density_increase(record);
    }
}

/// Reproduces the dense three-guide cells acceptance case with default limits.
#[test]
fn three_guide_cells_evaluate_after_density_increase() {
    let registry = PresetRegistry::bundled();
    let record = registry
        .entries()
        .iter()
        .find(|record| record.metadata.id == "three-guide-cells-scale")
        .expect("bundled three-guide cells preset");
    assert_bundled_recipe_evaluates_after_density_increase(record);
}

/// Proves both Guide Faces presets publish canonical regions after ordinary pattern rotation.
///
/// The test crosses both immutable source decoders with non-cardinal angles and includes the
/// intrinsic PNG's combined zoom-out/rotation regression. It exercises the engine-owned structural
/// path handoff, face traversal, sampling, and canonical scene boundary; no frontend preview state
/// participates in this correctness authority.
#[test]
fn rotated_guide_face_presets_publish_regions_for_png_and_svg_sources() {
    let registry = PresetRegistry::bundled();
    for (source_id, asset, format, preset, rotation, density, canvas) in [
        (
            "rotated-guide-faces-png",
            "raster-sample.png",
            SourceFormatHint::Png,
            "three-guide-cells-scale",
            17.0,
            100.0,
            256.0,
        ),
        (
            "rotated-guide-faces-svg",
            "vector-sample.svg",
            SourceFormatHint::Svg,
            "two-guide-cells-uniform-offset",
            89.5,
            100.0,
            256.0,
        ),
        (
            "zoomed-rotated-guide-faces-png",
            "raster-sample.png",
            SourceFormatHint::Png,
            "three-guide-cells-scale",
            17.0,
            125.0,
            1024.0,
        ),
    ] {
        let mut history = history(source_id, canvas, canvas);
        registry
            .apply_to_document_base(&mut history, preset)
            .expect("Guide Faces preset applies to the document base");
        if density != 100.0 {
            let command = history
                .document()
                .set_document_density_field(DensityEditedField::Density, density)
                .expect("combined Guide Faces density applies atomically");
            history
                .apply(&command)
                .expect("combined Guide Faces density publishes");
        }
        let base = history.document().pattern_settings().clone();
        let mut settings = base.clone();
        settings.pattern_rotation_degrees = rotation;
        history
            .apply(&DocumentCommand::SetDocumentPatternSettings { base, settings })
            .expect("ordinary Guide Faces rotation applies atomically");
        let result = evaluate(request(&history, source_id, asset, format))
            .expect("rotated Guide Faces evaluation publishes a complete result");
        assert!(result.scene().layers().iter().any(|layer| {
            layer.outputs().iter().any(|output| {
                matches!(
                    output.geometry(),
                    GeometryOutput::CanonicalRegions(regions) if !regions.regions().is_empty()
                )
            })
        }));
    }
}

/// Reproduces the dense even-dispersion acceptance case without treating
/// physical packing saturation as a render failure.
#[test]
fn even_dispersion_evaluates_after_density_increase() {
    let registry = PresetRegistry::bundled();
    let record = registry
        .entries()
        .iter()
        .find(|record| record.metadata.id == "even-random-circles")
        .expect("bundled even-dispersion preset");
    assert_bundled_recipe_evaluates_after_density_increase(record);
}

/// Proves density-aspect changes stretch region geometry through source sites and canonical output.
#[test]
fn density_aspect_stretches_canonical_regions_instead_of_postprocessing_pixels() {
    let mut history = history("region-aspect", 256.0, 256.0);
    PresetRegistry::bundled()
        .apply_to_document_base(&mut history, "two-guide-cells-uniform-offset")
        .expect("two-guide region recipe applies to document base");
    let square = evaluate(request(
        &history,
        "region-aspect",
        "raster-sample.png",
        SourceFormatHint::Png,
    ))
    .expect("1:1 region layout evaluates");
    let command = history
        .document()
        .set_document_density_field(DensityEditedField::Aspect, 2.0)
        .expect("2:1 density aspect forms one base command");
    history
        .apply(&command)
        .expect("2:1 density aspect applies atomically");
    let stretched = evaluate(request(
        &history,
        "region-aspect",
        "raster-sample.png",
        SourceFormatHint::Png,
    ))
    .expect("2:1 region layout evaluates");
    let square_aspect = center_region_bounds_aspect(&square);
    let stretched_aspect = center_region_bounds_aspect(&stretched);
    assert!(
        (square_aspect - stretched_aspect).abs() > 0.2,
        "canonical region bounds respond materially to site/baseline stretching: {square_aspect} vs {stretched_aspect}"
    );
    assert_ne!(square.scene(), stretched.scene());
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
                density_delta: 1.0,
                aspect_delta: 0.0,
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
