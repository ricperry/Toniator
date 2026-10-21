#[allow(dead_code)]
#[path = "../examples/artistic_construction_probe.rs"]
mod fixtures;
use toniator_domain::*;
use toniator_engine::*;
use toniator_patterns::ConstructionMode;

/// Preserves production family sharing while materialization mode rekeys whole semantic output units.
/// # Panics
/// Panics if mode changes reuse incompatible geometry or repeated Canvas loses deferred/paint slots.
#[test]
fn construction_mode_misses_then_warm_reuses_complete_semantic_decisions() {
    let id = SourceReferenceId::new("construction-cache").unwrap();
    let document = fixtures::fixture(
        "marks",
        CanvasSpec {
            width: 100.,
            height: 100.,
        },
        id.clone(),
    )
    .unwrap();
    let request = EvaluationRequest::new(
        DocumentSession::new(document)
            .unwrap()
            .document_evaluation_snapshot(),
        ResolvedSource::new(
            id,
            include_bytes!("../../../assets/raster-sample.png").as_slice(),
            SourceFormatHint::Png,
        )
        .unwrap(),
    )
    .for_scene()
    .unwrap();
    let mut cache = EvaluationProfileCache::default();
    let full = evaluate_profiled_cached_with_limits(
        request.clone(),
        EvaluationLimits::default().with_construction_mode(ConstructionMode::Complete),
        &mut cache,
    )
    .unwrap();
    let pruned = evaluate_profiled_cached_with_limits(
        request.clone(),
        EvaluationLimits::default().with_construction_mode(ConstructionMode::Canvas),
        &mut cache,
    )
    .unwrap();
    assert!(
        pruned
            .diagnostics
            .channels
            .iter()
            .all(|c| c.family == CacheDisposition::Hit && c.realization == CacheDisposition::Miss)
    );
    let warm = evaluate_profiled_cached_with_limits(
        request.clone(),
        EvaluationLimits::default().with_construction_mode(ConstructionMode::Canvas),
        &mut cache,
    )
    .unwrap();
    assert!(
        warm.diagnostics
            .channels
            .iter()
            .all(|c| c.family == CacheDisposition::Hit && c.realization == CacheDisposition::Hit)
    );
    let full_instances = full
        .result
        .scene()
        .layers()
        .iter()
        .flat_map(|layer| layer.outputs())
        .map(|output| match output.geometry() {
            toniator_render::GeometryOutput::CanonicalMarks(marks) => marks
                .iter()
                .map(|mark| match mark {
                    CanonicalMark::ClosedPath(mark) => mark.path.segments().len(),
                    _ => 0,
                })
                .sum::<usize>(),
            _ => 0,
        })
        .sum::<usize>();
    for mode in [ConstructionMode::Complete, ConstructionMode::Canvas] {
        let limited = EvaluationLimits::default()
            .with_construction_mode(mode)
            .with_max_transformed_curve_segment_instances(full_instances - 1)
            .unwrap();
        let error = evaluate_profiled_cached_with_limits(request.clone(), limited, &mut cache)
            .err()
            .unwrap();
        assert_eq!(error.path(), "realization.mark.segment_limit");
    }
    let recovered = evaluate_profiled_cached_with_limits(
        request,
        EvaluationLimits::default().with_construction_mode(ConstructionMode::Canvas),
        &mut cache,
    )
    .unwrap();
    assert!(
        recovered
            .diagnostics
            .channels
            .iter()
            .all(|c| c.realization == CacheDisposition::Hit)
    );
    for ((a, b), c) in full
        .result
        .scene()
        .layers()
        .iter()
        .zip(pruned.result.scene().layers())
        .zip(warm.result.scene().layers())
    {
        let a = &a.outputs()[0];
        let b = &b.outputs()[0];
        let c = &c.outputs()[0];
        assert!(std::sync::Arc::ptr_eq(
            &b.shared_geometry(),
            &c.shared_geometry()
        ));
        assert_eq!(
            a.geometry()
                .parts()
                .map(|part| match part {
                    toniator_render::GeometryOutputPart::CanonicalMarks(marks) => marks.len(),
                    _ => 0,
                })
                .sum::<usize>(),
            b.geometry()
                .parts()
                .map(|part| match part {
                    toniator_render::GeometryOutputPart::CanonicalMarks(marks) => marks.len(),
                    _ => 0,
                })
                .sum::<usize>()
        );
        assert_eq!(a.primitive_paints(), b.primitive_paints());
        let toniator_render::GeometryOutput::CanonicalMarks(marks) = b.geometry() else {
            unreachable!()
        };
        assert!(
            marks
                .iter()
                .any(|m| matches!(m, CanonicalMark::DeferredPath(_)))
        );
        let restored = toniator_render::prepare_geometry_for_canvas(
            b.geometry(),
            full.result.scene().canvas(),
            "changed-current-numeric-contract",
            &|| false,
        )
        .unwrap();
        assert_eq!(restored.as_ref(), a.geometry());
    }
}

