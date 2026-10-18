//! Current ordinary recipe/model rendering and keyed finite-cache controls.
#[allow(dead_code)]
#[path = "../examples/preview_recovery_probe.rs"]
mod probe;
use toniator_domain::*;
use toniator_engine::*;

/// Binds an explicitly synthetic monotone response with current validators, independent of fitting success.
/// # Panics
/// Panics if a current structural binding or source-independent response is invalid.
fn response(mut document: Document) -> Document {
    let knots = vec![
        CalibrationKnot {
            input: 0.0,
            coverage: 0.0,
        },
        CalibrationKnot {
            input: 2.0,
            coverage: 1.0,
        },
    ];
    let channels: Vec<_> = document
        .channel_topology()
        .unwrap()
        .channels()
        .iter()
        .map(|c| c.id)
        .collect();
    for channel in channels {
        let calibration = SavedCalibration::new(
            CalibrationBinding::for_channel(&document, channel).unwrap(),
            knots.clone(),
            None,
        )
        .unwrap();
        document = document
            .with_saved_calibration(channel, calibration)
            .unwrap();
    }
    let calibration = SavedCalibration::new(
        document.baseline_calibration_binding().unwrap(),
        knots,
        None,
    )
    .unwrap();
    document
        .with_saved_baseline_calibration(calibration)
        .unwrap()
}

/// Builds a fully validated current compatible document and normal production preview request.
/// Synthetic calibration isolates family construction; it is not a measured calibration or UI claim.
/// # Panics
/// Panics if immutable source, ID-free recipe, model command or current calibration authority fails.
fn request(source_name: &str, model: HalftoneChannelModel) -> EvaluationRequest {
    let (base, _, source) = probe::fixture(source_name, "grid").unwrap();
    let count = 513;
    let segments = (0..count)
        .map(|i| AuthoredCurveSegment::Line {
            start: AuthoredPoint2 {
                x: -1.0 + 2.0 * i as f64 / count as f64,
                y: 0.0,
            },
            end: AuthoredPoint2 {
                x: -1.0 + 2.0 * (i + 1) as f64 / count as f64,
                y: 0.0,
            },
        })
        .collect();
    let resource = AuthoredStructureDraft::new(AuthoredStructureKind::OpenPath, segments).unwrap();
    let recipe = PatternDefinitionRecipe::marks(PatternStructureRecipe::AuthoredResources {
        resources: vec![resource],
        definition: Box::new(PatternStructureRecipe::GenericGuides {
            name: "Perpendicular segmented guides — one crossing".into(),
            coverage: CoveragePolicy {
                guard_steps: 1,
                additional_margin: 0.0,
            },
            dimensions: [0.0, 90.0]
                .into_iter()
                .map(|angle| GenericGuideDimensionDraft {
                    baseline_angle_degrees: angle,
                    phase: 0.0,
                    prototype: GenericGuidePrototypeDraft::AuthoredOpenPathReference {
                        resource_index: 0,
                    },
                    repetition: GuideRepetition::Single,
                })
                .collect(),
            product: GeneralizedSiteProductDraft::Intersections {
                dimension_indices: vec![0, 1],
                merge_epsilon: 1e-7,
            },
            orientation: MarkOrientationDraft::Fixed,
        }),
    });
    let settings = base.pattern_settings().clone();
    let definition = base
        .pattern_definition_bundles()
        .iter()
        .find(|b| b.definition.id == settings.definition_id)
        .unwrap()
        .definition
        .clone();
    let mut history = DocumentHistory::new(DocumentSession::new(base).unwrap());
    history
        .apply(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base: settings,
            base_definition: definition,
            recipe,
        })
        .unwrap();
    if history.document().channel_model() != Some(model) {
        let topology = history
            .document()
            .canonical_channel_topology(model, ChannelTopologyTemplate::document_base())
            .unwrap();
        history
            .apply(&DocumentCommand::ReplaceChannelTopology { model, topology })
            .unwrap();
    }
    let document = response(history.document().clone());
    document.validate().unwrap();
    document.require_saved_calibration_authority().unwrap();
    EvaluationRequest::new(
        DocumentSession::new(document)
            .unwrap()
            .document_evaluation_snapshot(),
        source,
    )
    .for_preview(PreviewRasterTarget::new(1140, 908).unwrap())
}

/// Completes the proven513-segment recipe in all three domain-compatible modes for both immutable sources.
/// # Panics
/// Panics on a normal creative-work ceiling, lost cold/warm identity or incorrect family cardinality.
#[test]
fn segmented_guides_render_in_all_compatible_modes_and_reuse_exactly() {
    for source in ["png", "svg"] {
        for model in [
            HalftoneChannelModel::Rgb,
            HalftoneChannelModel::Cmyk,
            HalftoneChannelModel::SourceColorAlpha,
        ] {
            let request = request(source, model);
            let mut cache = EvaluationProfileCache::default();
            let cold = evaluate_profiled_cached_with_limits(
                request.clone(),
                EvaluationLimits::default(),
                &mut cache,
            )
            .unwrap();
            let warm = evaluate_profiled_cached_with_limits(
                request,
                EvaluationLimits::default(),
                &mut cache,
            )
            .unwrap();
            assert_eq!(cold.result.scene(), warm.result.scene());
            assert_eq!(cold.result.raster(), warm.result.raster());
            let family: Vec<_> = cold
                .performance
                .records
                .iter()
                .filter(|r| r.stage == EvaluationPerformanceStage::Family)
                .collect();
            assert!(!family.is_empty());
            assert!(family.iter().all(|r| {
                r.workloads
                    .iter()
                    .any(|w| w.kind == EvaluationWorkloadKind::FamilySites && w.count == 1)
            }));
            println!("{source} {model:?}: one site, cold/warm scene+RGBA exact");
        }
    }
}

/// Rechecks a stricter keyed family policy after accepted cache publication and preserves unrestricted recovery.
/// # Panics
/// Panics if a permissive result bypasses finite admission or a failure corrupts accepted output.
#[test]
fn finite_family_policy_does_not_reuse_permissive_intersection_cache() {
    let request = request("png", HalftoneChannelModel::Rgb);
    let mut cache = EvaluationProfileCache::default();
    let accepted = evaluate_profiled_cached_with_limits(
        request.clone(),
        EvaluationLimits::default(),
        &mut cache,
    )
    .unwrap();
    let failure = evaluate_profiled_cached_with_limits(
        request.clone(),
        EvaluationLimits::new(1_000).unwrap(),
        &mut cache,
    )
    .err()
    .unwrap();
    assert_eq!(failure.path(), "coverage.curved_guides.pairwise_limit");
    let recovered =
        evaluate_profiled_cached_with_limits(request, EvaluationLimits::default(), &mut cache)
            .unwrap();
    assert_eq!(accepted.result.scene(), recovered.result.scene());
    assert_eq!(accepted.result.raster(), recovered.result.raster());
}