/// Charges original outline work on warm cache hits independently of exterior construction suppression.
/// # Panics
/// Panics if an aggregate request limit admits pruned warm geometry that the complete reference rejects.
#[test]
fn construction_cached_outlines_preserve_original_aggregate_admission() {
    let id = SourceReferenceId::new("construction-outline-cache").unwrap();
    let original = fixtures::fixture(
        "motif",
        CanvasSpec {
            width: 100.,
            height: 80.,
        },
        id.clone(),
    )
    .unwrap();
    let target = original
        .channel_topology()
        .unwrap()
        .channels()
        .last()
        .unwrap()
        .id;
    let effective = original.effective_channel_pattern(target).unwrap();
    let definition = original
        .pattern_definition_bundles()
        .iter()
        .find(|b| b.definition.id == effective.definition_id)
        .unwrap()
        .definition
        .clone();
    let output_layer_id = definition.output_layers[0].id;
    let mut history = DocumentHistory::new(DocumentSession::new(original.clone()).unwrap());
    history
        .apply(&DocumentCommand::EditSelectedChannelPatternDefinition {
            channel_id: target,
            base_definition: definition,
            edit: PatternDefinitionEdit::SetToneSampleSpacing {
                output_layer_id,
                value: ToneSampleSpacing { multiplier: 0.25 },
            },
        })
        .unwrap();
    let changed = history.document().clone();
    let calibration = SavedCalibration::new(
        CalibrationBinding::for_channel(&changed, target).unwrap(),
        vec![
            CalibrationKnot {
                input: 0.,
                coverage: 0.,
            },
            CalibrationKnot {
                input: 2.,
                coverage: 1.,
            },
        ],
        None,
    )
    .unwrap();
    let changed = changed.with_saved_calibration(target, calibration).unwrap();
    let source = ResolvedSource::new(
        id,
        include_bytes!("../../../assets/raster-sample.png").as_slice(),
        SourceFormatHint::Png,
    )
    .unwrap();
    let request = |document| {
        EvaluationRequest::new(
            DocumentSession::new(document)
                .unwrap()
                .document_evaluation_snapshot(),
            source.clone(),
        )
        .for_scene()
        .unwrap()
    };
    let total = |document| {
        let result = evaluate_profiled_with_limits(
            request(document),
            EvaluationLimits::default().with_construction_mode(ConstructionMode::Complete),
        )
        .unwrap();
        result
            .result
            .scene()
            .layers()
            .iter()
            .flat_map(|layer| layer.outputs())
            .map(|output| match output.geometry() {
                toniator_render::GeometryOutput::CanonicalStrokes(strokes) => strokes
                    .iter()
                    .map(|stroke| stroke.outline.logical_segment_count().unwrap())
                    .sum::<usize>(),
                _ => 0,
            })
            .sum::<usize>()
    };
    let original_total = total(original.clone());
    let changed_total = total(changed.clone());
    assert_ne!(original_total, changed_total);
    // The smaller complete fixture is admitted first; only the last channel differs in either direction.
    let (original, changed, old_total, new_total) = if original_total < changed_total {
        (original, changed, original_total, changed_total)
    } else {
        (changed, original, changed_total, original_total)
    };
    let bound = old_total + (new_total - old_total) / 2;
    let mut failures = Vec::new();
    for mode in [ConstructionMode::Complete, ConstructionMode::Canvas] {
        let limits = EvaluationLimits::default()
            .with_construction_mode(mode)
            .with_max_stroke_outline_segments(bound)
            .unwrap();
        let mut cache = EvaluationProfileCache::default();
        evaluate_profiled_cached_with_limits(request(original.clone()), limits, &mut cache)
            .unwrap();
        let warm =
            evaluate_profiled_cached_with_limits(request(original.clone()), limits, &mut cache)
                .unwrap();
        assert!(
            warm.diagnostics
                .channels
                .iter()
                .all(|channel| channel.realization == CacheDisposition::Hit)
        );
        let error =
            evaluate_profiled_cached_with_limits(request(changed.clone()), limits, &mut cache)
                .err()
                .unwrap();
        assert!(
            matches!(
                error.path(),
                "curve.outline.segment_limit"
                    | "curve_motif.outline_limit"
                    | "realization.stroke.outline_limit"
            ),
            "{}",
            error.path()
        );
        failures.push(error.path());
        let recovered =
            evaluate_profiled_cached_with_limits(request(original.clone()), limits, &mut cache)
                .unwrap();
        assert!(
            recovered
                .diagnostics
                .channels
                .iter()
                .all(|channel| channel.realization == CacheDisposition::Hit)
        );
    }
    assert_eq!(failures[0], failures[1]);
}
