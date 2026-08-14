use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs,
    path::Path,
    time::{Duration, Instant},
};
use toniator_domain::{
    ArtworkWeightResponse, AuthoredCurveSegment, AuthoredPoint2, AuthoredStructure,
    AuthoredStructureDraft, AuthoredStructureId, AuthoredStructureKind, CanvasSpec,
    ChannelAppearance, ChannelId, ChannelPatternLayout, ChannelSourceMapping, ChannelState,
    ChannelTopology, ChannelTopologyTemplate, ColorValue, CoveragePolicy, DensityMetric2D,
    Document, DocumentCommand, DocumentHistory, DocumentId, DocumentSession,
    GeneralizedSiteProduct, GuideDimension, GuideDimensionId, GuidePrototype, GuideRepetition,
    HalftoneChannelModel, HalftoneChannelRole, InvalidationLevel, MarkGeometryFieldEdit,
    MarkGeometryResponse, MarkOrientation, PROPERTY_FIELD_IDS, PatternDefinition,
    PatternDefinitionEdit, PatternDefinitionId, PatternMechanism, PatternMechanismId,
    PatternOutputLayer, PatternOutputLayerId, PropertyFieldId, RandomSiteCharacter,
    SiteDensityModulation, SiteExclusionPolicy, SourceComponent, SourceMapping,
    SourceMappingComponent, SourcePlacement, SourceReference, SourceReferenceId,
    StraightGuideDimension, StraightGuideRepetition, TranslationEditedAxis,
    VisibleMarkSizingPolicy, property_field_contract, property_field_contracts,
};
use toniator_engine::{
    CacheDisposition, ChannelDiagnosticRequest, EvaluationCompletion, EvaluationLimits,
    EvaluationRequest, EvaluationScheduler, PreviewRasterTarget, RasterSurface, ResolvedSource,
    SourceFormatHint, encode_png, evaluate, evaluate_channel_diagnostic, evaluate_with_limits,
    write_svg,
};
use toniator_io::{EmbeddedSource, EmbeddedSourceFormat, SourceBundle, load, save};
use toniator_patterns::{CanonicalFillRule, PathClosure};

fn wait_for_latest(scheduler: &EvaluationScheduler) -> EvaluationCompletion {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(completion) = scheduler.try_receive_latest().unwrap() {
            return completion;
        }
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
}

/// Builds a complete source-assigned modeled session with a shared authored guide resource.
fn stage20d_session() -> DocumentSession {
    let source_id = SourceReferenceId::new("fixture-source").unwrap();
    let base = Document::new_default_document(
        CanvasSpec {
            width: 100.0,
            height: 100.0,
        },
        SourceReference::Assigned(source_id),
    )
    .unwrap();
    let definition = PatternDefinition::generalized_guides(
        PatternDefinitionId(1),
        "curved",
        PatternMechanismId(1),
        PatternMechanismId(2),
        PatternOutputLayerId(1),
        vec![
            GuideDimension {
                id: GuideDimensionId(1),
                baseline_angle_degrees: 0.0,
                phase: 0.0,
                prototype: GuidePrototype::AuthoredOpenPath {
                    structure_id: AuthoredStructureId(7),
                },
                repetition: GuideRepetition::Single,
            },
            GuideDimension {
                id: GuideDimensionId(2),
                baseline_angle_degrees: 90.0,
                phase: 0.0,
                prototype: GuidePrototype::CircularArc {
                    center: AuthoredPoint2 { x: 50.0, y: 50.0 },
                    radius: 30.0,
                    start_angle_degrees: 0.0,
                    sweep_angle_degrees: 180.0,
                },
                repetition: GuideRepetition::Single,
            },
        ],
        GeneralizedSiteProduct::Intersections {
            dimensions: vec![GuideDimensionId(1), GuideDimensionId(2)],
            merge_epsilon: 0.0,
        },
        MarkOrientation::Fixed,
        CoveragePolicy {
            guard_steps: 1,
            additional_margin: 4.5,
        },
    );
    let structure = AuthoredStructure::new(
        AuthoredStructureId(7),
        AuthoredStructureKind::OpenPath,
        vec![AuthoredCurveSegment::Line {
            start: AuthoredPoint2 { x: 0.0, y: 50.0 },
            end: AuthoredPoint2 { x: 100.0, y: 50.0 },
        }],
    )
    .unwrap();
    DocumentSession::new(
        Document::with_source_topology_and_authored_structures(
            base.id(),
            base.canvas().clone(),
            base.source().clone(),
            vec![definition],
            base.channel_model().unwrap(),
            base.channel_topology().unwrap().clone(),
            vec![structure],
        )
        .unwrap(),
    )
    .unwrap()
}

/// Builds one modeled shape-mark session whose retained family site comes from two authored
/// straight guides; only the mark resource varies between tests.
fn stage20e2_session(shape_segments: Vec<AuthoredCurveSegment>) -> DocumentSession {
    let source_id = SourceReferenceId::new("fixture-source").unwrap();
    let base = Document::new_default_document(
        CanvasSpec {
            width: 100.0,
            height: 100.0,
        },
        SourceReference::Assigned(source_id),
    )
    .unwrap();
    let mut definition = PatternDefinition::generalized_guides(
        PatternDefinitionId(1),
        "shape marks",
        PatternMechanismId(1),
        PatternMechanismId(2),
        PatternOutputLayerId(1),
        vec![
            GuideDimension {
                id: GuideDimensionId(1),
                baseline_angle_degrees: 0.0,
                phase: 0.0,
                prototype: GuidePrototype::AuthoredOpenPath {
                    structure_id: AuthoredStructureId(6),
                },
                repetition: GuideRepetition::Single,
            },
            GuideDimension {
                id: GuideDimensionId(2),
                baseline_angle_degrees: 90.0,
                phase: 0.0,
                prototype: GuidePrototype::AuthoredOpenPath {
                    structure_id: AuthoredStructureId(7),
                },
                repetition: GuideRepetition::Single,
            },
        ],
        GeneralizedSiteProduct::Intersections {
            dimensions: vec![GuideDimensionId(1), GuideDimensionId(2)],
            merge_epsilon: 0.0,
        },
        MarkOrientation::GuideTangent {
            dimension_id: GuideDimensionId(2),
        },
        CoveragePolicy {
            guard_steps: 1,
            additional_margin: 4.5,
        },
    );
    let PatternOutputLayer::MarkPrototype { prototype, .. } = &mut definition.output_layers[0]
    else {
        unreachable!("generalized guides own a typed mark output")
    };
    *prototype = toniator_domain::MarkPrototype::AuthoredClosedShape {
        structure_id: AuthoredStructureId(8),
    };
    let horizontal_guide = AuthoredStructure::new(
        AuthoredStructureId(6),
        AuthoredStructureKind::OpenPath,
        vec![AuthoredCurveSegment::Line {
            start: AuthoredPoint2 { x: 0.0, y: 50.0 },
            end: AuthoredPoint2 { x: 100.0, y: 50.0 },
        }],
    )
    .unwrap();
    let vertical_guide = AuthoredStructure::new(
        AuthoredStructureId(7),
        AuthoredStructureKind::OpenPath,
        vec![AuthoredCurveSegment::Line {
            start: AuthoredPoint2 { x: 50.0, y: 0.0 },
            end: AuthoredPoint2 { x: 50.0, y: 100.0 },
        }],
    )
    .unwrap();
    let shape = AuthoredStructure::new(
        AuthoredStructureId(8),
        AuthoredStructureKind::ClosedShape,
        shape_segments,
    )
    .unwrap();
    DocumentSession::new(
        Document::with_source_topology_and_authored_structures(
            base.id(),
            base.canvas().clone(),
            base.source().clone(),
            vec![definition],
            base.channel_model().unwrap(),
            base.channel_topology().unwrap().clone(),
            vec![horizontal_guide, vertical_guide, shape],
        )
        .unwrap(),
    )
    .unwrap()
}

/// Returns a nonzero-extent, zero-area two-segment closed line path accepted by the E2 contract.
fn stage20e2_zero_area_shape() -> Vec<AuthoredCurveSegment> {
    let left = AuthoredPoint2 { x: -2.0, y: 0.0 };
    let right = AuthoredPoint2 { x: 2.0, y: 0.0 };
    vec![
        AuthoredCurveSegment::Line {
            start: left,
            end: right,
        },
        AuthoredCurveSegment::Line {
            start: right,
            end: left,
        },
    ]
}

/// Returns a self-intersecting four-segment bow-tie used to exercise even-odd path output.
fn stage20e2_bow_tie_shape() -> Vec<AuthoredCurveSegment> {
    let points = [
        AuthoredPoint2 { x: -2.0, y: -2.0 },
        AuthoredPoint2 { x: 2.0, y: 2.0 },
        AuthoredPoint2 { x: -2.0, y: 2.0 },
        AuthoredPoint2 { x: 2.0, y: -2.0 },
    ];
    (0..4)
        .map(|index| AuthoredCurveSegment::Line {
            start: points[index],
            end: points[(index + 1) % 4],
        })
        .collect()
}

/// Exercises one self-intersecting authored prototype through canonical geometry, native raster,
/// and editable SVG while retaining exact family IDs, provenance, closure, and even-odd fill.
#[test]
fn stage20e2_shape_marks_flow_through_shared_canonical_consumers() {
    let session = stage20e2_session(stage20e2_bow_tie_shape());
    let bytes = fs::read("../../assets/raster-sample.png").unwrap();
    let result = evaluate(EvaluationRequest::new(
        session.document_evaluation_snapshot(),
        ResolvedSource::new(
            SourceReferenceId::new("fixture-source").unwrap(),
            bytes,
            SourceFormatHint::Png,
        )
        .unwrap(),
    ))
    .expect("the authored shape document evaluates");
    let mut mark_count = 0_usize;
    for layer in result.scene().layers() {
        let toniator_engine::GeometryOutput::CanonicalMarks(marks) = layer.geometry() else {
            panic!("shape-bearing typed outputs must use canonical mark geometry")
        };
        assert!(!marks.is_empty());
        for mark in marks {
            let toniator_engine::CanonicalMark::ClosedPath(path) = mark else {
                panic!("the authored prototype must not regress to circles")
            };
            assert_eq!(path.path.closure(), PathClosure::Closed);
            assert_eq!(path.fill_rule, CanonicalFillRule::EvenOdd);
            assert!(path.bounds.min.is_finite() && path.bounds.max.is_finite());
            assert_eq!(path.path.segments().len(), 4);
            mark_count += 1;
        }
    }
    assert!(mark_count > 0);
    let svg = write_svg(result.scene());
    assert_eq!(svg.matches("fill-rule=\"evenodd\"").count(), mark_count);
    assert_eq!(svg.matches("<path ").count(), mark_count);
    assert!(
        result
            .raster()
            .pixels()
            .chunks_exact(4)
            .any(|pixel| pixel[3] > 0),
        "raw native RGBA output must retain visible coverage"
    );
}

/// Fixes the request-wide transformed-segment boundary and proves an over-limit replacement
/// cannot publish partial cache state over a prior accepted zero-area shape realization.
#[test]
fn stage20e2_segment_limit_is_request_wide_and_failure_preserves_accepted_cache() {
    let mut history = DocumentHistory::new(stage20e2_session(stage20e2_zero_area_shape()));
    let bytes = fs::read("../../assets/raster-sample.png").unwrap();
    let request = |history: &DocumentHistory| {
        EvaluationRequest::new(
            history.session().document_evaluation_snapshot(),
            ResolvedSource::new(
                SourceReferenceId::new("fixture-source").unwrap(),
                bytes.clone(),
                SourceFormatHint::Png,
            )
            .unwrap(),
        )
    };
    let exact_limits = EvaluationLimits::default()
        .with_max_transformed_curve_segment_instances(6)
        .unwrap();
    evaluate_with_limits(request(&history), exact_limits)
        .expect("one site by two segments by three channels fits the exact limit");
    let under_limit = EvaluationLimits::default()
        .with_max_transformed_curve_segment_instances(5)
        .unwrap();
    let Err(under_limit_error) = evaluate_with_limits(request(&history), under_limit) else {
        panic!("the request-wide segment product must exceed five")
    };
    assert_eq!(under_limit_error.path(), "realization.mark.segment_limit");

    let scheduler = EvaluationScheduler::new_with_limits(exact_limits).unwrap();
    submit_and_accept(&scheduler, history.session(), request(&history));
    let base_structure = history
        .document()
        .authored_structure(AuthoredStructureId(8))
        .unwrap()
        .clone();
    history
        .apply(&DocumentCommand::ReplaceAuthoredStructure {
            base_structure,
            replacement: AuthoredStructureDraft::new(
                AuthoredStructureKind::ClosedShape,
                stage20e2_bow_tie_shape(),
            )
            .unwrap(),
        })
        .unwrap();
    let failed_ticket = scheduler.submit(request(&history)).unwrap();
    let failed = wait_for_latest(&scheduler);
    assert_eq!(failed.ticket(), failed_ticket);
    assert_eq!(
        failed
            .error()
            .expect("the larger shape exceeds the accepted scheduler limit")
            .path(),
        "realization.mark.segment_limit"
    );
    history.undo().unwrap();
    let reused = submit_and_accept(&scheduler, history.session(), request(&history));
    let diagnostics = reused.cache_diagnostics().unwrap();
    assert_eq!(diagnostics.aggregate.realization, CacheDisposition::Hit);
    assert_eq!(diagnostics.aggregate.scene, CacheDisposition::Hit);
    assert_eq!(diagnostics.aggregate.raster, CacheDisposition::Hit);
    scheduler.shutdown().unwrap();
}

/// Proves a same-ID authored-shape content replacement preserves the family cache while missing
/// realization/scene/raster exactly once, after which the accepted replacement is fully reusable.
#[test]
fn stage20e2_shape_content_participates_in_realization_and_downstream_cache_identity() {
    let mut history = DocumentHistory::new(stage20e2_session(stage20e2_zero_area_shape()));
    let source_id = SourceReferenceId::new("fixture-source").unwrap();
    let bytes = fs::read("../../assets/raster-sample.png").unwrap();
    let request = |history: &DocumentHistory| {
        EvaluationRequest::new(
            history.session().document_evaluation_snapshot(),
            ResolvedSource::new(source_id.clone(), bytes.clone(), SourceFormatHint::Png).unwrap(),
        )
    };
    let limits = EvaluationLimits::default()
        .with_max_transformed_curve_segment_instances(6)
        .unwrap();
    let scheduler = EvaluationScheduler::new_with_limits(limits).unwrap();
    let baseline = submit_and_accept(&scheduler, history.session(), request(&history));
    let baseline_identity = baseline.result().unwrap().scene().identity().clone();
    let base_structure = history
        .document()
        .authored_structure(AuthoredStructureId(8))
        .unwrap()
        .clone();
    let left = AuthoredPoint2 { x: -3.0, y: 1.0 };
    let right = AuthoredPoint2 { x: 3.0, y: 1.0 };
    history
        .apply(&DocumentCommand::ReplaceAuthoredStructure {
            base_structure,
            replacement: AuthoredStructureDraft::new(
                AuthoredStructureKind::ClosedShape,
                vec![
                    AuthoredCurveSegment::Line {
                        start: left,
                        end: right,
                    },
                    AuthoredCurveSegment::Line {
                        start: right,
                        end: left,
                    },
                ],
            )
            .unwrap(),
        })
        .unwrap();
    let changed = submit_and_accept(&scheduler, history.session(), request(&history));
    let diagnostics = changed.cache_diagnostics().unwrap();
    assert_eq!(diagnostics.aggregate.decoded_source, CacheDisposition::Hit);
    assert_eq!(diagnostics.aggregate.family, CacheDisposition::Hit);
    assert_eq!(diagnostics.aggregate.realization, CacheDisposition::Miss);
    assert_eq!(diagnostics.aggregate.scene, CacheDisposition::Miss);
    assert_eq!(diagnostics.aggregate.raster, CacheDisposition::Miss);
    assert_ne!(
        changed.result().unwrap().scene().identity(),
        &baseline_identity
    );
    let repeated = submit_and_accept(&scheduler, history.session(), request(&history));
    assert_eq!(
        repeated.cache_diagnostics().unwrap().aggregate,
        toniator_engine::CacheDiagnostics {
            decoded_source: CacheDisposition::Hit,
            family: CacheDisposition::Hit,
            realization: CacheDisposition::Hit,
            scene: CacheDisposition::Hit,
            raster: CacheDisposition::Hit,
        }
    );
    scheduler.shutdown().unwrap();
}

/// Proves sampled-source paint uses canonical authored paths and preserves every family site;
/// exact-zero source alpha becomes invisible zero-scale geometry without exposing hidden RGB.
#[test]
fn stage20e2_sampled_source_paint_keeps_canonical_shape_topology() {
    let rgb = stage20e2_session(stage20e2_bow_tie_shape());
    let document = rgb.document();
    let mut definitions = document.pattern_definitions().to_vec();
    let PatternMechanism::GuideDimensions { dimensions, .. } = &mut definitions[0].mechanisms[0]
    else {
        panic!("shape fixture owns generic guide dimensions")
    };
    dimensions[0].repetition = GuideRepetition::TransformStack {
        direction_degrees: 90.0,
        spacing_multiplier: 1.0,
    };
    dimensions[1].repetition = GuideRepetition::TransformStack {
        direction_degrees: 0.0,
        spacing_multiplier: 1.0,
    };
    let seed = &document.channel_topology().unwrap().channels()[0];
    let topology = document
        .canonical_channel_topology(
            HalftoneChannelModel::SourceColorAlpha,
            ChannelTopologyTemplate {
                pattern_definition_id: seed.pattern_definition_id,
                layout: seed.layout.clone(),
                mark_geometry_response: seed.mark_geometry_response.clone(),
            },
        )
        .unwrap();
    let sampled = DocumentSession::new(
        Document::with_source_topology_and_authored_structures(
            document.id(),
            document.canvas().clone(),
            document.source().clone(),
            definitions,
            HalftoneChannelModel::SourceColorAlpha,
            topology,
            document.authored_structures().to_vec(),
        )
        .unwrap(),
    )
    .unwrap();
    let bytes = fs::read("../../assets/raster-sample.png").unwrap();
    let result = evaluate(EvaluationRequest::new(
        sampled.document_evaluation_snapshot(),
        ResolvedSource::new(
            SourceReferenceId::new("fixture-source").unwrap(),
            bytes,
            SourceFormatHint::Png,
        )
        .unwrap(),
    ))
    .unwrap();
    assert_eq!(
        result.scene().model(),
        Some(HalftoneChannelModel::SourceColorAlpha)
    );
    assert_eq!(result.scene().layers().len(), 1);
    let toniator_engine::GeometryOutput::CanonicalMarks(marks) =
        result.scene().layers()[0].geometry()
    else {
        panic!("sampled authored output must retain canonical path geometry")
    };
    assert!(!marks.is_empty());
    assert!(
        marks
            .iter()
            .all(|mark| matches!(mark, toniator_engine::CanonicalMark::ClosedPath(_)))
    );
    let visible_colors = result
        .raster()
        .pixels()
        .chunks_exact(4)
        .filter(|pixel| pixel[3] > 0)
        .map(|pixel| (pixel[0], pixel[1], pixel[2]))
        .collect::<BTreeSet<_>>();
    assert!(
        visible_colors.len() > 1,
        "sampled-source path marks retain source-derived paint variation"
    );

    let transparent_png = encode_png(
        &RasterSurface::new(1, 1, vec![255, 0, 255, 0])
            .expect("one hidden-magenta transparent source pixel is valid RGBA"),
    )
    .unwrap();
    let transparent = evaluate(EvaluationRequest::new(
        sampled.document_evaluation_snapshot(),
        ResolvedSource::new(
            SourceReferenceId::new("fixture-source").unwrap(),
            transparent_png,
            SourceFormatHint::Png,
        )
        .unwrap(),
    ))
    .unwrap();
    let toniator_engine::GeometryOutput::CanonicalMarks(transparent_marks) =
        transparent.scene().layers()[0].geometry()
    else {
        panic!("zero-alpha sampled output must retain canonical path geometry")
    };
    assert_eq!(transparent_marks.len(), marks.len());
    assert!(transparent_marks.iter().all(|mark| {
        matches!(
            mark,
            toniator_engine::CanonicalMark::ClosedPath(path) if path.bounds.min == path.bounds.max
        )
    }));
    assert!(
        transparent
            .raster()
            .pixels()
            .iter()
            .all(|value| *value == 0)
    );
    let transparent_svg = write_svg(transparent.scene());
    assert_eq!(
        transparent_svg.matches("<path ").count(),
        transparent_marks.len()
    );
    assert!(transparent_svg.contains("fill=\"#000000\""));
    assert!(!transparent_svg.contains("fill=\"#ff00ff\""));
}

/// Builds a legacy single-channel snapshot so the public diagnostic entry proves document-aware guide resolution.
fn stage20d_legacy_diagnostic_session() -> DocumentSession {
    let modeled = stage20d_session();
    let document = modeled.document();
    let legacy = Document::with_source_and_authored_structures(
        document.id(),
        document.canvas().clone(),
        document.source().clone(),
        vec![document.pattern_definitions()[0].clone()],
        vec![ChannelState {
            id: ChannelId(71),
            pattern_definition_id: PatternDefinitionId(1),
            layout: ChannelPatternLayout {
                density: DensityMetric2D {
                    across_x: 10.0,
                    across_y: 10.0,
                    aspect_locked: false,
                },
                rotation_degrees: 0.0,
                translation_x: 0.0,
                translation_y: 0.0,
            },
            appearance: ChannelAppearance {
                visible: true,
                color: ColorValue {
                    red: 0.0,
                    green: 0.0,
                    blue: 0.0,
                    alpha: 1.0,
                },
                opacity: 1.0,
            },
            mark_geometry_response: MarkGeometryResponse {
                minimum_fill: 1.0,
                maximum_fill: 4.5,
                rotation_offset_degrees: 0.0,
            },
            source_mapping: ChannelSourceMapping {
                component: SourceComponent::Luminance,
                placement: SourcePlacement::StretchToCanvas,
            },
        }],
        document.authored_structures().to_vec(),
    )
    .unwrap();
    DocumentSession::new(legacy).unwrap()
}

/// Proves resolved authored content, repetition/root layout, and candidate bounds participate in family cache identity.
#[test]
fn stage20d_authored_content_repetition_and_layout_key_family_cache_exactly() {
    let mut history = DocumentHistory::new(stage20d_session());
    let scheduler = EvaluationScheduler::new().unwrap();
    let bytes = fs::read("../../assets/raster-sample.png").unwrap();
    let request = |history: &DocumentHistory| {
        EvaluationRequest::new(
            history.session().document_evaluation_snapshot(),
            ResolvedSource::new(
                SourceReferenceId::new("fixture-source").unwrap(),
                bytes.clone(),
                SourceFormatHint::Png,
            )
            .unwrap(),
        )
    };
    let legacy = stage20d_legacy_diagnostic_session();
    let diagnostic = ChannelDiagnosticRequest::new(
        legacy.evaluation_snapshot(ChannelId(71)).unwrap(),
        ResolvedSource::new(
            SourceReferenceId::new("fixture-source").unwrap(),
            bytes.clone(),
            SourceFormatHint::Png,
        )
        .unwrap(),
    );
    assert!(
        evaluate_channel_diagnostic(diagnostic).is_ok(),
        "the public single-channel route resolves authored guide content through its snapshot document"
    );
    let first = submit_and_accept(&scheduler, history.session(), request(&history));
    assert!(
        first
            .cache_diagnostics()
            .unwrap()
            .channels
            .iter()
            .all(|channel| channel.family == CacheDisposition::Miss)
    );
    let original = history
        .document()
        .authored_structure(AuthoredStructureId(7))
        .unwrap()
        .clone();
    let replacement = AuthoredStructureDraft::new(
        AuthoredStructureKind::OpenPath,
        vec![AuthoredCurveSegment::Line {
            start: AuthoredPoint2 { x: 0.0, y: 40.0 },
            end: AuthoredPoint2 { x: 100.0, y: 40.0 },
        }],
    )
    .unwrap();
    history
        .apply(&DocumentCommand::ReplaceAuthoredStructure {
            base_structure: original,
            replacement,
        })
        .unwrap();
    let second = submit_and_accept(&scheduler, history.session(), request(&history));
    assert!(
        second
            .cache_diagnostics()
            .unwrap()
            .channels
            .iter()
            .all(|channel| channel.family == CacheDisposition::Miss)
    );
}

/// Proves failed and superseded complete-document requests cannot overwrite a prior accepted Stage 20D family cache.
#[test]
fn stage20d_failed_or_superseded_evaluation_preserves_last_accepted_cache() {
    let history = DocumentHistory::new(stage20d_session());
    let scheduler = EvaluationScheduler::new().unwrap();
    let bytes = fs::read("../../assets/raster-sample.png").unwrap();
    let good = || {
        EvaluationRequest::new(
            history.session().document_evaluation_snapshot(),
            ResolvedSource::new(
                SourceReferenceId::new("fixture-source").unwrap(),
                bytes.clone(),
                SourceFormatHint::Png,
            )
            .unwrap(),
        )
    };
    submit_and_accept(&scheduler, history.session(), good());
    let bad = EvaluationRequest::new(
        history.session().document_evaluation_snapshot(),
        ResolvedSource::new(
            SourceReferenceId::new("wrong-source").unwrap(),
            bytes.clone(),
            SourceFormatHint::Png,
        )
        .unwrap(),
    );
    let ticket = scheduler.submit(bad).unwrap();
    let failed = wait_for_latest(&scheduler);
    assert_eq!(failed.ticket(), ticket);
    assert!(failed.error().is_some());
    let superseded_ticket = scheduler.submit(good()).unwrap();
    let latest_ticket = scheduler.submit(good()).unwrap();
    assert_ne!(superseded_ticket, latest_ticket);
    let superseding = wait_for_latest(&scheduler);
    assert_eq!(superseding.ticket(), latest_ticket);
    assert!(
        scheduler
            .accept_completion(&superseding, history.session())
            .unwrap(),
        "only the newer request can publish its cache transaction"
    );
    assert_eq!(scheduler.try_receive_latest().unwrap(), None);
    let reused = submit_and_accept(&scheduler, history.session(), good());
    assert!(
        reused
            .cache_diagnostics()
            .unwrap()
            .channels
            .iter()
            .all(|channel| channel.family == CacheDisposition::Hit)
    );
}

fn submit_and_accept(
    scheduler: &EvaluationScheduler,
    session: &DocumentSession,
    request: EvaluationRequest,
) -> EvaluationCompletion {
    let ticket = scheduler.submit(request).unwrap();
    let completion = wait_for_latest(scheduler);
    assert_eq!(completion.ticket(), ticket);
    assert!(scheduler.accept_completion(&completion, session).unwrap());
    completion
}

fn assert_presentation_reuse(completion: &EvaluationCompletion) {
    let diagnostics = completion.cache_diagnostics().unwrap();
    assert_eq!(diagnostics.aggregate.decoded_source, CacheDisposition::Hit);
    assert_eq!(diagnostics.aggregate.family, CacheDisposition::Hit);
    assert_eq!(diagnostics.aggregate.realization, CacheDisposition::Hit);
    assert_eq!(diagnostics.aggregate.scene, CacheDisposition::Miss);
    assert_eq!(diagnostics.aggregate.raster, CacheDisposition::Miss);
    assert!(diagnostics.channels.iter().all(|channel| {
        channel.family == CacheDisposition::Hit && channel.realization == CacheDisposition::Hit
    }));
}

#[test]
fn stage17_history_command_cache_matrix_preserves_earliest_layers_and_restoration() {
    let source_id = SourceReferenceId::new("fixture-source").unwrap();
    let bytes = fs::read("../../assets/raster-sample.png").unwrap();
    let mut history = DocumentHistory::new(session(HalftoneChannelModel::Rgb));
    let scheduler = EvaluationScheduler::new().unwrap();
    let request = |history: &DocumentHistory| {
        EvaluationRequest::new(
            history.session().document_evaluation_snapshot(),
            ResolvedSource::new(source_id.clone(), bytes.clone(), SourceFormatHint::Png).unwrap(),
        )
    };
    let baseline = submit_and_accept(&scheduler, history.session(), request(&history));
    let baseline_result = baseline.result().unwrap().clone();
    let baseline_document = history.document().clone();

    let presentation = history
        .apply(&DocumentCommand::SetOpacity {
            channel_id: ChannelId(1),
            opacity: 0.5,
        })
        .unwrap();
    assert_eq!(presentation.affected_channels, vec![ChannelId(1)]);
    assert_eq!(presentation.invalidation, InvalidationLevel::Presentation);
    let presentation_completion =
        submit_and_accept(&scheduler, history.session(), request(&history));
    assert_presentation_reuse(&presentation_completion);

    let realization = history
        .apply(&DocumentCommand::SetMarkGeometryField {
            channel_id: ChannelId(1),
            edit: MarkGeometryFieldEdit::MinimumFill(1.0),
        })
        .unwrap();
    assert_eq!(realization.invalidation, InvalidationLevel::Realization);
    let realization_completion =
        submit_and_accept(&scheduler, history.session(), request(&history));
    let realization_diagnostics = realization_completion.cache_diagnostics().unwrap();
    assert_eq!(
        realization_diagnostics.aggregate.family,
        CacheDisposition::Hit
    );
    assert_eq!(
        realization_diagnostics.aggregate.realization,
        CacheDisposition::Miss
    );

    let family = history
        .apply(&DocumentCommand::SetTranslationAxis {
            channel_id: ChannelId(1),
            edited_axis: TranslationEditedAxis::X,
            value: 1.0,
        })
        .unwrap();
    assert_eq!(family.invalidation, InvalidationLevel::Family);
    let family_completion = submit_and_accept(&scheduler, history.session(), request(&history));
    assert_eq!(
        family_completion
            .cache_diagnostics()
            .unwrap()
            .aggregate
            .family,
        CacheDisposition::Miss
    );

    history.undo().unwrap();
    history.undo().unwrap();
    history.undo().unwrap();
    assert_eq!(history.document(), &baseline_document);
    let restored = submit_and_accept(&scheduler, history.session(), request(&history));
    assert_eq!(
        restored.result().unwrap().channels(),
        baseline_result.channels()
    );
    assert_eq!(
        restored.result().unwrap().scene().identity(),
        baseline_result.scene().identity()
    );
    assert_eq!(
        restored.result().unwrap().raster().pixels(),
        baseline_result.raster().pixels()
    );
    assert!(
        history
            .apply(&DocumentCommand::SetOpacity {
                channel_id: ChannelId(1),
                opacity: 1.0,
            })
            .is_err()
    );
    scheduler.shutdown().unwrap();
}

#[test]
fn stage17_shared_output_edits_disclose_copy_escalation_and_restore_cache_authority() {
    let source_id = SourceReferenceId::new("fixture-source").unwrap();
    let bytes = fs::read("../../assets/raster-sample.png").unwrap();
    let mut history = DocumentHistory::new(generalized_session_named(
        HalftoneChannelModel::Rgb,
        GeneralizedConfiguration::ThreeDirection,
    ));
    let scheduler = EvaluationScheduler::new().unwrap();
    let request = |history: &DocumentHistory| {
        EvaluationRequest::new(
            history.session().document_evaluation_snapshot(),
            ResolvedSource::new(source_id.clone(), bytes.clone(), SourceFormatHint::Png).unwrap(),
        )
    };
    let baseline = submit_and_accept(&scheduler, history.session(), request(&history));
    let original = history.document().clone();
    let base = original.pattern_definitions()[0].clone();
    let selected = history
        .apply(&DocumentCommand::EditSelectedChannelPatternDefinition {
            channel_id: ChannelId(1),
            base_definition: base.clone(),
            edit: PatternDefinitionEdit::SetOutputOrientation {
                output_layer_id: PatternOutputLayerId(1),
                orientation: MarkOrientation::Fixed,
            },
        })
        .unwrap();
    assert_eq!(selected.affected_channels, vec![ChannelId(1)]);
    assert_eq!(selected.invalidation, InvalidationLevel::Family);
    assert_ne!(history.document(), &original);
    assert_ne!(
        history
            .document()
            .modeled_channel(ChannelId(1))
            .unwrap()
            .pattern_definition_id,
        original
            .modeled_channel(ChannelId(1))
            .unwrap()
            .pattern_definition_id
    );
    let copied = submit_and_accept(&scheduler, history.session(), request(&history));
    let copied_diagnostics = copied.cache_diagnostics().unwrap();
    assert_eq!(copied_diagnostics.aggregate.family, CacheDisposition::Miss);
    assert_eq!(
        copied_diagnostics.aggregate.realization,
        CacheDisposition::Miss
    );
    history.undo().unwrap();
    assert_eq!(history.document(), &original);
    let restored = submit_and_accept(&scheduler, history.session(), request(&history));
    assert_eq!(
        restored.result().unwrap().channels(),
        baseline.result().unwrap().channels()
    );
    history.redo().unwrap();
    assert_eq!(history.document(), history.document());
    history.undo().unwrap();

    let shared = history
        .apply(&DocumentCommand::EditSharedPatternDefinition {
            definition_id: PatternDefinitionId(1),
            base_definition: base,
            edit: PatternDefinitionEdit::SetOutputOrientation {
                output_layer_id: PatternOutputLayerId(1),
                orientation: MarkOrientation::Fixed,
            },
        })
        .unwrap();
    assert_eq!(
        shared.affected_channels,
        vec![ChannelId(1), ChannelId(2), ChannelId(3)]
    );
    assert_eq!(shared.invalidation, InvalidationLevel::Realization);
    let shared_completion = submit_and_accept(&scheduler, history.session(), request(&history));
    assert_eq!(
        shared_completion
            .cache_diagnostics()
            .unwrap()
            .aggregate
            .family,
        CacheDisposition::Hit
    );
    assert_eq!(
        shared_completion
            .cache_diagnostics()
            .unwrap()
            .aggregate
            .realization,
        CacheDisposition::Miss
    );
    scheduler.shutdown().unwrap();
}

#[test]
fn stage17_source_and_topology_commands_disclose_complete_order_and_restore_history() {
    let source_id = SourceReferenceId::new("fixture-source").unwrap();
    let replacement_id = SourceReferenceId::new("replacement-source").unwrap();
    let bytes = fs::read("../../assets/raster-sample.png").unwrap();
    let mut history = DocumentHistory::new(session(HalftoneChannelModel::Rgb));
    let scheduler = EvaluationScheduler::new().unwrap();
    let request = |history: &DocumentHistory, id: SourceReferenceId| {
        EvaluationRequest::new(
            history.session().document_evaluation_snapshot(),
            ResolvedSource::new(id, bytes.clone(), SourceFormatHint::Png).unwrap(),
        )
    };
    let baseline = submit_and_accept(
        &scheduler,
        history.session(),
        request(&history, source_id.clone()),
    );
    let baseline_document = history.document().clone();
    let source = history
        .apply(&DocumentCommand::SetSourceReference {
            source: SourceReference::Assigned(replacement_id.clone()),
        })
        .unwrap();
    assert_eq!(
        source.affected_channels,
        vec![ChannelId(1), ChannelId(2), ChannelId(3)]
    );
    assert_eq!(source.invalidation, InvalidationLevel::Source);
    let source_completion = submit_and_accept(
        &scheduler,
        history.session(),
        request(&history, replacement_id.clone()),
    );
    let source_diagnostics = source_completion.cache_diagnostics().unwrap();
    assert_eq!(
        source_diagnostics.aggregate.decoded_source,
        CacheDisposition::Miss
    );
    assert_eq!(source_diagnostics.aggregate.family, CacheDisposition::Hit);
    // Logical lookup identity is a Source command result, while the accepted
    // realization key uses decoder-owned bytes; unchanged bytes therefore
    // retain the realization layer.
    assert_eq!(
        source_diagnostics.aggregate.realization,
        CacheDisposition::Hit
    );
    history.undo().unwrap();
    assert_eq!(history.document(), &baseline_document);
    let restored_source = submit_and_accept(
        &scheduler,
        history.session(),
        request(&history, source_id.clone()),
    );
    assert_eq!(
        restored_source.result().unwrap().scene().identity(),
        baseline.result().unwrap().scene().identity()
    );
    history.redo().unwrap();
    history.undo().unwrap();

    let mut channels = history
        .document()
        .channel_topology()
        .unwrap()
        .channels()
        .to_vec();
    channels[1].id = ChannelId(22);
    channels[2].id = ChannelId(23);
    let topology = ChannelTopology::new(channels);
    let topology_result = history
        .apply(&DocumentCommand::ReplaceChannelTopology {
            model: HalftoneChannelModel::Rgb,
            topology,
        })
        .unwrap();
    assert_eq!(
        topology_result.affected_channels,
        vec![
            ChannelId(1),
            ChannelId(2),
            ChannelId(3),
            ChannelId(22),
            ChannelId(23)
        ]
    );
    assert_eq!(
        topology_result.invalidation,
        InvalidationLevel::ChannelTopology
    );
    let topology_completion = submit_and_accept(
        &scheduler,
        history.session(),
        request(&history, source_id.clone()),
    );
    let topology_diagnostics = topology_completion.cache_diagnostics().unwrap();
    assert_eq!(
        topology_diagnostics.aggregate.decoded_source,
        CacheDisposition::Hit
    );
    assert_eq!(topology_diagnostics.aggregate.family, CacheDisposition::Hit);
    assert_eq!(
        topology_diagnostics.aggregate.realization,
        CacheDisposition::Hit
    );
    assert_eq!(topology_diagnostics.aggregate.scene, CacheDisposition::Miss);
    assert_eq!(
        topology_diagnostics.aggregate.raster,
        CacheDisposition::Miss
    );
    let topology_document = history.document().clone();
    history.undo().unwrap();
    assert_eq!(history.document(), &baseline_document);
    history.redo().unwrap();
    assert_eq!(history.document(), &topology_document);
    scheduler.shutdown().unwrap();
}

#[test]
fn stage17_contract_invalidation_matrix_and_descriptor_reads_are_cache_inert() {
    // Each field-contract invalidation class is represented by one typed
    // command exercised in this Stage 17 engine matrix. ChannelTopology is
    // intentionally command-only rather than a field contract.
    let represented = [
        (InvalidationLevel::Presentation, "SetOpacity"),
        (InvalidationLevel::Realization, "SetMarkGeometryField"),
        (InvalidationLevel::Family, "SetTranslationAxis"),
        (InvalidationLevel::Source, "SetSourceReference"),
    ];
    let contract_levels: BTreeSet<_> = property_field_contracts()
        .map(|contract| format!("{:?}", contract.invalidation))
        .collect();
    let represented_levels: BTreeSet<_> = represented
        .iter()
        .map(|(level, _)| format!("{:?}", level))
        .collect();
    assert_eq!(contract_levels, represented_levels);
    assert!(represented.iter().all(|(_, command)| !command.is_empty()));

    let source_id = SourceReferenceId::new("fixture-source").unwrap();
    let bytes = fs::read("../../assets/raster-sample.png").unwrap();
    let history = DocumentHistory::new(session(HalftoneChannelModel::Rgb));
    let scheduler = EvaluationScheduler::new().unwrap();
    let request = || {
        EvaluationRequest::new(
            history.session().document_evaluation_snapshot(),
            ResolvedSource::new(source_id.clone(), bytes.clone(), SourceFormatHint::Png).unwrap(),
        )
    };
    let baseline = submit_and_accept(&scheduler, history.session(), request());
    let baseline_result = baseline.result().unwrap();
    let baseline_svg = write_svg(baseline_result.scene());
    let baseline_hash = Sha256::digest(baseline_result.raster().pixels());
    let revision = history.revision();
    let document = history.document().clone();

    let first = history.document().property_descriptors();
    let second = history.document().property_descriptors();
    assert_eq!(first, second);
    assert!(
        first
            .iter()
            .all(|descriptor| descriptor.field != PropertyFieldId::RandomClusterDensity)
    );
    assert!(
        first
            .iter()
            .all(|descriptor| descriptor.field != PropertyFieldId::VisibleMarkMargin)
    );
    assert_eq!(history.revision(), revision);
    assert_eq!(history.document(), &document);
    let repeated = submit_and_accept(&scheduler, history.session(), request());
    let diagnostics = repeated.cache_diagnostics().unwrap();
    assert_eq!(diagnostics.aggregate.decoded_source, CacheDisposition::Hit);
    assert_eq!(diagnostics.aggregate.family, CacheDisposition::Hit);
    assert_eq!(diagnostics.aggregate.realization, CacheDisposition::Hit);
    assert_eq!(diagnostics.aggregate.scene, CacheDisposition::Hit);
    assert_eq!(diagnostics.aggregate.raster, CacheDisposition::Hit);
    let repeated_result = repeated.result().unwrap();
    assert_eq!(write_svg(repeated_result.scene()), baseline_svg);
    assert_eq!(
        Sha256::digest(repeated_result.raster().pixels()),
        baseline_hash
    );
    scheduler.shutdown().unwrap();
}

#[derive(Clone, Copy)]
enum Stage17StructuralFixture {
    Intersections,
    AlongGuides,
    RawRandom,
    EvenRandom,
    ClusteredRandom,
    WeightedRandom,
    MinimumCenterRandom,
    VisibleMarginRandom,
}

fn stage17_structural_session(fixture: Stage17StructuralFixture) -> DocumentSession {
    match fixture {
        Stage17StructuralFixture::Intersections => generalized_session_named(
            HalftoneChannelModel::Rgb,
            GeneralizedConfiguration::ThreeDirection,
        ),
        Stage17StructuralFixture::AlongGuides => generalized_session_named(
            HalftoneChannelModel::Rgb,
            GeneralizedConfiguration::AlongGuide,
        ),
        Stage17StructuralFixture::RawRandom => random_session(
            HalftoneChannelModel::Rgb,
            90.0,
            60.0,
            random_definition(
                RandomSiteCharacter::RawUniform,
                SiteDensityModulation::Uniform,
                SiteExclusionPolicy::None,
                64,
            ),
        ),
        Stage17StructuralFixture::EvenRandom => random_session(
            HalftoneChannelModel::Rgb,
            90.0,
            60.0,
            random_definition(
                RandomSiteCharacter::Even {
                    minimum_center_distance: 6.0,
                },
                SiteDensityModulation::Uniform,
                SiteExclusionPolicy::None,
                64,
            ),
        ),
        Stage17StructuralFixture::ClusteredRandom => random_session(
            HalftoneChannelModel::Rgb,
            90.0,
            60.0,
            random_definition(
                RandomSiteCharacter::Clustered {
                    cluster_density: 0.02,
                    cluster_spread: 4.0,
                    cluster_strength: 0.5,
                },
                SiteDensityModulation::Uniform,
                SiteExclusionPolicy::None,
                64,
            ),
        ),
        Stage17StructuralFixture::WeightedRandom => random_session(
            HalftoneChannelModel::Rgb,
            90.0,
            60.0,
            random_definition(
                RandomSiteCharacter::RawUniform,
                SiteDensityModulation::ArtworkWeighted {
                    mapping: SourceMapping::canonical(SourceMappingComponent::Luminance),
                    strength: 0.5,
                    response: ArtworkWeightResponse::Smoothstep,
                },
                SiteExclusionPolicy::None,
                64,
            ),
        ),
        Stage17StructuralFixture::MinimumCenterRandom => random_session(
            HalftoneChannelModel::Rgb,
            90.0,
            60.0,
            random_definition(
                RandomSiteCharacter::RawUniform,
                SiteDensityModulation::Uniform,
                SiteExclusionPolicy::MinimumCenterDistance { minimum: 4.0 },
                64,
            ),
        ),
        Stage17StructuralFixture::VisibleMarginRandom => random_session(
            HalftoneChannelModel::Rgb,
            90.0,
            60.0,
            random_definition(
                RandomSiteCharacter::RawUniform,
                SiteDensityModulation::Uniform,
                SiteExclusionPolicy::VisibleMarkMargin {
                    margin: 0.5,
                    sizing: VisibleMarkSizingPolicy::MaximumSupportRadius,
                },
                64,
            ),
        ),
    }
}

/// Classifies property descriptors whose edits invalidate the active pattern-definition authority.
fn is_pattern_definition_leaf(field: PropertyFieldId) -> bool {
    match field {
        PropertyFieldId::MarkRotationOffsetDegrees => false,
        PropertyFieldId::CoverageGuardSteps
        | PropertyFieldId::CoverageAdditionalMargin
        | PropertyFieldId::GuideBaselineAngle
        | PropertyFieldId::GuidePhase
        | PropertyFieldId::GuideSpacingMultiplier
        | PropertyFieldId::GuidePrototype
        | PropertyFieldId::GuideAuthoredStructure
        | PropertyFieldId::GuideArcCenterX
        | PropertyFieldId::GuideArcCenterY
        | PropertyFieldId::GuideArcRadius
        | PropertyFieldId::GuideArcStartAngle
        | PropertyFieldId::GuideArcSweepAngle
        | PropertyFieldId::GuideRepetition
        | PropertyFieldId::GuideStackDirection
        | PropertyFieldId::GuideStackSpacingMultiplier
        | PropertyFieldId::IntersectionDimensions
        | PropertyFieldId::IntersectionMergeEpsilon
        | PropertyFieldId::AlongGuideDimensions
        | PropertyFieldId::AlongGuideIntervalMultiplier
        | PropertyFieldId::AlongGuidePhase
        | PropertyFieldId::RandomCharacter
        | PropertyFieldId::RandomEvenMinimumCenterDistance
        | PropertyFieldId::RandomClusterDensity
        | PropertyFieldId::RandomClusterSpread
        | PropertyFieldId::RandomClusterStrength
        | PropertyFieldId::RandomSeed
        | PropertyFieldId::RandomDensityModulation
        | PropertyFieldId::ArtworkWeightMappingComponent
        | PropertyFieldId::ArtworkWeightMappingPlacement
        | PropertyFieldId::ArtworkWeightMappingInverted
        | PropertyFieldId::ArtworkWeightMappingGain
        | PropertyFieldId::ArtworkWeightMappingBias
        | PropertyFieldId::ArtworkWeightStrength
        | PropertyFieldId::ArtworkWeightResponse
        | PropertyFieldId::RandomExclusion
        | PropertyFieldId::ExclusionMinimumCenterDistance
        | PropertyFieldId::VisibleMarkMargin
        | PropertyFieldId::VisibleMarkSizingPolicy
        | PropertyFieldId::RandomMaximumAttempts
        | PropertyFieldId::RandomMaximumNeighborChecks
        | PropertyFieldId::OutputSiteProduct
        | PropertyFieldId::OutputPrototype
        | PropertyFieldId::OutputAuthoredClosedShape
        | PropertyFieldId::OutputOrientation
        | PropertyFieldId::OutputOrientationDimension => true,
        PropertyFieldId::SourceReference
        | PropertyFieldId::DensityAcrossX
        | PropertyFieldId::DensityAcrossY
        | PropertyFieldId::DensityAspectLocked
        | PropertyFieldId::RotationDegrees
        | PropertyFieldId::TranslationX
        | PropertyFieldId::TranslationY
        | PropertyFieldId::MarkMinimumFill
        | PropertyFieldId::MarkMaximumFill
        | PropertyFieldId::LegacyMappingComponent
        | PropertyFieldId::LegacyMappingPlacement
        | PropertyFieldId::ModeledMappingComponent
        | PropertyFieldId::ModeledMappingPlacement
        | PropertyFieldId::ModeledMappingInverted
        | PropertyFieldId::ModeledMappingGain
        | PropertyFieldId::ModeledMappingBias
        | PropertyFieldId::Paint
        | PropertyFieldId::ColorRed
        | PropertyFieldId::ColorGreen
        | PropertyFieldId::ColorBlue
        | PropertyFieldId::ColorAlpha
        | PropertyFieldId::Opacity
        | PropertyFieldId::Visibility
        | PropertyFieldId::DefinitionSelection => false,
    }
}

#[test]
fn stage17_every_pattern_definition_leaf_obeys_its_cache_contract() {
    struct Case {
        name: &'static str,
        field: PropertyFieldId,
        fixture: Stage17StructuralFixture,
        edit: PatternDefinitionEdit,
        accepts_transition: bool,
    }

    let cases = vec![
        Case {
            name: "coverage guard",
            field: PropertyFieldId::CoverageGuardSteps,
            fixture: Stage17StructuralFixture::Intersections,
            edit: PatternDefinitionEdit::SetCoverageGuardSteps { guard_steps: 3 },
            accepts_transition: true,
        },
        Case {
            name: "coverage support",
            field: PropertyFieldId::CoverageAdditionalMargin,
            fixture: Stage17StructuralFixture::Intersections,
            edit: PatternDefinitionEdit::SetCoverageAdditionalMargin {
                additional_margin: 5.0,
            },
            accepts_transition: true,
        },
        Case {
            name: "guide angle",
            field: PropertyFieldId::GuideBaselineAngle,
            fixture: Stage17StructuralFixture::Intersections,
            edit: PatternDefinitionEdit::SetGuideBaselineAngle {
                mechanism_id: PatternMechanismId(1),
                dimension_id: GuideDimensionId(11),
                baseline_angle_degrees: 18.0,
            },
            accepts_transition: true,
        },
        Case {
            name: "guide phase",
            field: PropertyFieldId::GuidePhase,
            fixture: Stage17StructuralFixture::Intersections,
            edit: PatternDefinitionEdit::SetGuidePhase {
                mechanism_id: PatternMechanismId(1),
                dimension_id: GuideDimensionId(11),
                phase: 1.5,
            },
            accepts_transition: true,
        },
        Case {
            name: "guide spacing",
            field: PropertyFieldId::GuideSpacingMultiplier,
            fixture: Stage17StructuralFixture::Intersections,
            edit: PatternDefinitionEdit::SetGuideSpacingMultiplier {
                mechanism_id: PatternMechanismId(1),
                dimension_id: GuideDimensionId(11),
                spacing_multiplier: 1.1,
            },
            accepts_transition: true,
        },
        Case {
            name: "intersection dimensions",
            field: PropertyFieldId::IntersectionDimensions,
            fixture: Stage17StructuralFixture::Intersections,
            edit: PatternDefinitionEdit::SetIntersectionDimensions {
                mechanism_id: PatternMechanismId(2),
                dimensions: vec![GuideDimensionId(11), GuideDimensionId(12)],
            },
            accepts_transition: true,
        },
        Case {
            name: "intersection epsilon",
            field: PropertyFieldId::IntersectionMergeEpsilon,
            fixture: Stage17StructuralFixture::Intersections,
            edit: PatternDefinitionEdit::SetIntersectionMergeEpsilon {
                mechanism_id: PatternMechanismId(2),
                merge_epsilon: 0.1,
            },
            accepts_transition: true,
        },
        Case {
            name: "along dimensions",
            field: PropertyFieldId::AlongGuideDimensions,
            fixture: Stage17StructuralFixture::AlongGuides,
            edit: PatternDefinitionEdit::SetAlongGuideDimensions {
                mechanism_id: PatternMechanismId(2),
                dimensions: vec![GuideDimensionId(11)],
            },
            accepts_transition: true,
        },
        Case {
            name: "along interval",
            field: PropertyFieldId::AlongGuideIntervalMultiplier,
            fixture: Stage17StructuralFixture::AlongGuides,
            edit: PatternDefinitionEdit::SetAlongGuideIntervalMultiplier {
                mechanism_id: PatternMechanismId(2),
                interval_multiplier: 1.0,
            },
            accepts_transition: true,
        },
        Case {
            name: "along phase",
            field: PropertyFieldId::AlongGuidePhase,
            fixture: Stage17StructuralFixture::AlongGuides,
            edit: PatternDefinitionEdit::SetAlongGuidePhase {
                mechanism_id: PatternMechanismId(2),
                phase: 0.75,
            },
            accepts_transition: true,
        },
        Case {
            name: "random character",
            field: PropertyFieldId::RandomCharacter,
            fixture: Stage17StructuralFixture::RawRandom,
            edit: PatternDefinitionEdit::SetRandomCharacter {
                mechanism_id: PatternMechanismId(101),
                character: RandomSiteCharacter::Even {
                    minimum_center_distance: 5.0,
                },
            },
            accepts_transition: true,
        },
        Case {
            name: "random seed",
            field: PropertyFieldId::RandomSeed,
            fixture: Stage17StructuralFixture::RawRandom,
            edit: PatternDefinitionEdit::SetRandomSeed {
                mechanism_id: PatternMechanismId(101),
                seed: 9,
            },
            accepts_transition: true,
        },
        Case {
            name: "even separation",
            field: PropertyFieldId::RandomEvenMinimumCenterDistance,
            fixture: Stage17StructuralFixture::EvenRandom,
            edit: PatternDefinitionEdit::SetRandomEvenMinimumCenterDistance {
                mechanism_id: PatternMechanismId(101),
                minimum_center_distance: 7.0,
            },
            accepts_transition: true,
        },
        Case {
            name: "cluster density",
            field: PropertyFieldId::RandomClusterDensity,
            fixture: Stage17StructuralFixture::ClusteredRandom,
            edit: PatternDefinitionEdit::SetRandomClusterDensity {
                mechanism_id: PatternMechanismId(101),
                cluster_density: 0.03,
            },
            accepts_transition: true,
        },
        Case {
            name: "cluster spread",
            field: PropertyFieldId::RandomClusterSpread,
            fixture: Stage17StructuralFixture::ClusteredRandom,
            edit: PatternDefinitionEdit::SetRandomClusterSpread {
                mechanism_id: PatternMechanismId(101),
                cluster_spread: 5.0,
            },
            accepts_transition: true,
        },
        Case {
            name: "cluster strength",
            field: PropertyFieldId::RandomClusterStrength,
            fixture: Stage17StructuralFixture::ClusteredRandom,
            edit: PatternDefinitionEdit::SetRandomClusterStrength {
                mechanism_id: PatternMechanismId(101),
                cluster_strength: 0.6,
            },
            accepts_transition: true,
        },
        Case {
            name: "density variant",
            field: PropertyFieldId::RandomDensityModulation,
            fixture: Stage17StructuralFixture::RawRandom,
            edit: PatternDefinitionEdit::SetDensityModulationVariant {
                mechanism_id: PatternMechanismId(102),
                modulation: SiteDensityModulation::ArtworkWeighted {
                    mapping: SourceMapping::canonical(SourceMappingComponent::Luminance),
                    strength: 0.5,
                    response: ArtworkWeightResponse::Smoothstep,
                },
            },
            accepts_transition: true,
        },
        Case {
            name: "artwork component",
            field: PropertyFieldId::ArtworkWeightMappingComponent,
            fixture: Stage17StructuralFixture::WeightedRandom,
            edit: PatternDefinitionEdit::SetArtworkWeightMappingComponent {
                mechanism_id: PatternMechanismId(102),
                component: SourceMappingComponent::Red,
            },
            accepts_transition: true,
        },
        Case {
            name: "artwork placement no-op",
            field: PropertyFieldId::ArtworkWeightMappingPlacement,
            fixture: Stage17StructuralFixture::WeightedRandom,
            edit: PatternDefinitionEdit::SetArtworkWeightMappingPlacement {
                mechanism_id: PatternMechanismId(102),
                placement: SourcePlacement::StretchToCanvas,
            },
            accepts_transition: false,
        },
        Case {
            name: "artwork inverted",
            field: PropertyFieldId::ArtworkWeightMappingInverted,
            fixture: Stage17StructuralFixture::WeightedRandom,
            edit: PatternDefinitionEdit::SetArtworkWeightMappingInverted {
                mechanism_id: PatternMechanismId(102),
                inverted: true,
            },
            accepts_transition: true,
        },
        Case {
            name: "artwork gain",
            field: PropertyFieldId::ArtworkWeightMappingGain,
            fixture: Stage17StructuralFixture::WeightedRandom,
            edit: PatternDefinitionEdit::SetArtworkWeightMappingGain {
                mechanism_id: PatternMechanismId(102),
                gain: 0.5,
            },
            accepts_transition: true,
        },
        Case {
            name: "artwork bias",
            field: PropertyFieldId::ArtworkWeightMappingBias,
            fixture: Stage17StructuralFixture::WeightedRandom,
            edit: PatternDefinitionEdit::SetArtworkWeightMappingBias {
                mechanism_id: PatternMechanismId(102),
                bias: 0.1,
            },
            accepts_transition: true,
        },
        Case {
            name: "artwork strength",
            field: PropertyFieldId::ArtworkWeightStrength,
            fixture: Stage17StructuralFixture::WeightedRandom,
            edit: PatternDefinitionEdit::SetArtworkWeightStrength {
                mechanism_id: PatternMechanismId(102),
                strength: 0.6,
            },
            accepts_transition: true,
        },
        Case {
            name: "artwork response",
            field: PropertyFieldId::ArtworkWeightResponse,
            fixture: Stage17StructuralFixture::WeightedRandom,
            edit: PatternDefinitionEdit::SetArtworkWeightResponse {
                mechanism_id: PatternMechanismId(102),
                response: ArtworkWeightResponse::Linear,
            },
            accepts_transition: true,
        },
        Case {
            name: "exclusion variant",
            field: PropertyFieldId::RandomExclusion,
            fixture: Stage17StructuralFixture::RawRandom,
            edit: PatternDefinitionEdit::SetExclusionVariant {
                mechanism_id: PatternMechanismId(103),
                policy: SiteExclusionPolicy::MinimumCenterDistance { minimum: 4.0 },
            },
            accepts_transition: true,
        },
        Case {
            name: "minimum center",
            field: PropertyFieldId::ExclusionMinimumCenterDistance,
            fixture: Stage17StructuralFixture::MinimumCenterRandom,
            edit: PatternDefinitionEdit::SetExclusionMinimumCenterDistance {
                mechanism_id: PatternMechanismId(103),
                minimum_center_distance: 5.0,
            },
            accepts_transition: true,
        },
        Case {
            name: "visible margin",
            field: PropertyFieldId::VisibleMarkMargin,
            fixture: Stage17StructuralFixture::VisibleMarginRandom,
            edit: PatternDefinitionEdit::SetVisibleMarkMargin {
                mechanism_id: PatternMechanismId(103),
                margin: 0.75,
            },
            accepts_transition: true,
        },
        Case {
            name: "visible sizing no-op",
            field: PropertyFieldId::VisibleMarkSizingPolicy,
            fixture: Stage17StructuralFixture::VisibleMarginRandom,
            edit: PatternDefinitionEdit::SetVisibleMarkSizingPolicy {
                mechanism_id: PatternMechanismId(103),
                sizing: VisibleMarkSizingPolicy::MaximumSupportRadius,
            },
            accepts_transition: false,
        },
        Case {
            name: "attempts",
            field: PropertyFieldId::RandomMaximumAttempts,
            fixture: Stage17StructuralFixture::RawRandom,
            edit: PatternDefinitionEdit::SetRandomMaximumAttempts {
                mechanism_id: PatternMechanismId(104),
                maximum_attempts: 65,
            },
            accepts_transition: true,
        },
        Case {
            name: "neighbor checks",
            field: PropertyFieldId::RandomMaximumNeighborChecks,
            fixture: Stage17StructuralFixture::RawRandom,
            edit: PatternDefinitionEdit::SetRandomMaximumNeighborChecks {
                mechanism_id: PatternMechanismId(104),
                maximum_neighbor_checks: 15_999_999,
            },
            accepts_transition: true,
        },
        Case {
            name: "output site product no-op",
            field: PropertyFieldId::OutputSiteProduct,
            fixture: Stage17StructuralFixture::Intersections,
            edit: PatternDefinitionEdit::SetOutputSiteProduct {
                output_layer_id: PatternOutputLayerId(1),
                site_mechanism_id: PatternMechanismId(2),
            },
            accepts_transition: false,
        },
        Case {
            name: "output prototype no-op",
            field: PropertyFieldId::OutputPrototype,
            fixture: Stage17StructuralFixture::Intersections,
            edit: PatternDefinitionEdit::SetOutputMarkPrototype {
                output_layer_id: PatternOutputLayerId(1),
                prototype: toniator_domain::MarkPrototype::Circle,
            },
            accepts_transition: false,
        },
        Case {
            name: "output orientation",
            field: PropertyFieldId::OutputOrientation,
            fixture: Stage17StructuralFixture::Intersections,
            edit: PatternDefinitionEdit::SetOutputOrientation {
                output_layer_id: PatternOutputLayerId(1),
                orientation: MarkOrientation::GuideNormal {
                    dimension_id: GuideDimensionId(11),
                },
            },
            accepts_transition: true,
        },
        Case {
            name: "output orientation dimension",
            field: PropertyFieldId::OutputOrientationDimension,
            fixture: Stage17StructuralFixture::Intersections,
            edit: PatternDefinitionEdit::SetOutputOrientationDimension {
                output_layer_id: PatternOutputLayerId(1),
                dimension_id: GuideDimensionId(12),
            },
            accepts_transition: true,
        },
    ];

    let expected: BTreeSet<_> = PROPERTY_FIELD_IDS
        .iter()
        .copied()
        .filter(|field| is_pattern_definition_leaf(*field))
        .collect();
    let listed: BTreeSet<_> = cases.iter().map(|case| case.field).collect();
    assert_eq!(
        listed, expected,
        "every structural field has one cache case"
    );

    let bytes = fs::read("../../assets/raster-sample.png").unwrap();
    let source_id = SourceReferenceId::new("fixture-source").unwrap();
    for case in cases {
        let mut history = DocumentHistory::new(stage17_structural_session(case.fixture));
        let scheduler = EvaluationScheduler::new().unwrap();
        let request = |history: &DocumentHistory| {
            EvaluationRequest::new(
                history.session().document_evaluation_snapshot(),
                ResolvedSource::new(source_id.clone(), bytes.clone(), SourceFormatHint::Png)
                    .unwrap(),
            )
        };
        submit_and_accept(&scheduler, history.session(), request(&history));
        let before = history.document().clone();
        let revision = history.revision();
        let command = DocumentCommand::EditSharedPatternDefinition {
            definition_id: PatternDefinitionId(1),
            base_definition: before.pattern_definitions()[0].clone(),
            edit: case.edit,
        };
        assert_eq!(
            command.field_projections()[0].field,
            case.field,
            "{}",
            case.name
        );
        let contract = property_field_contract(case.field);
        if case.accepts_transition {
            let result = history
                .apply(&command)
                .unwrap_or_else(|error| panic!("{}: {error}", case.name));
            assert_eq!(
                result.affected_channels,
                vec![ChannelId(1), ChannelId(2), ChannelId(3)],
                "{}",
                case.name
            );
            assert_eq!(result.invalidation, contract.invalidation, "{}", case.name);
            let completion = submit_and_accept(&scheduler, history.session(), request(&history));
            let diagnostics = completion.cache_diagnostics().unwrap();
            assert_eq!(
                diagnostics.aggregate.decoded_source,
                CacheDisposition::Hit,
                "{}",
                case.name
            );
            assert_eq!(
                diagnostics.aggregate.scene,
                CacheDisposition::Miss,
                "{}",
                case.name
            );
            assert_eq!(
                diagnostics.aggregate.raster,
                CacheDisposition::Miss,
                "{}",
                case.name
            );
            match contract.invalidation {
                InvalidationLevel::Family => {
                    assert_eq!(
                        diagnostics.aggregate.family,
                        CacheDisposition::Miss,
                        "{}",
                        case.name
                    );
                    assert_eq!(
                        diagnostics.aggregate.realization,
                        CacheDisposition::Miss,
                        "{}",
                        case.name
                    );
                }
                InvalidationLevel::Realization => {
                    assert_eq!(
                        diagnostics.aggregate.family,
                        CacheDisposition::Hit,
                        "{}",
                        case.name
                    );
                    assert_eq!(
                        diagnostics.aggregate.realization,
                        CacheDisposition::Miss,
                        "{}",
                        case.name
                    );
                }
                other => panic!(
                    "unexpected structural invalidation {other:?} for {}",
                    case.name
                ),
            }
        } else {
            assert!(
                history.apply(&command).is_err(),
                "{} must reject a semantic no-op",
                case.name
            );
            assert_eq!(history.document(), &before, "{}", case.name);
            assert_eq!(history.revision(), revision, "{}", case.name);
            assert!(!history.can_undo(), "{}", case.name);
            let completion = submit_and_accept(&scheduler, history.session(), request(&history));
            let diagnostics = completion.cache_diagnostics().unwrap();
            assert_eq!(
                diagnostics.aggregate.family,
                CacheDisposition::Hit,
                "{}",
                case.name
            );
            assert_eq!(
                diagnostics.aggregate.realization,
                CacheDisposition::Hit,
                "{}",
                case.name
            );
            assert_eq!(
                diagnostics.aggregate.scene,
                CacheDisposition::Hit,
                "{}",
                case.name
            );
            assert_eq!(
                diagnostics.aggregate.raster,
                CacheDisposition::Hit,
                "{}",
                case.name
            );
        }
        scheduler.shutdown().unwrap();
    }
}

fn session(model: HalftoneChannelModel) -> DocumentSession {
    session_with_canvas(model, 90.0, 60.0)
}

fn session_with_canvas(model: HalftoneChannelModel, width: f64, height: f64) -> DocumentSession {
    let definition = PatternDefinition::supported_straight_grid(
        PatternDefinitionId(1),
        "grid",
        PatternMechanismId(1),
        PatternMechanismId(2),
        PatternOutputLayerId(1),
        CoveragePolicy {
            guard_steps: 2,
            additional_margin: 4.5,
        },
    );
    let channel = ChannelState {
        id: ChannelId(1),
        pattern_definition_id: definition.id,
        layout: ChannelPatternLayout {
            density: DensityMetric2D {
                across_x: width / 10.0,
                across_y: height / 10.0,
                aspect_locked: true,
            },
            rotation_degrees: 0.0,
            translation_x: 0.0,
            translation_y: 0.0,
        },
        appearance: ChannelAppearance {
            visible: true,
            color: ColorValue {
                red: 1.0,
                green: 0.0,
                blue: 0.0,
                alpha: 1.0,
            },
            opacity: 1.0,
        },
        mark_geometry_response: MarkGeometryResponse {
            minimum_fill: 2.0,
            maximum_fill: 9.0,
            rotation_offset_degrees: 0.0,
        },
        source_mapping: ChannelSourceMapping {
            component: SourceComponent::Luminance,
            placement: SourcePlacement::StretchToCanvas,
        },
    };
    let source = SourceReferenceId::new("fixture-source").unwrap();
    let document = Document::with_source(
        DocumentId(1),
        CanvasSpec { width, height },
        SourceReference::Assigned(source),
        vec![definition],
        vec![channel],
    )
    .unwrap();
    let mut session = DocumentSession::new(document).unwrap();
    let template = ChannelTopologyTemplate {
        pattern_definition_id: PatternDefinitionId(1),
        layout: session
            .document()
            .channel(ChannelId(1))
            .unwrap()
            .layout
            .clone(),
        mark_geometry_response: session
            .document()
            .channel(ChannelId(1))
            .unwrap()
            .mark_geometry_response
            .clone(),
    };
    let topology = session
        .document()
        .canonical_channel_topology(model, template)
        .unwrap();
    session
        .apply(&DocumentCommand::ReplaceChannelTopology { model, topology })
        .unwrap();
    session
}

fn generalized_session(model: HalftoneChannelModel, along: bool) -> DocumentSession {
    generalized_session_named(
        model,
        if along {
            GeneralizedConfiguration::AlongGuide
        } else {
            GeneralizedConfiguration::ThreeDirection
        },
    )
}

#[derive(Clone, Copy)]
enum GeneralizedConfiguration {
    Orthogonal,
    Nonorthogonal,
    ThreeDirection,
    FourDirection,
    ParallelAlong,
    AlongGuide,
}

impl GeneralizedConfiguration {
    const fn label(self) -> &'static str {
        match self {
            Self::Orthogonal => "orthogonal",
            Self::Nonorthogonal => "nonorthogonal",
            Self::ThreeDirection => "three-direction",
            Self::FourDirection => "four-direction",
            Self::ParallelAlong => "parallel-one-dimension",
            Self::AlongGuide => "along-guide",
        }
    }
}

fn generalized_session_named(
    model: HalftoneChannelModel,
    configuration: GeneralizedConfiguration,
) -> DocumentSession {
    let dimensions = vec![
        StraightGuideDimension {
            id: GuideDimensionId(11),
            baseline_angle_degrees: 17.0,
            phase: 1.25,
            repetition: StraightGuideRepetition {
                spacing_multiplier: 1.0,
            },
        },
        StraightGuideDimension {
            id: GuideDimensionId(12),
            baseline_angle_degrees: 89.5,
            phase: -2.5,
            repetition: StraightGuideRepetition {
                spacing_multiplier: 0.75,
            },
        },
        StraightGuideDimension {
            id: GuideDimensionId(13),
            baseline_angle_degrees: 137.0,
            phase: 0.0,
            repetition: StraightGuideRepetition {
                spacing_multiplier: 1.0,
            },
        },
    ];
    let (dimensions, product, orientation) = match configuration {
        GeneralizedConfiguration::Orthogonal => {
            let mut dimensions = dimensions[..2].to_vec();
            dimensions[0].baseline_angle_degrees = 0.0;
            dimensions[1].baseline_angle_degrees = 90.0;
            (
                dimensions,
                GeneralizedSiteProduct::Intersections {
                    dimensions: vec![GuideDimensionId(11), GuideDimensionId(12)],
                    merge_epsilon: 1e-9,
                },
                MarkOrientation::Fixed,
            )
        }
        GeneralizedConfiguration::Nonorthogonal => {
            let mut dimensions = dimensions[..2].to_vec();
            dimensions[1].baseline_angle_degrees = 77.0;
            (
                dimensions,
                GeneralizedSiteProduct::Intersections {
                    dimensions: vec![GuideDimensionId(11), GuideDimensionId(12)],
                    merge_epsilon: 1e-9,
                },
                MarkOrientation::GuideNormal {
                    dimension_id: GuideDimensionId(12),
                },
            )
        }
        GeneralizedConfiguration::ThreeDirection => (
            dimensions[..3].to_vec(),
            GeneralizedSiteProduct::Intersections {
                dimensions: vec![
                    GuideDimensionId(11),
                    GuideDimensionId(12),
                    GuideDimensionId(13),
                ],
                merge_epsilon: 1e-9,
            },
            MarkOrientation::GuideTangent {
                dimension_id: GuideDimensionId(11),
            },
        ),
        GeneralizedConfiguration::FourDirection => {
            let mut dimensions = dimensions;
            dimensions.push(StraightGuideDimension {
                id: GuideDimensionId(14),
                baseline_angle_degrees: 45.0,
                phase: 0.75,
                repetition: StraightGuideRepetition {
                    spacing_multiplier: 1.25,
                },
            });
            (
                dimensions,
                GeneralizedSiteProduct::Intersections {
                    dimensions: vec![
                        GuideDimensionId(11),
                        GuideDimensionId(12),
                        GuideDimensionId(13),
                        GuideDimensionId(14),
                    ],
                    merge_epsilon: 1e-9,
                },
                MarkOrientation::GuideTangent {
                    dimension_id: GuideDimensionId(14),
                },
            )
        }
        GeneralizedConfiguration::ParallelAlong => (
            vec![StraightGuideDimension {
                id: GuideDimensionId(11),
                baseline_angle_degrees: 17.0,
                phase: 1.25,
                repetition: StraightGuideRepetition {
                    spacing_multiplier: 1.0,
                },
            }],
            GeneralizedSiteProduct::AlongGuides {
                dimensions: vec![GuideDimensionId(11)],
                interval_multiplier: 0.75,
                phase: 0.5,
            },
            MarkOrientation::GuideNormal {
                dimension_id: GuideDimensionId(11),
            },
        ),
        GeneralizedConfiguration::AlongGuide => (
            dimensions[..3].to_vec(),
            GeneralizedSiteProduct::AlongGuides {
                dimensions: vec![GuideDimensionId(11), GuideDimensionId(13)],
                interval_multiplier: 0.75,
                phase: 0.5,
            },
            MarkOrientation::GuideTangent {
                dimension_id: GuideDimensionId(11),
            },
        ),
    };
    let definition = PatternDefinition::generalized_straight_guides(
        PatternDefinitionId(1),
        "data-only",
        PatternMechanismId(1),
        PatternMechanismId(2),
        PatternOutputLayerId(1),
        dimensions,
        product,
        orientation,
        CoveragePolicy {
            guard_steps: 2,
            additional_margin: 4.5,
        },
    );
    let source = SourceReferenceId::new("fixture-source").unwrap();
    let channel = ChannelState {
        id: ChannelId(1),
        pattern_definition_id: definition.id,
        layout: ChannelPatternLayout {
            density: DensityMetric2D {
                across_x: 9.0,
                across_y: 6.0,
                aspect_locked: false,
            },
            rotation_degrees: 17.0,
            translation_x: 3.25,
            translation_y: -4.5,
        },
        appearance: ChannelAppearance {
            visible: true,
            color: ColorValue {
                red: 1.0,
                green: 0.0,
                blue: 0.0,
                alpha: 1.0,
            },
            opacity: 1.0,
        },
        mark_geometry_response: MarkGeometryResponse {
            minimum_fill: 2.0,
            maximum_fill: 9.0,
            rotation_offset_degrees: 0.0,
        },
        source_mapping: ChannelSourceMapping {
            component: SourceComponent::Luminance,
            placement: SourcePlacement::StretchToCanvas,
        },
    };
    let document = Document::with_source(
        DocumentId(1),
        CanvasSpec {
            width: 90.0,
            height: 60.0,
        },
        SourceReference::Assigned(source),
        vec![definition],
        vec![channel],
    )
    .unwrap();
    let mut session = DocumentSession::new(document).unwrap();
    let template = ChannelTopologyTemplate {
        pattern_definition_id: PatternDefinitionId(1),
        layout: session
            .document()
            .channel(ChannelId(1))
            .unwrap()
            .layout
            .clone(),
        mark_geometry_response: session
            .document()
            .channel(ChannelId(1))
            .unwrap()
            .mark_geometry_response
            .clone(),
    };
    let topology = session
        .document()
        .canonical_channel_topology(model, template)
        .unwrap();
    session
        .apply(&DocumentCommand::ReplaceChannelTopology { model, topology })
        .unwrap();
    session
}

fn generalized_session_with_definition(
    model: HalftoneChannelModel,
    configuration: GeneralizedConfiguration,
    definition: PatternDefinition,
    source: SourceReference,
    canvas: CanvasSpec,
) -> DocumentSession {
    let base = generalized_session_named(model, configuration);
    let document = Document::with_source_and_topology(
        base.document().id(),
        canvas,
        source,
        vec![definition],
        model,
        base.document().channel_topology().unwrap().clone(),
    )
    .unwrap();
    DocumentSession::new(document).unwrap()
}

#[test]
fn canonical_modeled_topologies_evaluate_in_authoritative_order() {
    for (model, expected) in [
        (
            HalftoneChannelModel::Rgb,
            &[
                (HalftoneChannelRole::Red, ChannelId(1)),
                (HalftoneChannelRole::Green, ChannelId(2)),
                (HalftoneChannelRole::Blue, ChannelId(3)),
            ][..],
        ),
        (
            HalftoneChannelModel::Cmyk,
            &[
                (HalftoneChannelRole::Cyan, ChannelId(4)),
                (HalftoneChannelRole::Magenta, ChannelId(5)),
                (HalftoneChannelRole::Yellow, ChannelId(6)),
                (HalftoneChannelRole::Black, ChannelId(7)),
            ][..],
        ),
        (
            HalftoneChannelModel::SourceColorAlpha,
            &[(HalftoneChannelRole::SourceColor, ChannelId(8))][..],
        ),
    ] {
        let session = session(model);
        let source_id = SourceReferenceId::new("fixture-source").unwrap();
        let request = EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(
                source_id,
                std::fs::read("../../assets/raster-sample.png").unwrap(),
                SourceFormatHint::Png,
            )
            .unwrap(),
        );
        let result = evaluate(request).unwrap();
        assert_eq!(result.scene().model(), Some(model));
        assert_eq!(result.channels().len(), expected.len());
        assert_eq!(result.scene().layers().len(), expected.len());
        assert_eq!(
            result
                .channels()
                .iter()
                .map(|summary| (summary.role(), summary.channel_id()))
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(
            result
                .scene()
                .layers()
                .iter()
                .map(|layer| layer.channel_id())
                .collect::<Vec<_>>(),
            expected
                .iter()
                .map(|(_, channel_id)| *channel_id)
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn immutable_baselines_are_deterministic_uncached_and_keep_decoded_pixels_downstream() {
    let source_id = SourceReferenceId::new("fixture-source").unwrap();
    let mut identities = Vec::new();
    for (path, format) in [
        ("../../assets/raster-sample.png", SourceFormatHint::Png),
        ("../../assets/vector-sample.svg", SourceFormatHint::Svg),
    ] {
        let bytes = std::fs::read(path).unwrap();
        let first = evaluate(EvaluationRequest::new(
            session(HalftoneChannelModel::SourceColorAlpha).document_evaluation_snapshot(),
            ResolvedSource::new(source_id.clone(), bytes.clone(), format).unwrap(),
        ))
        .unwrap();
        let second = evaluate(EvaluationRequest::new(
            session(HalftoneChannelModel::SourceColorAlpha).document_evaluation_snapshot(),
            ResolvedSource::new(source_id.clone(), bytes, format).unwrap(),
        ))
        .unwrap();
        assert_eq!(first, second);
        assert_eq!(first.raster(), second.raster());
        assert_eq!(write_svg(first.scene()), write_svg(second.scene()));
        identities.push((
            first.source_identity().decoded_pixel_hash.clone(),
            first.channels()[0].realization_identity().to_owned(),
        ));
    }
    assert_ne!(identities[0].0, identities[1].0);
    assert_ne!(identities[0].1, identities[1].1);
}

#[test]
fn cached_document_results_match_uncached_results_for_both_immutable_baselines() {
    let source_id = SourceReferenceId::new("fixture-source").unwrap();
    for (path, format) in [
        ("../../assets/raster-sample.png", SourceFormatHint::Png),
        ("../../assets/vector-sample.svg", SourceFormatHint::Svg),
    ] {
        let bytes = std::fs::read(path).unwrap();
        let session = session(HalftoneChannelModel::SourceColorAlpha);
        let uncached = evaluate(EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(source_id.clone(), bytes.clone(), format).unwrap(),
        ))
        .unwrap();
        let scheduler = EvaluationScheduler::new().unwrap();

        let first = submit_and_accept(
            &scheduler,
            &session,
            EvaluationRequest::new(
                session.document_evaluation_snapshot(),
                ResolvedSource::new(source_id.clone(), bytes.clone(), format).unwrap(),
            ),
        );
        let first_diagnostics = first.cache_diagnostics().unwrap();
        assert_eq!(
            first_diagnostics.aggregate,
            toniator_engine::CacheDiagnostics {
                decoded_source: CacheDisposition::Miss,
                family: CacheDisposition::Miss,
                realization: CacheDisposition::Miss,
                scene: CacheDisposition::Miss,
                raster: CacheDisposition::Miss,
            }
        );
        assert!(first_diagnostics.channels.iter().all(|channel| {
            channel.family == CacheDisposition::Miss
                && channel.realization == CacheDisposition::Miss
        }));
        let first_result = first.result().unwrap();

        let repeated = submit_and_accept(
            &scheduler,
            &session,
            EvaluationRequest::new(
                session.document_evaluation_snapshot(),
                ResolvedSource::new(source_id.clone(), bytes, format).unwrap(),
            ),
        );
        let repeated_diagnostics = repeated.cache_diagnostics().unwrap();
        assert_eq!(
            repeated_diagnostics.aggregate,
            toniator_engine::CacheDiagnostics {
                decoded_source: CacheDisposition::Hit,
                family: CacheDisposition::Hit,
                realization: CacheDisposition::Hit,
                scene: CacheDisposition::Hit,
                raster: CacheDisposition::Hit,
            }
        );
        assert!(repeated_diagnostics.channels.iter().all(|channel| {
            channel.family == CacheDisposition::Hit && channel.realization == CacheDisposition::Hit
        }));
        let repeated_result = repeated.result().unwrap();
        assert_eq!(uncached, *first_result);
        assert_eq!(uncached, *repeated_result);
        assert_eq!(uncached.channels(), first_result.channels());
        assert_eq!(uncached.channels(), repeated_result.channels());
        assert_eq!(uncached.raster(), first_result.raster());
        assert_eq!(uncached.raster(), repeated_result.raster());
        assert_eq!(write_svg(uncached.scene()), write_svg(first_result.scene()));
        assert_eq!(
            write_svg(uncached.scene()),
            write_svg(repeated_result.scene())
        );

        scheduler.shutdown().unwrap();
    }
}

#[test]
fn history_undo_restores_all_models_and_baselines_and_rejects_held_completions() {
    let source_id = SourceReferenceId::new("fixture-source").unwrap();
    for model in [
        HalftoneChannelModel::Rgb,
        HalftoneChannelModel::Cmyk,
        HalftoneChannelModel::SourceColorAlpha,
    ] {
        for (path, format) in [
            ("../../assets/raster-sample.png", SourceFormatHint::Png),
            ("../../assets/vector-sample.svg", SourceFormatHint::Svg),
        ] {
            let bytes = std::fs::read(path).unwrap();
            let mut history = DocumentHistory::new(session(model));
            let baseline = evaluate(EvaluationRequest::new(
                history.session().document_evaluation_snapshot(),
                ResolvedSource::new(source_id.clone(), bytes.clone(), format).unwrap(),
            ))
            .unwrap();

            let scheduler = EvaluationScheduler::new().unwrap();
            let held_ticket = scheduler
                .submit(EvaluationRequest::new(
                    history.session().document_evaluation_snapshot(),
                    ResolvedSource::new(source_id.clone(), bytes.clone(), format).unwrap(),
                ))
                .unwrap();
            let held = wait_for_latest(&scheduler);
            assert_eq!(held.ticket(), held_ticket);

            let channel_id = baseline.channels()[0].channel_id();
            history
                .apply(&DocumentCommand::SetVisibility {
                    channel_id,
                    visible: false,
                })
                .unwrap();
            assert!(
                !scheduler
                    .accept_completion(&held, history.session())
                    .unwrap(),
                "{model:?} {path} held completion must be stale after history apply"
            );
            history.undo().unwrap();

            let restored = evaluate(EvaluationRequest::new(
                history.session().document_evaluation_snapshot(),
                ResolvedSource::new(source_id.clone(), bytes, format).unwrap(),
            ))
            .unwrap();
            assert_eq!(restored.scene(), baseline.scene(), "{model:?} {path}");
            assert_eq!(
                restored.raster().pixels(),
                baseline.raster().pixels(),
                "{model:?} {path}"
            );
            assert_eq!(
                write_svg(restored.scene()),
                write_svg(baseline.scene()),
                "{model:?} {path}"
            );
            scheduler.shutdown().unwrap();
        }
    }
}

#[test]
fn invisible_rgb_layer_remains_ordered_authoritative_geometry() {
    let mut session = session(HalftoneChannelModel::Rgb);
    session
        .apply(&DocumentCommand::SetVisibility {
            channel_id: ChannelId(2),
            visible: false,
        })
        .unwrap();
    let source_id = SourceReferenceId::new("fixture-source").unwrap();
    let bytes = std::fs::read("../../assets/raster-sample.png").unwrap();
    let result = evaluate(EvaluationRequest::new(
        session.document_evaluation_snapshot(),
        ResolvedSource::new(source_id, bytes, SourceFormatHint::Png).unwrap(),
    ))
    .unwrap();
    assert_eq!(result.channels().len(), 3);
    assert_eq!(result.scene().layers().len(), 3);
    assert_eq!(result.scene().layers()[1].channel_id(), ChannelId(2));
    assert!(!result.scene().layers()[1].visible());
    assert!(
        matches!(result.scene().layers()[1].geometry(), toniator_engine::GeometryOutput::CircularMarks(marks) if !marks.is_empty())
    );
}

#[test]
fn complete_document_checks_candidate_limit_for_required_channels() {
    let source_id = SourceReferenceId::new("fixture-source").unwrap();
    let bytes = std::fs::read("../../assets/raster-sample.png").unwrap();
    let limited = evaluate_with_limits(
        EvaluationRequest::new(
            session(HalftoneChannelModel::Rgb).document_evaluation_snapshot(),
            ResolvedSource::new(source_id.clone(), bytes.clone(), SourceFormatHint::Png).unwrap(),
        ),
        EvaluationLimits::new(1).unwrap(),
    )
    .unwrap_err();
    assert_eq!(limited.path(), "coverage.candidate_limit");
    assert!(
        evaluate_with_limits(
            EvaluationRequest::new(
                session(HalftoneChannelModel::Rgb).document_evaluation_snapshot(),
                ResolvedSource::new(source_id, bytes, SourceFormatHint::Png).unwrap()
            ),
            EvaluationLimits::new(100_000).unwrap()
        )
        .is_ok()
    );
}

#[test]
fn successful_candidate_policies_preserve_family_and_realization_content_identities() {
    let source_id = SourceReferenceId::new("fixture-source").unwrap();
    let bytes = std::fs::read("../../assets/raster-sample.png").unwrap();
    let first = evaluate_with_limits(
        EvaluationRequest::new(
            session(HalftoneChannelModel::Rgb).document_evaluation_snapshot(),
            ResolvedSource::new(source_id.clone(), bytes.clone(), SourceFormatHint::Png).unwrap(),
        ),
        EvaluationLimits::new(100_000).unwrap(),
    )
    .unwrap();
    let second = evaluate_with_limits(
        EvaluationRequest::new(
            session(HalftoneChannelModel::Rgb).document_evaluation_snapshot(),
            ResolvedSource::new(source_id, bytes, SourceFormatHint::Png).unwrap(),
        ),
        EvaluationLimits::new(100_001).unwrap(),
    )
    .unwrap();
    assert_eq!(
        first
            .channels()
            .iter()
            .map(|channel| channel.family_identity())
            .collect::<Vec<_>>(),
        second
            .channels()
            .iter()
            .map(|channel| channel.family_identity())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        first
            .channels()
            .iter()
            .map(|channel| channel.realization_identity())
            .collect::<Vec<_>>(),
        second
            .channels()
            .iter()
            .map(|channel| channel.realization_identity())
            .collect::<Vec<_>>()
    );
}

#[test]
fn scheduler_rebuilds_families_and_realizations_when_canvas_changes() {
    let source_id = SourceReferenceId::new("fixture-source").unwrap();
    let source_bytes = std::fs::read("../../assets/raster-sample.png").unwrap();
    let baseline = session_with_canvas(HalftoneChannelModel::Rgb, 90.0, 60.0);
    let resized = session_with_canvas(HalftoneChannelModel::Rgb, 100.0, 60.0);
    let scheduler = EvaluationScheduler::new().unwrap();

    submit_and_accept(
        &scheduler,
        &baseline,
        EvaluationRequest::new(
            baseline.document_evaluation_snapshot(),
            ResolvedSource::new(
                source_id.clone(),
                source_bytes.clone(),
                SourceFormatHint::Png,
            )
            .unwrap(),
        ),
    );
    let resized_completion = submit_and_accept(
        &scheduler,
        &resized,
        EvaluationRequest::new(
            resized.document_evaluation_snapshot(),
            ResolvedSource::new(source_id, source_bytes, SourceFormatHint::Png).unwrap(),
        ),
    );
    let diagnostics = resized_completion.cache_diagnostics().unwrap();
    assert_eq!(diagnostics.aggregate.decoded_source, CacheDisposition::Hit);
    assert_eq!(diagnostics.aggregate.family, CacheDisposition::Miss);
    assert_eq!(diagnostics.aggregate.realization, CacheDisposition::Miss);
    assert_eq!(diagnostics.aggregate.scene, CacheDisposition::Miss);
    assert_eq!(diagnostics.aggregate.raster, CacheDisposition::Miss);
    assert!(diagnostics.channels.iter().all(|channel| {
        channel.family == CacheDisposition::Miss && channel.realization == CacheDisposition::Miss
    }));

    scheduler.shutdown().unwrap();
}

#[test]
fn frozen_v1_migration_and_saved_v2_preserve_accepted_outputs_for_every_model() {
    let validation = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/validation/stage-15");
    fs::create_dir_all(&validation).unwrap();
    for (fixture, source_format) in [
        ("raster-sample-v1.toniator", SourceFormatHint::Png),
        ("vector-sample-v1.toniator", SourceFormatHint::Svg),
    ] {
        let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets")
            .join(fixture);
        let migrated = load(&fixture_path).unwrap();
        let source = migrated.sources().entries().next().unwrap();
        let original_session = DocumentSession::new(migrated.document().clone()).unwrap();
        let original = evaluate_with_limits(
            EvaluationRequest::new(
                original_session.document_evaluation_snapshot(),
                ResolvedSource::new(source.id().clone(), source.bytes().to_vec(), source_format)
                    .unwrap(),
            ),
            EvaluationLimits::default(),
        )
        .unwrap();
        let seed = migrated.document().channel_topology().unwrap().channels()[0].clone();
        for model in [
            HalftoneChannelModel::Rgb,
            HalftoneChannelModel::Cmyk,
            HalftoneChannelModel::SourceColorAlpha,
        ] {
            let topology = migrated
                .document()
                .canonical_channel_topology(
                    model,
                    ChannelTopologyTemplate {
                        pattern_definition_id: seed.pattern_definition_id,
                        layout: seed.layout.clone(),
                        mark_geometry_response: seed.mark_geometry_response.clone(),
                    },
                )
                .unwrap();
            let document = Document::with_source_and_topology(
                migrated.document().id(),
                migrated.document().canvas().clone(),
                migrated.document().source().clone(),
                migrated.document().pattern_definitions().to_vec(),
                model,
                topology,
            )
            .unwrap();
            let current_session = DocumentSession::new(document.clone()).unwrap();
            let current = evaluate_with_limits(
                EvaluationRequest::new(
                    current_session.document_evaluation_snapshot(),
                    ResolvedSource::new(
                        source.id().clone(),
                        source.bytes().to_vec(),
                        source_format,
                    )
                    .unwrap(),
                ),
                EvaluationLimits::default(),
            )
            .unwrap();
            if model == HalftoneChannelModel::Rgb {
                assert_eq!(current.channels(), original.channels());
                assert_eq!(current.scene().identity(), original.scene().identity());
                assert_eq!(current.raster().pixels(), original.raster().pixels());
                assert_eq!(write_svg(current.scene()), write_svg(original.scene()));
            }
            let label = fixture.trim_end_matches(".toniator");
            let v1_png = encode_png(current.raster()).unwrap();
            let v1_svg = write_svg(current.scene());
            fs::write(
                validation.join(format!("{label}-{model:?}-frozen-v1.png")),
                &v1_png,
            )
            .unwrap();
            fs::write(
                validation.join(format!("{label}-{model:?}-frozen-v1.svg")),
                &v1_svg,
            )
            .unwrap();
            let saved = validation.join(format!(
                "{}-{model:?}-saved-v2.toniator",
                fixture.trim_end_matches(".toniator")
            ));
            save(&saved, &document, migrated.sources()).unwrap();
            let expected_archive_hash = match (fixture, model) {
                ("raster-sample-v1.toniator", HalftoneChannelModel::Rgb) => {
                    "7135531041b8a4f9136731267b356ce4b3acbdb74c6e12c6670817e0613436cf"
                }
                ("raster-sample-v1.toniator", HalftoneChannelModel::Cmyk) => {
                    "9aa1ec4c5fe5fca6b023278719ebe56160ec526617ec46eb2f4864277c3ea588"
                }
                ("raster-sample-v1.toniator", HalftoneChannelModel::SourceColorAlpha) => {
                    "1137a5bd4ccc0905087081ff62aa70feb0bf195a7c10272b12bfc323760db6d2"
                }
                ("vector-sample-v1.toniator", HalftoneChannelModel::Rgb) => {
                    "b2d6f3116d9b5aa4bef37d89268be5aa6092a9eb195b33049d53ecad7e910d97"
                }
                ("vector-sample-v1.toniator", HalftoneChannelModel::Cmyk) => {
                    "9424c9d9278fe0e4780a1b4c2ba7688a8b46292c1be7e24144fb3ce1ae81041a"
                }
                ("vector-sample-v1.toniator", HalftoneChannelModel::SourceColorAlpha) => {
                    "419e16a7e8b6de45799dd3780e8c2a781e5050ede74dfd2a33cb114097e0b515"
                }
                _ => unreachable!("fixed frozen-v1/model matrix"),
            };
            assert_eq!(
                format!("{:x}", Sha256::digest(fs::read(&saved).unwrap())),
                expected_archive_hash,
                "additive current-v2 DTO variants must not alter saved Stage 15 bytes"
            );
            let reopened = load(&saved).unwrap();
            assert_eq!(reopened.versions().document(), 2);
            let reopened_session = DocumentSession::new(reopened.document().clone()).unwrap();
            let reopened_result = evaluate_with_limits(
                EvaluationRequest::new(
                    reopened_session.document_evaluation_snapshot(),
                    ResolvedSource::new(
                        source.id().clone(),
                        source.bytes().to_vec(),
                        source_format,
                    )
                    .unwrap(),
                ),
                EvaluationLimits::default(),
            )
            .unwrap();
            assert_eq!(reopened_result.channels(), current.channels());
            assert_eq!(
                reopened_result.scene().identity(),
                current.scene().identity()
            );
            assert_eq!(reopened_result.raster().pixels(), current.raster().pixels());
            assert_eq!(
                write_svg(reopened_result.scene()),
                write_svg(current.scene())
            );
            let v2_png = encode_png(reopened_result.raster()).unwrap();
            let v2_svg = write_svg(reopened_result.scene());
            assert_eq!(v2_png, v1_png);
            assert_eq!(v2_svg, v1_svg);
            fs::write(
                validation.join(format!("{label}-{model:?}-saved-reopened-v2.png")),
                v2_png,
            )
            .unwrap();
            fs::write(
                validation.join(format!("{label}-{model:?}-saved-reopened-v2.svg")),
                v2_svg,
            )
            .unwrap();
        }
    }
}

#[test]
fn preview_target_changes_reuse_scene_and_miss_only_raster_then_repeat_hits() {
    let session = session(HalftoneChannelModel::Rgb);
    let scheduler = EvaluationScheduler::new().unwrap();
    let bytes = std::fs::read("../../assets/raster-sample.png").unwrap();
    let source = || {
        ResolvedSource::new(
            SourceReferenceId::new("fixture-source").unwrap(),
            bytes.clone(),
            SourceFormatHint::Png,
        )
        .unwrap()
    };
    let first = submit_and_accept(
        &scheduler,
        &session,
        EvaluationRequest::with_preview_target(
            session.document_evaluation_snapshot(),
            source(),
            PreviewRasterTarget::new(320, 180).unwrap(),
        ),
    );
    assert_eq!(
        (
            first.result().unwrap().raster().width(),
            first.result().unwrap().raster().height()
        ),
        (320, 180)
    );
    let changed = submit_and_accept(
        &scheduler,
        &session,
        EvaluationRequest::with_preview_target(
            session.document_evaluation_snapshot(),
            source(),
            PreviewRasterTarget::new(480, 270).unwrap(),
        ),
    );
    let diagnostics = changed.cache_diagnostics().unwrap();
    assert_eq!(diagnostics.aggregate.decoded_source, CacheDisposition::Hit);
    assert_eq!(diagnostics.aggregate.family, CacheDisposition::Hit);
    assert_eq!(diagnostics.aggregate.realization, CacheDisposition::Hit);
    assert_eq!(diagnostics.aggregate.scene, CacheDisposition::Hit);
    assert_eq!(diagnostics.aggregate.raster, CacheDisposition::Miss);
    let repeated = submit_and_accept(
        &scheduler,
        &session,
        EvaluationRequest::with_preview_target(
            session.document_evaluation_snapshot(),
            source(),
            PreviewRasterTarget::new(480, 270).unwrap(),
        ),
    );
    assert_eq!(
        repeated.cache_diagnostics().unwrap().aggregate.raster,
        CacheDisposition::Hit
    );
    scheduler.shutdown().unwrap();
}

#[test]
fn reddit_inputs_evaluate_intrinsically_to_large_preview_targets_for_every_model() {
    for (path, format, width, height) in [
        (
            "../../assets/Reddit.png",
            SourceFormatHint::Png,
            128.0,
            128.0,
        ),
        ("../../assets/Reddit.svg", SourceFormatHint::Svg, 14.0, 14.0),
    ] {
        let bytes = std::fs::read(path).unwrap();
        for model in [
            HalftoneChannelModel::Rgb,
            HalftoneChannelModel::Cmyk,
            HalftoneChannelModel::SourceColorAlpha,
        ] {
            let session = session_with_canvas(model, width, height);
            let channel = &session.document().channel_topology().unwrap().channels()[0];
            assert_eq!(channel.layout.density.across_x, width / 10.0);
            assert_eq!(channel.layout.density.across_y, height / 10.0);
            let request = EvaluationRequest::with_preview_target(
                session.document_evaluation_snapshot(),
                ResolvedSource::new(
                    SourceReferenceId::new("fixture-source").unwrap(),
                    bytes.clone(),
                    format,
                )
                .unwrap(),
                PreviewRasterTarget::new(512, 512).unwrap(),
            );
            let result = evaluate(request).unwrap();
            assert_eq!(result.scene().canvas(), &CanvasSpec { width, height });
            assert_eq!(
                (result.raster().width(), result.raster().height()),
                (512, 512)
            );
            assert_eq!(result.raster().pixels().len(), 512 * 512 * 4);
        }
    }
}

#[test]
fn newer_preview_target_ticket_rejects_held_older_completion() {
    let session = session(HalftoneChannelModel::Rgb);
    let scheduler = EvaluationScheduler::new().unwrap();
    let bytes = std::fs::read("../../assets/raster-sample.png").unwrap();
    let request = |target| {
        EvaluationRequest::with_preview_target(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(
                SourceReferenceId::new("fixture-source").unwrap(),
                bytes.clone(),
                SourceFormatHint::Png,
            )
            .unwrap(),
            PreviewRasterTarget::new(target, target).unwrap(),
        )
    };
    let ticket_a = scheduler.submit(request(256)).unwrap();
    let completion_a = wait_for_latest(&scheduler);
    assert_eq!(completion_a.ticket(), ticket_a);
    let ticket_b = scheduler.submit(request(512)).unwrap();
    assert!(
        !scheduler
            .accept_completion(&completion_a, &session)
            .unwrap()
    );
    let completion_b = wait_for_latest(&scheduler);
    assert_eq!(completion_b.ticket(), ticket_b);
    assert!(
        scheduler
            .accept_completion(&completion_b, &session)
            .unwrap()
    );
    assert_eq!(
        (
            completion_b.result().unwrap().raster().width(),
            completion_b.result().unwrap().raster().height()
        ),
        (512, 512)
    );
    scheduler.shutdown().unwrap();
}

#[test]
fn splash_preview_clips_letterbox_rows_for_all_models() {
    let bytes = std::fs::read("../../assets/splash.png").unwrap();
    for model in [
        HalftoneChannelModel::Rgb,
        HalftoneChannelModel::Cmyk,
        HalftoneChannelModel::SourceColorAlpha,
    ] {
        let session = session_with_canvas(model, 1280.0, 640.0);
        let result = evaluate(EvaluationRequest::with_preview_target(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(
                SourceReferenceId::new("fixture-source").unwrap(),
                bytes.clone(),
                SourceFormatHint::Png,
            )
            .unwrap(),
            PreviewRasterTarget::new(960, 720).unwrap(),
        ))
        .unwrap();
        assert_eq!(
            result.scene().canvas(),
            &CanvasSpec {
                width: 1280.0,
                height: 640.0
            }
        );
        let raster = result.raster();
        assert_eq!((raster.width(), raster.height()), (960, 720));
        for y in 0..120 {
            assert!(
                raster.pixels()[y * 960 * 4..(y + 1) * 960 * 4]
                    .chunks_exact(4)
                    .all(|p| p[3] == 0)
            );
        }
        for y in 600..720 {
            assert!(
                raster.pixels()[y * 960 * 4..(y + 1) * 960 * 4]
                    .chunks_exact(4)
                    .all(|p| p[3] == 0)
            );
        }
        assert!(
            raster.pixels()[120 * 960 * 4..600 * 960 * 4]
                .chunks_exact(4)
                .any(|p| p[3] > 0)
        );
    }
}

#[test]
fn complete_document_rejects_mismatched_resolved_source_before_evaluation() {
    let other = SourceReferenceId::new("other-source").unwrap();
    let error = evaluate(EvaluationRequest::new(
        session(HalftoneChannelModel::Rgb).document_evaluation_snapshot(),
        ResolvedSource::new(other, vec![1], SourceFormatHint::Png).unwrap(),
    ))
    .unwrap_err();
    assert_eq!(error.path(), "evaluation.source_reference");
}

#[test]
fn complete_document_rejects_unassigned_modeled_source() {
    let session = session(HalftoneChannelModel::Rgb);
    // Rebuild the accepted modeled topology on an otherwise identical base
    // document whose source was never assigned.
    let document = session.document().clone();
    let source_id = SourceReferenceId::new("fixture-source").unwrap();
    let topology = document.channel_topology().unwrap().clone();
    let base = Document::new(
        DocumentId(9),
        document.canvas().clone(),
        document.pattern_definitions().to_vec(),
        vec![ChannelState {
            id: ChannelId(1),
            pattern_definition_id: PatternDefinitionId(1),
            layout: document.channel_topology().unwrap().channels()[0]
                .layout
                .clone(),
            appearance: ChannelAppearance {
                visible: true,
                color: ColorValue {
                    red: 1.0,
                    green: 0.0,
                    blue: 0.0,
                    alpha: 1.0,
                },
                opacity: 1.0,
            },
            mark_geometry_response: document.channel_topology().unwrap().channels()[0]
                .mark_geometry_response
                .clone(),
            source_mapping: ChannelSourceMapping {
                component: SourceComponent::Luminance,
                placement: SourcePlacement::StretchToCanvas,
            },
        }],
    )
    .unwrap();
    let mut unassigned = DocumentSession::new(base).unwrap();
    unassigned
        .apply(&DocumentCommand::ReplaceChannelTopology {
            model: HalftoneChannelModel::Rgb,
            topology,
        })
        .unwrap();
    let error = evaluate(EvaluationRequest::new(
        unassigned.document_evaluation_snapshot(),
        ResolvedSource::new(
            source_id,
            std::fs::read("../../assets/raster-sample.png").unwrap(),
            SourceFormatHint::Png,
        )
        .unwrap(),
    ))
    .unwrap_err();
    assert_eq!(error.path(), "evaluation.source_reference");
}

#[test]
fn scheduler_commits_then_reuses_the_accepted_complete_document_cache() {
    let session = session(HalftoneChannelModel::Rgb);
    let source_id = SourceReferenceId::new("fixture-source").unwrap();
    let source_bytes = std::fs::read("../../assets/raster-sample.png").unwrap();
    let scheduler = EvaluationScheduler::new().unwrap();

    let first = submit_and_accept(
        &scheduler,
        &session,
        EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(
                source_id.clone(),
                source_bytes.clone(),
                SourceFormatHint::Png,
            )
            .unwrap(),
        ),
    );
    let first_diagnostics = first.cache_diagnostics().unwrap();
    assert_eq!(
        first_diagnostics.aggregate,
        toniator_engine::CacheDiagnostics {
            decoded_source: CacheDisposition::Miss,
            family: CacheDisposition::Miss,
            realization: CacheDisposition::Miss,
            scene: CacheDisposition::Miss,
            raster: CacheDisposition::Miss,
        }
    );
    assert_eq!(first_diagnostics.channels.len(), 3);
    assert!(first_diagnostics.channels.iter().all(|channel| {
        channel.family == CacheDisposition::Miss && channel.realization == CacheDisposition::Miss
    }));
    let first_result = first.result().unwrap().clone();

    let repeated = submit_and_accept(
        &scheduler,
        &session,
        EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(source_id, source_bytes, SourceFormatHint::Png).unwrap(),
        ),
    );
    let repeated_diagnostics = repeated.cache_diagnostics().unwrap();
    assert_eq!(
        repeated_diagnostics.aggregate,
        toniator_engine::CacheDiagnostics {
            decoded_source: CacheDisposition::Hit,
            family: CacheDisposition::Hit,
            realization: CacheDisposition::Hit,
            scene: CacheDisposition::Hit,
            raster: CacheDisposition::Hit,
        }
    );
    assert_eq!(repeated_diagnostics.channels.len(), 3);
    assert!(repeated_diagnostics.channels.iter().all(|channel| {
        channel.family == CacheDisposition::Hit && channel.realization == CacheDisposition::Hit
    }));
    let repeated_result = repeated.result().unwrap();
    assert_eq!(repeated_result, &first_result);
    assert_eq!(repeated_result.raster(), first_result.raster());
    assert_eq!(
        write_svg(repeated_result.scene()),
        write_svg(first_result.scene())
    );

    scheduler.shutdown().unwrap();
}

#[test]
fn generalized_intersection_and_along_products_share_the_complete_cache_and_consumer_boundary() {
    let source_id = SourceReferenceId::new("fixture-source").unwrap();
    let source_bytes = std::fs::read("../../assets/raster-sample.png").unwrap();
    for along in [false, true] {
        let session = generalized_session(HalftoneChannelModel::Rgb, along);
        let scheduler = EvaluationScheduler::new().unwrap();
        let first = submit_and_accept(
            &scheduler,
            &session,
            EvaluationRequest::new(
                session.document_evaluation_snapshot(),
                ResolvedSource::new(
                    source_id.clone(),
                    source_bytes.clone(),
                    SourceFormatHint::Png,
                )
                .unwrap(),
            ),
        );
        let first_result = first.result().unwrap().clone();
        let repeated = submit_and_accept(
            &scheduler,
            &session,
            EvaluationRequest::new(
                session.document_evaluation_snapshot(),
                ResolvedSource::new(
                    source_id.clone(),
                    source_bytes.clone(),
                    SourceFormatHint::Png,
                )
                .unwrap(),
            ),
        );
        assert_eq!(
            repeated.cache_diagnostics().unwrap().aggregate.family,
            CacheDisposition::Hit
        );
        assert_eq!(
            repeated.cache_diagnostics().unwrap().aggregate.realization,
            CacheDisposition::Hit
        );
        assert_eq!(repeated.result().unwrap().scene(), first_result.scene());
        assert_eq!(repeated.result().unwrap().raster(), first_result.raster());
        assert_eq!(
            write_svg(repeated.result().unwrap().scene()),
            write_svg(first_result.scene())
        );
        scheduler.shutdown().unwrap();
    }
}

/// Proves engine preflight reserves a repeated along-guide envelope before complete realization.
#[test]
fn engine_preflights_multiplier_ten_along_guides_before_realization() {
    let source_id = SourceReferenceId::new("multiplier-ten-source").unwrap();
    let definition = PatternDefinition::generalized_straight_guides(
        PatternDefinitionId(601),
        "multiplier-ten along guides",
        PatternMechanismId(602),
        PatternMechanismId(603),
        PatternOutputLayerId(604),
        vec![StraightGuideDimension {
            id: GuideDimensionId(605),
            baseline_angle_degrees: 0.0,
            phase: 0.0,
            repetition: StraightGuideRepetition {
                spacing_multiplier: 10.0,
            },
        }],
        GeneralizedSiteProduct::AlongGuides {
            dimensions: vec![GuideDimensionId(605)],
            interval_multiplier: 10.0,
            phase: 0.0,
        },
        MarkOrientation::Fixed,
        CoveragePolicy {
            guard_steps: 1,
            additional_margin: 0.0,
        },
    );
    let document = Document::with_source(
        DocumentId(601),
        CanvasSpec {
            width: 100.0,
            height: 100.0,
        },
        SourceReference::Assigned(source_id.clone()),
        vec![definition],
        vec![ChannelState {
            id: ChannelId(601),
            pattern_definition_id: PatternDefinitionId(601),
            layout: ChannelPatternLayout {
                density: DensityMetric2D {
                    across_x: 10.0,
                    across_y: 10.0,
                    aspect_locked: true,
                },
                rotation_degrees: 0.0,
                translation_x: 0.0,
                translation_y: 0.0,
            },
            appearance: ChannelAppearance {
                visible: true,
                color: ColorValue {
                    red: 0.0,
                    green: 0.0,
                    blue: 0.0,
                    alpha: 1.0,
                },
                opacity: 1.0,
            },
            mark_geometry_response: MarkGeometryResponse {
                minimum_fill: 0.0,
                maximum_fill: 1.0,
                rotation_offset_degrees: 0.0,
            },
            source_mapping: ChannelSourceMapping {
                component: SourceComponent::Luminance,
                placement: SourcePlacement::StretchToCanvas,
            },
        }],
    )
    .unwrap();
    let session = DocumentSession::new(document).unwrap();
    let result = evaluate_channel_diagnostic(ChannelDiagnosticRequest::new(
        session.evaluation_snapshot(ChannelId(601)).unwrap(),
        ResolvedSource::new(
            source_id,
            std::fs::read("../../assets/raster-sample.png").unwrap(),
            SourceFormatHint::Png,
        )
        .unwrap(),
    ))
    .expect("engine preflight must cover multiplier-ten realized marks");
    let toniator_engine::GeometryOutput::CircularMarks(marks) =
        result.scene().layers()[0].geometry()
    else {
        panic!("the retained legacy diagnostic output must remain a circle adapter");
    };
    assert!(!marks.is_empty());
    assert!(marks.iter().all(|mark| mark.radius <= 550.0));
}

#[test]
fn generalized_history_and_scheduler_keep_authority_and_reject_stale_publication() {
    let source_id = SourceReferenceId::new("fixture-source").unwrap();
    let bytes = fs::read("../../assets/raster-sample.png").unwrap();
    let mut history = DocumentHistory::new(generalized_session_named(
        HalftoneChannelModel::Rgb,
        GeneralizedConfiguration::AlongGuide,
    ));
    let original = evaluate(EvaluationRequest::new(
        history.session().document_evaluation_snapshot(),
        ResolvedSource::new(source_id.clone(), bytes.clone(), SourceFormatHint::Png).unwrap(),
    ))
    .unwrap();
    let scheduler = EvaluationScheduler::new().unwrap();
    let ticket = scheduler
        .submit(EvaluationRequest::new(
            history.session().document_evaluation_snapshot(),
            ResolvedSource::new(source_id.clone(), bytes.clone(), SourceFormatHint::Png).unwrap(),
        ))
        .unwrap();
    let completion = wait_for_latest(&scheduler);
    assert_eq!(completion.ticket(), ticket);

    let base = history
        .session()
        .document()
        .pattern_definitions()
        .iter()
        .find(|definition| definition.id == PatternDefinitionId(1))
        .unwrap()
        .clone();
    history
        .apply(&DocumentCommand::EditSharedPatternDefinition {
            definition_id: PatternDefinitionId(1),
            base_definition: base,
            edit: PatternDefinitionEdit::SetCoverageGuardSteps { guard_steps: 3 },
        })
        .unwrap();
    assert!(
        !scheduler
            .accept_completion(&completion, history.session())
            .unwrap()
    );
    history.undo().unwrap();
    let restored = evaluate(EvaluationRequest::new(
        history.session().document_evaluation_snapshot(),
        ResolvedSource::new(source_id.clone(), bytes.clone(), SourceFormatHint::Png).unwrap(),
    ))
    .unwrap();
    assert_eq!(restored.channels(), original.channels());
    assert_eq!(restored.scene().identity(), original.scene().identity());
    assert_eq!(restored.raster().pixels(), original.raster().pixels());
    history.redo().unwrap();
    history.undo().unwrap();

    let accepted = submit_and_accept(
        &scheduler,
        history.session(),
        EvaluationRequest::new(
            history.session().document_evaluation_snapshot(),
            ResolvedSource::new(source_id, bytes, SourceFormatHint::Png).unwrap(),
        ),
    );
    assert!(accepted.result().is_some());
    scheduler.shutdown().unwrap();
}

#[test]
fn generalized_scheduler_supersession_and_failure_preserve_the_last_accepted_cache() {
    let source_id = SourceReferenceId::new("fixture-source").unwrap();
    let bytes = fs::read("../../assets/raster-sample.png").unwrap();
    for along in [false, true] {
        let session = generalized_session(HalftoneChannelModel::Rgb, along);
        let scheduler = EvaluationScheduler::new().unwrap();
        submit_and_accept(
            &scheduler,
            &session,
            EvaluationRequest::new(
                session.document_evaluation_snapshot(),
                ResolvedSource::new(source_id.clone(), bytes.clone(), SourceFormatHint::Png)
                    .unwrap(),
            ),
        );
        let failed_ticket = scheduler
            .submit(EvaluationRequest::new(
                session.document_evaluation_snapshot(),
                ResolvedSource::new(
                    SourceReferenceId::new("different-logical-source").unwrap(),
                    bytes.clone(),
                    SourceFormatHint::Png,
                )
                .unwrap(),
            ))
            .unwrap();
        let failed = wait_for_latest(&scheduler);
        assert_eq!(failed.ticket(), failed_ticket);
        assert_eq!(
            failed.error().unwrap().path(),
            "evaluation.source_reference"
        );
        assert!(scheduler.accept_completion(&failed, &session).unwrap());

        let superseded = scheduler
            .submit(EvaluationRequest::new(
                session.document_evaluation_snapshot(),
                ResolvedSource::new(source_id.clone(), bytes.clone(), SourceFormatHint::Png)
                    .unwrap(),
            ))
            .unwrap();
        let newest = scheduler
            .submit(EvaluationRequest::new(
                session.document_evaluation_snapshot(),
                ResolvedSource::new(source_id.clone(), bytes.clone(), SourceFormatHint::Png)
                    .unwrap(),
            ))
            .unwrap();
        assert!(!scheduler.is_latest(superseded));
        assert!(scheduler.is_latest(newest));
        let completion = wait_for_latest(&scheduler);
        assert_eq!(completion.ticket(), newest);
        assert!(scheduler.accept_completion(&completion, &session).unwrap());
        let diagnostics = completion.cache_diagnostics().unwrap();
        assert_eq!(diagnostics.aggregate.decoded_source, CacheDisposition::Hit);
        assert_eq!(diagnostics.aggregate.family, CacheDisposition::Hit);
        assert_eq!(diagnostics.aggregate.realization, CacheDisposition::Hit);
        assert_eq!(diagnostics.aggregate.scene, CacheDisposition::Hit);
        assert_eq!(diagnostics.aggregate.raster, CacheDisposition::Hit);
        scheduler.shutdown().unwrap();
    }
}

#[test]
fn generalized_cache_identity_matrix_misses_at_the_first_authoritative_layer() {
    let source_id = SourceReferenceId::new("fixture-source").unwrap();
    let raster = fs::read("../../assets/raster-sample.png").unwrap();
    let vector = fs::read("../../assets/vector-sample.svg").unwrap();
    let configuration = GeneralizedConfiguration::ThreeDirection;
    let session = generalized_session_named(HalftoneChannelModel::Rgb, configuration);
    let scheduler = EvaluationScheduler::new().unwrap();
    let baseline = submit_and_accept(
        &scheduler,
        &session,
        EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(source_id.clone(), raster.clone(), SourceFormatHint::Png).unwrap(),
        ),
    );
    let baseline_identity = baseline.result().unwrap().channels()[0]
        .realization_identity()
        .to_owned();
    let definition = session.document().pattern_definitions()[0].clone();

    let mut phase_definition = definition.clone();
    let PatternMechanism::StraightGuideDimensions { dimensions, .. } =
        &mut phase_definition.mechanisms[0]
    else {
        unreachable!()
    };
    dimensions[0].phase += 0.25;
    let phase_session = generalized_session_with_definition(
        HalftoneChannelModel::Rgb,
        configuration,
        phase_definition,
        SourceReference::Assigned(source_id.clone()),
        session.document().canvas().clone(),
    );
    let phase = submit_and_accept(
        &scheduler,
        &phase_session,
        EvaluationRequest::new(
            phase_session.document_evaluation_snapshot(),
            ResolvedSource::new(source_id.clone(), raster.clone(), SourceFormatHint::Png).unwrap(),
        ),
    );
    assert_eq!(
        phase.cache_diagnostics().unwrap().aggregate.family,
        CacheDisposition::Miss
    );

    // The accepted cache keeps one last-successful transaction. Restore the
    // unchanged structural key before asserting that an output-only edit is a
    // family hit and realization miss.
    submit_and_accept(
        &scheduler,
        &session,
        EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(source_id.clone(), raster.clone(), SourceFormatHint::Png).unwrap(),
        ),
    );

    let mut orientation_definition = definition.clone();
    let PatternOutputLayer::MarkPrototype { orientation, .. } =
        &mut orientation_definition.output_layers[0]
    else {
        unreachable!()
    };
    *orientation = MarkOrientation::Fixed;
    let orientation_session = generalized_session_with_definition(
        HalftoneChannelModel::Rgb,
        configuration,
        orientation_definition,
        SourceReference::Assigned(source_id.clone()),
        session.document().canvas().clone(),
    );
    let orientation = submit_and_accept(
        &scheduler,
        &orientation_session,
        EvaluationRequest::new(
            orientation_session.document_evaluation_snapshot(),
            ResolvedSource::new(source_id.clone(), raster.clone(), SourceFormatHint::Png).unwrap(),
        ),
    );
    let orientation_diagnostics = orientation.cache_diagnostics().unwrap();
    assert_eq!(
        orientation_diagnostics.aggregate.family,
        CacheDisposition::Hit
    );
    assert_eq!(
        orientation_diagnostics.aggregate.realization,
        CacheDisposition::Miss
    );
    assert_ne!(
        orientation.result().unwrap().channels()[0].realization_identity(),
        baseline_identity
    );

    submit_and_accept(
        &scheduler,
        &session,
        EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(source_id.clone(), raster.clone(), SourceFormatHint::Png).unwrap(),
        ),
    );

    let alternate_id = SourceReferenceId::new("same-content-other-reference").unwrap();
    let alternate_source_session = generalized_session_with_definition(
        HalftoneChannelModel::Rgb,
        configuration,
        definition.clone(),
        SourceReference::Assigned(alternate_id.clone()),
        session.document().canvas().clone(),
    );
    let alternate_source = submit_and_accept(
        &scheduler,
        &alternate_source_session,
        EvaluationRequest::new(
            alternate_source_session.document_evaluation_snapshot(),
            ResolvedSource::new(alternate_id, raster.clone(), SourceFormatHint::Png).unwrap(),
        ),
    );
    let alternate_diagnostics = alternate_source.cache_diagnostics().unwrap();
    assert_eq!(
        alternate_diagnostics.aggregate.decoded_source,
        CacheDisposition::Miss
    );
    assert_eq!(
        alternate_diagnostics.aggregate.family,
        CacheDisposition::Hit
    );
    assert_eq!(
        alternate_diagnostics.aggregate.realization,
        CacheDisposition::Hit
    );

    let decoded_changed = submit_and_accept(
        &scheduler,
        &session,
        EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(source_id.clone(), vector, SourceFormatHint::Svg).unwrap(),
        ),
    );
    let decoded_diagnostics = decoded_changed.cache_diagnostics().unwrap();
    assert_eq!(
        decoded_diagnostics.aggregate.decoded_source,
        CacheDisposition::Miss
    );
    assert_eq!(decoded_diagnostics.aggregate.family, CacheDisposition::Hit);
    assert_eq!(
        decoded_diagnostics.aggregate.realization,
        CacheDisposition::Miss
    );

    submit_and_accept(
        &scheduler,
        &session,
        EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(source_id.clone(), raster.clone(), SourceFormatHint::Png).unwrap(),
        ),
    );

    let mut presentation_session =
        generalized_session_named(HalftoneChannelModel::Rgb, configuration);
    presentation_session
        .apply(&DocumentCommand::SetOpacity {
            channel_id: ChannelId(1),
            opacity: 0.5,
        })
        .unwrap();
    let presentation = submit_and_accept(
        &scheduler,
        &presentation_session,
        EvaluationRequest::new(
            presentation_session.document_evaluation_snapshot(),
            ResolvedSource::new(source_id, raster, SourceFormatHint::Png).unwrap(),
        ),
    );
    let presentation_diagnostics = presentation.cache_diagnostics().unwrap();
    assert_eq!(
        presentation_diagnostics.aggregate.family,
        CacheDisposition::Hit
    );
    assert_eq!(
        presentation_diagnostics.aggregate.realization,
        CacheDisposition::Hit
    );
    assert_eq!(
        presentation_diagnostics.aggregate.scene,
        CacheDisposition::Miss
    );
    assert_eq!(
        presentation_diagnostics.aggregate.raster,
        CacheDisposition::Miss
    );
    scheduler.shutdown().unwrap();
}

#[test]
fn generalized_saved_v2_documents_reopen_with_identical_complete_outputs() {
    let validation =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/validation/stage-16a");
    fs::create_dir_all(&validation).unwrap();
    for configuration in [
        GeneralizedConfiguration::Orthogonal,
        GeneralizedConfiguration::Nonorthogonal,
        GeneralizedConfiguration::ThreeDirection,
        GeneralizedConfiguration::FourDirection,
        GeneralizedConfiguration::ParallelAlong,
        GeneralizedConfiguration::AlongGuide,
    ] {
        let product_label = configuration.label();
        for (source_label, source_format, source_path, embedded_format) in [
            (
                "raster",
                SourceFormatHint::Png,
                "../../assets/raster-sample.png",
                EmbeddedSourceFormat::Png,
            ),
            (
                "vector",
                SourceFormatHint::Svg,
                "../../assets/vector-sample.svg",
                EmbeddedSourceFormat::Svg,
            ),
        ] {
            let bytes = fs::read(source_path).unwrap();
            let source_id = SourceReferenceId::new("fixture-source").unwrap();
            let bundle = SourceBundle::new([EmbeddedSource::new(
                source_id.clone(),
                embedded_format,
                bytes.clone(),
                None,
            )
            .unwrap()])
            .unwrap();
            for model in [
                HalftoneChannelModel::Rgb,
                HalftoneChannelModel::Cmyk,
                HalftoneChannelModel::SourceColorAlpha,
            ] {
                let base = generalized_session_named(model, configuration);
                let original = evaluate(EvaluationRequest::new(
                    base.document_evaluation_snapshot(),
                    ResolvedSource::new(source_id.clone(), bytes.clone(), source_format).unwrap(),
                ))
                .unwrap();
                let path =
                    validation.join(format!("{product_label}-{source_label}-{model:?}.toniator"));
                save(&path, base.document(), &bundle).unwrap();
                let reopened = load(&path).unwrap();
                let reopened_session = DocumentSession::new(reopened.document().clone()).unwrap();
                let current = evaluate(EvaluationRequest::new(
                    reopened_session.document_evaluation_snapshot(),
                    ResolvedSource::new(source_id.clone(), bytes.clone(), source_format).unwrap(),
                ))
                .unwrap();
                assert_eq!(current.channels(), original.channels());
                assert_eq!(current.scene().identity(), original.scene().identity());
                assert_eq!(current.raster().pixels(), original.raster().pixels());
                assert_eq!(write_svg(current.scene()), write_svg(original.scene()));
                fs::write(
                    validation.join(format!("{product_label}-{source_label}-{model:?}.png")),
                    encode_png(current.raster()).unwrap(),
                )
                .unwrap();
                fs::write(
                    validation.join(format!("{product_label}-{source_label}-{model:?}.svg")),
                    write_svg(current.scene()),
                )
                .unwrap();
            }
        }
    }
}

fn random_definition(
    character: RandomSiteCharacter,
    modulation: SiteDensityModulation,
    exclusion: SiteExclusionPolicy,
    attempts: u32,
) -> PatternDefinition {
    PatternDefinition::random_sites(
        PatternDefinitionId(1),
        "site distributions",
        PatternMechanismId(101),
        PatternMechanismId(102),
        PatternMechanismId(103),
        PatternMechanismId(104),
        PatternOutputLayerId(105),
        character,
        0x1357_9bdf,
        modulation,
        exclusion,
        attempts,
        16_000_000,
        CoveragePolicy {
            guard_steps: 2,
            additional_margin: 4.5,
        },
    )
}

fn random_session(
    model: HalftoneChannelModel,
    width: f64,
    height: f64,
    definition: PatternDefinition,
) -> DocumentSession {
    let base = session(model);
    let mut channels = base
        .document()
        .channel_topology()
        .unwrap()
        .channels()
        .to_vec();
    for channel in &mut channels {
        channel.layout.density = DensityMetric2D {
            across_x: (width / 10.0).round(),
            across_y: (height / 10.0).round(),
            aspect_locked: true,
        };
    }
    let document = Document::with_source_and_topology(
        base.document().id(),
        CanvasSpec { width, height },
        base.document().source().clone(),
        vec![definition],
        model,
        ChannelTopology::new(channels),
    )
    .unwrap();
    DocumentSession::new(document).unwrap()
}

/// Verifies random current mark output remains visibly nonempty through native scene and SVG consumers.
fn assert_nonempty_random_native_output(result: &toniator_engine::EvaluationResult, label: &str) {
    let mut positive_marks = 0usize;
    for layer in result.scene().layers() {
        assert!(layer.visible(), "{label}: enabled channel was hidden");
        match layer.geometry() {
            toniator_engine::GeometryOutput::CircularMarks(marks) => {
                assert!(
                    !marks.is_empty(),
                    "{label}: enabled channel had no canonical marks"
                );
                assert!(
                    marks.iter().any(|mark| mark.radius > 0.0),
                    "{label}: enabled channel had no positive-radius canonical marks"
                );
                positive_marks += marks.iter().filter(|mark| mark.radius > 0.0).count();
            }
            toniator_engine::GeometryOutput::CanonicalMarks(marks) => {
                assert!(
                    !marks.is_empty(),
                    "{label}: enabled channel had no generalized canonical marks"
                );
                let circles = marks
                    .iter()
                    .filter_map(|mark| match mark {
                        toniator_engine::CanonicalMark::Circle { radius, .. } => Some(*radius),
                        toniator_engine::CanonicalMark::ClosedPath(_) => None,
                    })
                    .collect::<Vec<_>>();
                assert!(
                    circles.iter().any(|radius| *radius > 0.0),
                    "{label}: enabled channel had no positive-radius canonical marks"
                );
                positive_marks += circles.iter().filter(|radius| **radius > 0.0).count();
            }
        }
    }
    assert!(
        positive_marks > 0,
        "{label}: no positive-radius canonical marks"
    );
    let svg = write_svg(result.scene());
    assert!(
        svg.matches("<circle ").count() >= positive_marks,
        "{label}: SVG circles missing"
    );
    assert_eq!(
        svg.matches("<clipPath id=\"canvas-clip\"").count(),
        1,
        "{label}: SVG clip definition"
    );
    assert_eq!(
        svg.matches("clip-path=").count(),
        1,
        "{label}: SVG canvas clip use"
    );
    assert!(
        result
            .raster()
            .pixels()
            .chunks_exact(4)
            .any(|pixel| pixel[3] > 0),
        "{label}: native PNG raster alpha was entirely zero"
    );
}

#[test]
fn random_site_saved_v2_documents_reopen_with_native_png_svg_and_both_sources() {
    let validation =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/validation/stage-16b");
    fs::create_dir_all(&validation).unwrap();
    let configurations = [
        (
            "raw",
            RandomSiteCharacter::RawUniform,
            SiteDensityModulation::Uniform,
            SiteExclusionPolicy::None,
            32,
        ),
        (
            "even",
            RandomSiteCharacter::Even {
                minimum_center_distance: 8.0,
            },
            SiteDensityModulation::Uniform,
            SiteExclusionPolicy::None,
            32,
        ),
        (
            "clustered",
            RandomSiteCharacter::Clustered {
                cluster_density: 0.2,
                cluster_spread: 12.0,
                cluster_strength: 0.85,
            },
            SiteDensityModulation::Uniform,
            SiteExclusionPolicy::None,
            32,
        ),
        (
            "center-excluded",
            RandomSiteCharacter::RawUniform,
            SiteDensityModulation::Uniform,
            SiteExclusionPolicy::MinimumCenterDistance { minimum: 8.0 },
            32,
        ),
        (
            "visible-mark-excluded",
            RandomSiteCharacter::RawUniform,
            SiteDensityModulation::Uniform,
            SiteExclusionPolicy::VisibleMarkMargin {
                margin: 0.5,
                sizing: VisibleMarkSizingPolicy::MaximumSupportRadius,
            },
            32,
        ),
        (
            "unsatisfiable",
            RandomSiteCharacter::Even {
                minimum_center_distance: 120.0,
            },
            SiteDensityModulation::Uniform,
            SiteExclusionPolicy::None,
            1,
        ),
    ];
    for (label, character, modulation, exclusion, attempts) in configurations {
        let session = random_session(
            HalftoneChannelModel::Rgb,
            90.0,
            60.0,
            random_definition(character, modulation, exclusion, attempts),
        );
        let source_id = SourceReferenceId::new("fixture-source").unwrap();
        let bytes = fs::read("../../assets/raster-sample.png").unwrap();
        let result = evaluate(EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(source_id.clone(), bytes.clone(), SourceFormatHint::Png).unwrap(),
        ))
        .unwrap();
        if label != "unsatisfiable" {
            assert_nonempty_random_native_output(&result, label);
        }
        let bundle = SourceBundle::new([EmbeddedSource::new(
            source_id,
            EmbeddedSourceFormat::Png,
            bytes,
            None,
        )
        .unwrap()])
        .unwrap();
        let document_path = validation.join(format!("{label}-raster-Rgb.toniator"));
        save(&document_path, session.document(), &bundle).unwrap();
        let reopened = load(&document_path).unwrap();
        let reopened_session = DocumentSession::new(reopened.document().clone()).unwrap();
        let reopened_result = evaluate(EvaluationRequest::new(
            reopened_session.document_evaluation_snapshot(),
            ResolvedSource::new(
                SourceReferenceId::new("fixture-source").unwrap(),
                reopened
                    .sources()
                    .entries()
                    .next()
                    .unwrap()
                    .bytes()
                    .to_vec(),
                SourceFormatHint::Png,
            )
            .unwrap(),
        ))
        .unwrap();
        assert_eq!(result.channels(), reopened_result.channels());
        assert_eq!(result.raster().pixels(), reopened_result.raster().pixels());
        assert_eq!(
            write_svg(result.scene()),
            write_svg(reopened_result.scene())
        );
        fs::write(
            validation.join(format!("{label}-raster-Rgb.png")),
            encode_png(result.raster()).unwrap(),
        )
        .unwrap();
        fs::write(
            validation.join(format!("{label}-raster-Rgb.svg")),
            write_svg(result.scene()),
        )
        .unwrap();
    }
    for (source_label, source_format, source_path, embedded_format, width, height) in [
        (
            "raster",
            SourceFormatHint::Png,
            "../../assets/raster-sample.png",
            EmbeddedSourceFormat::Png,
            1024.0,
            1024.0,
        ),
        (
            "vector",
            SourceFormatHint::Svg,
            "../../assets/vector-sample.svg",
            EmbeddedSourceFormat::Svg,
            900.0,
            620.0,
        ),
    ] {
        let bytes = fs::read(source_path).unwrap();
        for model in [
            HalftoneChannelModel::Rgb,
            HalftoneChannelModel::Cmyk,
            HalftoneChannelModel::SourceColorAlpha,
        ] {
            let mapping = SourceMapping {
                component: SourceMappingComponent::Luminance,
                placement: SourcePlacement::StretchToCanvas,
                inverted: false,
                gain: 1.0,
                bias: 0.0,
            };
            let definition = random_definition(
                RandomSiteCharacter::RawUniform,
                SiteDensityModulation::ArtworkWeighted {
                    mapping,
                    strength: 0.85,
                    response: ArtworkWeightResponse::Linear,
                },
                SiteExclusionPolicy::MinimumCenterDistance { minimum: 8.0 },
                32,
            );
            let session = random_session(model, width, height, definition);
            let label = format!("artwork-weighted-{source_label}-{model:?}");
            let source_id = SourceReferenceId::new("fixture-source").unwrap();
            let bundle = SourceBundle::new([EmbeddedSource::new(
                source_id.clone(),
                embedded_format,
                bytes.clone(),
                None,
            )
            .unwrap()])
            .unwrap();
            let result = evaluate(EvaluationRequest::new(
                session.document_evaluation_snapshot(),
                ResolvedSource::new(source_id, bytes.clone(), source_format).unwrap(),
            ))
            .unwrap();
            assert_nonempty_random_native_output(&result, &label);
            save(
                &validation.join(format!("{label}.toniator")),
                session.document(),
                &bundle,
            )
            .unwrap();
            let reopened = load(&validation.join(format!("{label}.toniator"))).unwrap();
            let reopened_session = DocumentSession::new(reopened.document().clone()).unwrap();
            let reopened_result = evaluate(EvaluationRequest::new(
                reopened_session.document_evaluation_snapshot(),
                ResolvedSource::new(
                    SourceReferenceId::new("fixture-source").unwrap(),
                    reopened
                        .sources()
                        .entries()
                        .next()
                        .unwrap()
                        .bytes()
                        .to_vec(),
                    source_format,
                )
                .unwrap(),
            ))
            .unwrap();
            assert_eq!(result.channels(), reopened_result.channels());
            assert_eq!(
                result.scene().identity(),
                reopened_result.scene().identity()
            );
            assert_eq!(result.raster().pixels(), reopened_result.raster().pixels());
            assert_eq!(
                write_svg(result.scene()),
                write_svg(reopened_result.scene())
            );
            fs::write(
                validation.join(format!("{label}.png")),
                encode_png(result.raster()).unwrap(),
            )
            .unwrap();
            fs::write(
                validation.join(format!("{label}.svg")),
                write_svg(result.scene()),
            )
            .unwrap();
        }
        for (kind, character) in [
            ("raw", RandomSiteCharacter::RawUniform),
            (
                "even",
                RandomSiteCharacter::Even {
                    minimum_center_distance: 8.0,
                },
            ),
            (
                "clustered",
                RandomSiteCharacter::Clustered {
                    cluster_density: 0.001,
                    cluster_spread: 18.0,
                    cluster_strength: 1.0,
                },
            ),
        ] {
            let exclusion = if kind == "raw" {
                SiteExclusionPolicy::None
            } else {
                SiteExclusionPolicy::MinimumCenterDistance { minimum: 8.0 }
            };
            let session = random_session(
                HalftoneChannelModel::Rgb,
                width,
                height,
                random_definition(character, SiteDensityModulation::Uniform, exclusion, 32),
            );
            let source_id = SourceReferenceId::new("fixture-source").unwrap();
            let result = evaluate(EvaluationRequest::new(
                session.document_evaluation_snapshot(),
                ResolvedSource::new(source_id.clone(), bytes.clone(), source_format).unwrap(),
            ))
            .unwrap();
            let bundle = SourceBundle::new([EmbeddedSource::new(
                source_id,
                embedded_format,
                bytes.clone(),
                None,
            )
            .unwrap()])
            .unwrap();
            let label = format!("{kind}-{source_label}-Rgb-natural");
            save(
                &validation.join(format!("{label}.toniator")),
                session.document(),
                &bundle,
            )
            .unwrap();
            let reopened = load(&validation.join(format!("{label}.toniator"))).unwrap();
            let reopened_session = DocumentSession::new(reopened.document().clone()).unwrap();
            let reopened_result = evaluate(EvaluationRequest::new(
                reopened_session.document_evaluation_snapshot(),
                ResolvedSource::new(
                    SourceReferenceId::new("fixture-source").unwrap(),
                    reopened
                        .sources()
                        .entries()
                        .next()
                        .unwrap()
                        .bytes()
                        .to_vec(),
                    source_format,
                )
                .unwrap(),
            ))
            .unwrap();
            assert_eq!(result.channels(), reopened_result.channels(), "{label}");
            assert_eq!(
                result.raster().pixels(),
                reopened_result.raster().pixels(),
                "{label}"
            );
            assert_eq!(
                write_svg(result.scene()),
                write_svg(reopened_result.scene()),
                "{label}"
            );
            assert_nonempty_random_native_output(&result, &label);
            fs::write(
                validation.join(format!("{label}.png")),
                encode_png(result.raster()).unwrap(),
            )
            .unwrap();
            fs::write(
                validation.join(format!("{label}.svg")),
                write_svg(result.scene()),
            )
            .unwrap();
        }
    }
}

#[test]
fn random_family_cache_identity_and_scheduler_transactions_are_conditional_on_weighting() {
    let bytes = fs::read("../../assets/raster-sample.png").unwrap();
    let changed = fs::read("../../assets/vector-sample.svg").unwrap();
    for (weighted, expected_changed_family) in [
        (false, CacheDisposition::Hit),
        (true, CacheDisposition::Miss),
    ] {
        let modulation = if weighted {
            SiteDensityModulation::ArtworkWeighted {
                mapping: SourceMapping {
                    component: SourceMappingComponent::Luminance,
                    placement: SourcePlacement::StretchToCanvas,
                    inverted: false,
                    gain: 1.0,
                    bias: 0.0,
                },
                strength: 0.8,
                response: ArtworkWeightResponse::Smoothstep,
            }
        } else {
            SiteDensityModulation::Uniform
        };
        let mut session = random_session(
            HalftoneChannelModel::Rgb,
            90.0,
            60.0,
            random_definition(
                RandomSiteCharacter::Even {
                    minimum_center_distance: 6.0,
                },
                modulation,
                SiteExclusionPolicy::None,
                64,
            ),
        );
        let scheduler = EvaluationScheduler::new().unwrap();
        let first_id = SourceReferenceId::new("fixture-source").unwrap();
        submit_and_accept(
            &scheduler,
            &session,
            EvaluationRequest::new(
                session.document_evaluation_snapshot(),
                ResolvedSource::new(first_id.clone(), bytes.clone(), SourceFormatHint::Png)
                    .unwrap(),
            ),
        );
        session
            .apply(&DocumentCommand::SetSourceReference {
                source: SourceReference::Assigned(
                    SourceReferenceId::new("same-bytes-new-logical").unwrap(),
                ),
            })
            .unwrap();
        let same = submit_and_accept(
            &scheduler,
            &session,
            EvaluationRequest::new(
                session.document_evaluation_snapshot(),
                ResolvedSource::new(
                    SourceReferenceId::new("same-bytes-new-logical").unwrap(),
                    bytes.clone(),
                    SourceFormatHint::Png,
                )
                .unwrap(),
            ),
        );
        let same_diagnostics = same.cache_diagnostics().unwrap();
        assert_eq!(
            same_diagnostics.aggregate.decoded_source,
            CacheDisposition::Miss
        );
        assert_eq!(same_diagnostics.aggregate.family, CacheDisposition::Hit);
        session
            .apply(&DocumentCommand::SetSourceReference {
                source: SourceReference::Assigned(
                    SourceReferenceId::new("changed-source").unwrap(),
                ),
            })
            .unwrap();
        let changed_completion = submit_and_accept(
            &scheduler,
            &session,
            EvaluationRequest::new(
                session.document_evaluation_snapshot(),
                ResolvedSource::new(
                    SourceReferenceId::new("changed-source").unwrap(),
                    changed.clone(),
                    SourceFormatHint::Svg,
                )
                .unwrap(),
            ),
        );
        let diagnostics = changed_completion.cache_diagnostics().unwrap();
        assert_eq!(diagnostics.aggregate.decoded_source, CacheDisposition::Miss);
        assert_eq!(diagnostics.aggregate.family, expected_changed_family);
        assert_eq!(diagnostics.aggregate.realization, CacheDisposition::Miss);
        assert_eq!(diagnostics.aggregate.scene, CacheDisposition::Miss);
        assert_eq!(diagnostics.aggregate.raster, CacheDisposition::Miss);
        // Current failure must not disturb the five accepted slots; acceptance remains idempotent.
        let failed = scheduler
            .submit(EvaluationRequest::new(
                session.document_evaluation_snapshot(),
                ResolvedSource::new(
                    SourceReferenceId::new("wrong-id").unwrap(),
                    changed.clone(),
                    SourceFormatHint::Svg,
                )
                .unwrap(),
            ))
            .unwrap();
        let failed_completion = wait_for_latest(&scheduler);
        assert_eq!(failed_completion.ticket(), failed);
        assert!(failed_completion.error().is_some());
        assert!(
            scheduler
                .accept_completion(&failed_completion, &session)
                .unwrap()
        );
        assert!(
            !scheduler
                .accept_completion(&changed_completion, &session)
                .unwrap()
        );
        scheduler.shutdown().unwrap();
    }
}

/// Exercises the Stage 20A site interchange through complete-document cache,
/// history, and scheduler publication boundaries without changing their authority.
#[test]
fn stage20a_family_site_interchange_preserves_complete_document_cache_and_output_identity() {
    let source_id = SourceReferenceId::new("fixture-source").unwrap();
    let bytes = fs::read("../../assets/raster-sample.png").unwrap();
    let mut history = DocumentHistory::new(generalized_session_named(
        HalftoneChannelModel::Rgb,
        GeneralizedConfiguration::AlongGuide,
    ));
    let baseline = evaluate(EvaluationRequest::new(
        history.session().document_evaluation_snapshot(),
        ResolvedSource::new(source_id.clone(), bytes.clone(), SourceFormatHint::Png).unwrap(),
    ))
    .unwrap();
    let scheduler = EvaluationScheduler::new().unwrap();
    let first = submit_and_accept(
        &scheduler,
        history.session(),
        EvaluationRequest::new(
            history.session().document_evaluation_snapshot(),
            ResolvedSource::new(source_id.clone(), bytes.clone(), SourceFormatHint::Png).unwrap(),
        ),
    );
    assert_eq!(
        first.result().unwrap().scene().identity(),
        baseline.scene().identity()
    );
    assert_eq!(
        first.result().unwrap().raster().pixels(),
        baseline.raster().pixels()
    );
    let repeated = submit_and_accept(
        &scheduler,
        history.session(),
        EvaluationRequest::new(
            history.session().document_evaluation_snapshot(),
            ResolvedSource::new(source_id.clone(), bytes.clone(), SourceFormatHint::Png).unwrap(),
        ),
    );
    assert_eq!(
        repeated.cache_diagnostics().unwrap().aggregate,
        toniator_engine::CacheDiagnostics {
            decoded_source: CacheDisposition::Hit,
            family: CacheDisposition::Hit,
            realization: CacheDisposition::Hit,
            scene: CacheDisposition::Hit,
            raster: CacheDisposition::Hit,
        }
    );
    let base = history
        .document()
        .pattern_definitions()
        .iter()
        .find(|definition| definition.id == PatternDefinitionId(1))
        .unwrap()
        .clone();
    history
        .apply(&DocumentCommand::EditSharedPatternDefinition {
            definition_id: PatternDefinitionId(1),
            base_definition: base,
            edit: PatternDefinitionEdit::SetCoverageGuardSteps { guard_steps: 3 },
        })
        .unwrap();
    assert!(
        !scheduler
            .accept_completion(&repeated, history.session())
            .unwrap()
    );
    history.undo().unwrap();
    let restored = submit_and_accept(
        &scheduler,
        history.session(),
        EvaluationRequest::new(
            history.session().document_evaluation_snapshot(),
            ResolvedSource::new(source_id, bytes, SourceFormatHint::Png).unwrap(),
        ),
    );
    assert_eq!(
        restored.result().unwrap().scene().identity(),
        baseline.scene().identity()
    );
    assert_eq!(
        restored.result().unwrap().raster().pixels(),
        baseline.raster().pixels()
    );
    scheduler.shutdown().unwrap();
}

/// Locks both immutable natural inputs to accepted complete-document circle
/// identities while treating source SVG live text structurally, never as pixels.
#[test]
fn stage20a_natural_png_and_svg_inputs_preserve_current_circle_outputs() {
    let definition = random_definition(
        RandomSiteCharacter::Even {
            minimum_center_distance: 8.0,
        },
        SiteDensityModulation::Uniform,
        SiteExclusionPolicy::None,
        64,
    );
    let raster_bytes = fs::read("../../assets/raster-sample.png").unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(&raster_bytes)),
        "324ac232e319002a13fbcfac46538ca5d7e8ba8a127eea2eaf20e8ddb3ed2ef2"
    );
    let raster_session = random_session(
        HalftoneChannelModel::Rgb,
        1024.0,
        1024.0,
        definition.clone(),
    );
    let raster_result = evaluate(EvaluationRequest::new(
        raster_session.document_evaluation_snapshot(),
        ResolvedSource::new(
            SourceReferenceId::new("fixture-source").unwrap(),
            raster_bytes,
            SourceFormatHint::Png,
        )
        .unwrap(),
    ))
    .unwrap();
    assert_eq!(
        (
            raster_result.source_identity().width,
            raster_result.source_identity().height
        ),
        (1024, 1024)
    );
    let raster_svg = write_svg(raster_result.scene());
    let raster_identity = raster_result.scene().identity();
    assert_eq!(
        format!(
            "{}|{}|{}",
            raster_identity.family_fingerprint(),
            raster_identity.realization_fingerprint(),
            raster_identity.scene_fingerprint()
        ),
        "family:Rgb:Red:1:fnv1a64:362aedb40f0f839f:Green:2:fnv1a64:362aedb40f0f839f:Blue:3:fnv1a64:362aedb40f0f839f|realization:Rgb:Red:1:fnv1a64:1e0c7281d8b76bf0:Green:2:fnv1a64:bd90d174a377ce3b:Blue:3:fnv1a64:0c4f8869317e9c01|fnv1a64:88c5e8210e66dd16"
    );
    assert_eq!(
        format!("{:x}", Sha256::digest(raster_result.raster().pixels())),
        "37c334f2f1faefb23c1625411710e709741d310d0beb1f36183ff95b0eb1393e"
    );
    assert_eq!(
        format!("{:x}", Sha256::digest(raster_svg.as_bytes())),
        "6f28bf7d84d24cf7a9cbc43bf1f0c4a38982b736b4b9aae408f357921a97dd94"
    );

    let vector_bytes = fs::read("../../assets/vector-sample.svg").unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(&vector_bytes)),
        "42eb5e23111a5dbad66f2b1802a7cc06391c7ede829b99eb28aeb1ac91596e2e"
    );
    let text = std::str::from_utf8(&vector_bytes).unwrap();
    assert!(text.contains("<text"));
    assert!(text.contains(">T<"));
    let vector_session = random_session(HalftoneChannelModel::Rgb, 900.0, 620.0, definition);
    let evaluate_vector = || {
        evaluate(EvaluationRequest::new(
            vector_session.document_evaluation_snapshot(),
            ResolvedSource::new(
                SourceReferenceId::new("fixture-source").unwrap(),
                vector_bytes.clone(),
                SourceFormatHint::Svg,
            )
            .unwrap(),
        ))
        .unwrap()
    };
    let first_vector = evaluate_vector();
    let second_vector = evaluate_vector();
    let diagnostic = first_vector
        .source_identity()
        .svg_text
        .as_ref()
        .expect("SVG decode records live-text diagnostics");
    assert!(diagnostic.has_live_text_node);
    assert!(diagnostic.rendered_glyph_coverage);
    assert_eq!(
        diagnostic.font_policy,
        "system sans-serif fallback required"
    );
    assert_eq!(
        (
            first_vector.source_identity().width,
            first_vector.source_identity().height
        ),
        (900, 620)
    );
    assert_eq!(
        first_vector.scene().identity(),
        second_vector.scene().identity()
    );
    assert_eq!(
        first_vector.raster().pixels(),
        second_vector.raster().pixels()
    );
    let first_svg = write_svg(first_vector.scene());
    let second_svg = write_svg(second_vector.scene());
    assert_eq!(first_svg, second_svg);
    assert!(first_svg.contains("<circle "));
    assert!(first_svg.contains("<clipPath id=\"canvas-clip\""));
    assert_eq!(first_svg.matches("clip-path=").count(), 1);
    assert!(!first_svg.contains("<text"));
}

#[test]
fn scheduler_reuses_all_but_the_edited_mapping_realization() {
    let mut session = session(HalftoneChannelModel::Rgb);
    let source_id = SourceReferenceId::new("fixture-source").unwrap();
    let source_bytes = std::fs::read("../../assets/raster-sample.png").unwrap();
    let scheduler = EvaluationScheduler::new().unwrap();

    submit_and_accept(
        &scheduler,
        &session,
        EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(
                source_id.clone(),
                source_bytes.clone(),
                SourceFormatHint::Png,
            )
            .unwrap(),
        ),
    );
    session
        .apply(&DocumentCommand::SetModeledMappingField {
            channel_id: ChannelId(1),
            edit: toniator_domain::ModeledMappingFieldEdit::Inverted(true),
        })
        .unwrap();

    let completion = submit_and_accept(
        &scheduler,
        &session,
        EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(source_id, source_bytes, SourceFormatHint::Png).unwrap(),
        ),
    );
    let diagnostics = completion.cache_diagnostics().unwrap();
    assert_eq!(diagnostics.aggregate.decoded_source, CacheDisposition::Hit);
    assert_eq!(diagnostics.aggregate.family, CacheDisposition::Hit);
    assert_eq!(diagnostics.aggregate.realization, CacheDisposition::Miss);
    assert_eq!(diagnostics.aggregate.scene, CacheDisposition::Miss);
    assert_eq!(diagnostics.aggregate.raster, CacheDisposition::Miss);
    assert_eq!(diagnostics.channels.len(), 3);
    assert!(
        diagnostics
            .channels
            .iter()
            .all(|channel| channel.family == CacheDisposition::Hit)
    );
    assert_eq!(diagnostics.channels[0].channel_id, ChannelId(1));
    assert_eq!(diagnostics.channels[0].realization, CacheDisposition::Miss);
    assert!(
        diagnostics.channels[1..]
            .iter()
            .all(|channel| channel.realization == CacheDisposition::Hit)
    );

    scheduler.shutdown().unwrap();
}

#[test]
fn scheduler_reuses_derived_channels_for_a_presentation_only_edit() {
    let mut session = session(HalftoneChannelModel::Rgb);
    let source_id = SourceReferenceId::new("fixture-source").unwrap();
    let source_bytes = std::fs::read("../../assets/raster-sample.png").unwrap();
    let scheduler = EvaluationScheduler::new().unwrap();

    submit_and_accept(
        &scheduler,
        &session,
        EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(
                source_id.clone(),
                source_bytes.clone(),
                SourceFormatHint::Png,
            )
            .unwrap(),
        ),
    );
    session
        .apply(&DocumentCommand::SetColorComponent {
            channel_id: ChannelId(1),
            component: toniator_domain::ColorComponent::Red,
            value: 0.25,
        })
        .unwrap();
    let color_completion = submit_and_accept(
        &scheduler,
        &session,
        EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(
                source_id.clone(),
                source_bytes.clone(),
                SourceFormatHint::Png,
            )
            .unwrap(),
        ),
    );
    assert_presentation_reuse(&color_completion);
    session
        .apply(&DocumentCommand::SetOpacity {
            channel_id: ChannelId(3),
            opacity: 0.5,
        })
        .unwrap();
    let opacity_completion = submit_and_accept(
        &scheduler,
        &session,
        EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(
                source_id.clone(),
                source_bytes.clone(),
                SourceFormatHint::Png,
            )
            .unwrap(),
        ),
    );
    assert_presentation_reuse(&opacity_completion);
    session
        .apply(&DocumentCommand::SetVisibility {
            channel_id: ChannelId(2),
            visible: false,
        })
        .unwrap();

    let completion = submit_and_accept(
        &scheduler,
        &session,
        EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(source_id, source_bytes, SourceFormatHint::Png).unwrap(),
        ),
    );
    assert_presentation_reuse(&completion);
    let result = completion.result().unwrap();
    assert_eq!(result.channels().len(), 3);
    assert_eq!(result.scene().layers().len(), 3);
    assert_eq!(result.scene().layers()[1].channel_id(), ChannelId(2));
    assert!(!result.scene().layers()[1].visible());

    scheduler.shutdown().unwrap();
}

#[test]
fn scheduler_rebuilds_only_the_structurally_edited_channel() {
    let mut session = session(HalftoneChannelModel::Rgb);
    let source_id = SourceReferenceId::new("fixture-source").unwrap();
    let source_bytes = std::fs::read("../../assets/raster-sample.png").unwrap();
    let scheduler = EvaluationScheduler::new().unwrap();

    submit_and_accept(
        &scheduler,
        &session,
        EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(
                source_id.clone(),
                source_bytes.clone(),
                SourceFormatHint::Png,
            )
            .unwrap(),
        ),
    );
    session
        .apply(&DocumentCommand::SetTranslationAxis {
            channel_id: ChannelId(2),
            edited_axis: toniator_domain::TranslationEditedAxis::X,
            value: 1.0,
        })
        .unwrap();

    let completion = submit_and_accept(
        &scheduler,
        &session,
        EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(source_id, source_bytes, SourceFormatHint::Png).unwrap(),
        ),
    );
    let diagnostics = completion.cache_diagnostics().unwrap();
    assert_eq!(diagnostics.aggregate.decoded_source, CacheDisposition::Hit);
    assert_eq!(diagnostics.aggregate.family, CacheDisposition::Miss);
    assert_eq!(diagnostics.aggregate.realization, CacheDisposition::Miss);
    assert_eq!(diagnostics.aggregate.scene, CacheDisposition::Miss);
    assert_eq!(diagnostics.aggregate.raster, CacheDisposition::Miss);
    assert_eq!(diagnostics.channels.len(), 3);
    for (index, channel) in diagnostics.channels.iter().enumerate() {
        let expected = if index == 1 {
            CacheDisposition::Miss
        } else {
            CacheDisposition::Hit
        };
        assert_eq!(channel.family, expected);
        assert_eq!(channel.realization, expected);
    }

    scheduler.shutdown().unwrap();
}

#[test]
fn scheduler_reuses_structure_but_rebuilds_realizations_for_a_source_edit() {
    let mut session = session(HalftoneChannelModel::Rgb);
    let raster_source_id = SourceReferenceId::new("fixture-source").unwrap();
    let vector_source_id = SourceReferenceId::new("vector-source").unwrap();
    let scheduler = EvaluationScheduler::new().unwrap();

    let first = submit_and_accept(
        &scheduler,
        &session,
        EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(
                raster_source_id,
                std::fs::read("../../assets/raster-sample.png").unwrap(),
                SourceFormatHint::Png,
            )
            .unwrap(),
        ),
    );
    let decoded_before = first
        .result()
        .unwrap()
        .source_identity()
        .decoded_pixel_hash
        .clone();
    session
        .apply(&DocumentCommand::SetSourceReference {
            source: SourceReference::Assigned(vector_source_id.clone()),
        })
        .unwrap();

    let completion = submit_and_accept(
        &scheduler,
        &session,
        EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(
                vector_source_id,
                std::fs::read("../../assets/vector-sample.svg").unwrap(),
                SourceFormatHint::Svg,
            )
            .unwrap(),
        ),
    );
    let diagnostics = completion.cache_diagnostics().unwrap();
    assert_eq!(diagnostics.aggregate.decoded_source, CacheDisposition::Miss);
    assert_eq!(diagnostics.aggregate.family, CacheDisposition::Hit);
    assert_eq!(diagnostics.aggregate.realization, CacheDisposition::Miss);
    assert_eq!(diagnostics.aggregate.scene, CacheDisposition::Miss);
    assert_eq!(diagnostics.aggregate.raster, CacheDisposition::Miss);
    assert!(
        diagnostics
            .channels
            .iter()
            .all(|channel| channel.family == CacheDisposition::Hit)
    );
    assert!(
        diagnostics
            .channels
            .iter()
            .all(|channel| channel.realization == CacheDisposition::Miss)
    );
    assert_ne!(
        decoded_before,
        completion
            .result()
            .unwrap()
            .source_identity()
            .decoded_pixel_hash
    );

    scheduler.shutdown().unwrap();
}

#[test]
fn scheduler_reuses_channel_content_across_safe_topology_id_replacement() {
    let mut session = session(HalftoneChannelModel::Rgb);
    let source_id = SourceReferenceId::new("fixture-source").unwrap();
    let source_bytes = std::fs::read("../../assets/raster-sample.png").unwrap();
    let scheduler = EvaluationScheduler::new().unwrap();

    submit_and_accept(
        &scheduler,
        &session,
        EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(
                source_id.clone(),
                source_bytes.clone(),
                SourceFormatHint::Png,
            )
            .unwrap(),
        ),
    );
    let topology = ChannelTopology::new(
        session
            .document()
            .channel_topology()
            .unwrap()
            .channels()
            .iter()
            .cloned()
            .zip([ChannelId(101), ChannelId(102), ChannelId(103)])
            .map(|(mut channel, id)| {
                channel.id = id;
                channel
            })
            .collect(),
    );
    session
        .apply(&DocumentCommand::ReplaceChannelTopology {
            model: HalftoneChannelModel::Rgb,
            topology,
        })
        .unwrap();

    let completion = submit_and_accept(
        &scheduler,
        &session,
        EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(source_id, source_bytes, SourceFormatHint::Png).unwrap(),
        ),
    );
    let diagnostics = completion.cache_diagnostics().unwrap();
    assert_eq!(diagnostics.aggregate.decoded_source, CacheDisposition::Hit);
    assert_eq!(diagnostics.aggregate.family, CacheDisposition::Hit);
    assert_eq!(diagnostics.aggregate.realization, CacheDisposition::Hit);
    assert_eq!(diagnostics.aggregate.scene, CacheDisposition::Miss);
    assert_eq!(diagnostics.aggregate.raster, CacheDisposition::Miss);
    assert_eq!(
        diagnostics
            .channels
            .iter()
            .map(|channel| channel.channel_id)
            .collect::<Vec<_>>(),
        vec![ChannelId(101), ChannelId(102), ChannelId(103)]
    );
    assert!(diagnostics.channels.iter().all(|channel| {
        channel.family == CacheDisposition::Hit && channel.realization == CacheDisposition::Hit
    }));

    scheduler.shutdown().unwrap();
}

#[test]
fn scheduler_reuses_matching_channel_content_across_model_role_and_id_replacement() {
    let cmyk_template = session(HalftoneChannelModel::Cmyk)
        .document()
        .channel_topology()
        .unwrap()
        .clone();
    let mut session = session(HalftoneChannelModel::Rgb);
    let source_id = SourceReferenceId::new("fixture-source").unwrap();
    let source_bytes = std::fs::read("../../assets/raster-sample.png").unwrap();
    let scheduler = EvaluationScheduler::new().unwrap();

    submit_and_accept(
        &scheduler,
        &session,
        EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(
                source_id.clone(),
                source_bytes.clone(),
                SourceFormatHint::Png,
            )
            .unwrap(),
        ),
    );
    let topology = ChannelTopology::new(
        cmyk_template
            .channels()
            .iter()
            .cloned()
            .zip([
                ChannelId(201),
                ChannelId(202),
                ChannelId(203),
                ChannelId(204),
            ])
            .map(|(mut channel, id)| {
                channel.id = id;
                if channel.role == HalftoneChannelRole::Cyan {
                    channel.mapping = SourceMapping::canonical(SourceMappingComponent::Red);
                }
                channel
            })
            .collect(),
    );
    session
        .apply(&DocumentCommand::ReplaceChannelTopology {
            model: HalftoneChannelModel::Cmyk,
            topology,
        })
        .unwrap();

    let completion = submit_and_accept(
        &scheduler,
        &session,
        EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(source_id, source_bytes, SourceFormatHint::Png).unwrap(),
        ),
    );
    let diagnostics = completion.cache_diagnostics().unwrap();
    assert_eq!(diagnostics.aggregate.decoded_source, CacheDisposition::Hit);
    assert_eq!(diagnostics.aggregate.family, CacheDisposition::Hit);
    assert_eq!(diagnostics.aggregate.realization, CacheDisposition::Miss);
    assert_eq!(diagnostics.aggregate.scene, CacheDisposition::Miss);
    assert_eq!(diagnostics.aggregate.raster, CacheDisposition::Miss);
    assert!(
        diagnostics
            .channels
            .iter()
            .all(|channel| channel.family == CacheDisposition::Hit)
    );
    assert_eq!(diagnostics.channels[0].channel_id, ChannelId(201));
    assert_eq!(diagnostics.channels[0].realization, CacheDisposition::Hit);
    assert!(
        diagnostics.channels[1..]
            .iter()
            .all(|channel| channel.realization == CacheDisposition::Miss)
    );
    let result = completion.result().unwrap();
    assert_eq!(result.scene().model(), Some(HalftoneChannelModel::Cmyk));
    assert_eq!(
        result
            .channels()
            .iter()
            .map(|channel| channel.channel_id())
            .collect::<Vec<_>>(),
        vec![
            ChannelId(201),
            ChannelId(202),
            ChannelId(203),
            ChannelId(204)
        ]
    );

    scheduler.shutdown().unwrap();
}

#[test]
fn scheduler_accepts_current_failure_without_mutating_accepted_cache() {
    let session = session(HalftoneChannelModel::Rgb);
    let source_id = SourceReferenceId::new("fixture-source").unwrap();
    let source_bytes = std::fs::read("../../assets/raster-sample.png").unwrap();
    let scheduler = EvaluationScheduler::new().unwrap();

    submit_and_accept(
        &scheduler,
        &session,
        EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(
                source_id.clone(),
                source_bytes.clone(),
                SourceFormatHint::Png,
            )
            .unwrap(),
        ),
    );
    let ticket = scheduler
        .submit(EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(
                SourceReferenceId::new("mismatched-source").unwrap(),
                source_bytes.clone(),
                SourceFormatHint::Png,
            )
            .unwrap(),
        ))
        .unwrap();
    let failed = wait_for_latest(&scheduler);
    assert_eq!(failed.ticket(), ticket);
    assert_eq!(
        failed.error().unwrap().path(),
        "evaluation.source_reference"
    );
    assert!(scheduler.accept_completion(&failed, &session).unwrap());

    let repeated = submit_and_accept(
        &scheduler,
        &session,
        EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(source_id, source_bytes, SourceFormatHint::Png).unwrap(),
        ),
    );
    let diagnostics = repeated.cache_diagnostics().unwrap();
    assert_eq!(
        diagnostics.aggregate,
        toniator_engine::CacheDiagnostics {
            decoded_source: CacheDisposition::Hit,
            family: CacheDisposition::Hit,
            realization: CacheDisposition::Hit,
            scene: CacheDisposition::Hit,
            raster: CacheDisposition::Hit,
        }
    );
    assert!(diagnostics.channels.iter().all(|channel| {
        channel.family == CacheDisposition::Hit && channel.realization == CacheDisposition::Hit
    }));

    scheduler.shutdown().unwrap();
}

#[test]
fn scheduler_never_commits_partially_staged_channels_from_a_failed_document() {
    let mut session = session(HalftoneChannelModel::Rgb);
    let source_id = SourceReferenceId::new("fixture-source").unwrap();
    let source_bytes = std::fs::read("../../assets/raster-sample.png").unwrap();
    let limits = EvaluationLimits::new(10_000).unwrap();
    let scheduler = EvaluationScheduler::new_with_limits(limits).unwrap();

    submit_and_accept(
        &scheduler,
        &session,
        EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(
                source_id.clone(),
                source_bytes.clone(),
                SourceFormatHint::Png,
            )
            .unwrap(),
        ),
    );
    session
        .apply(&DocumentCommand::SetTranslationAxis {
            channel_id: ChannelId(1),
            edited_axis: toniator_domain::TranslationEditedAxis::X,
            value: 1.0,
        })
        .unwrap();
    session
        .apply(&DocumentCommand::SetDensityAxis {
            channel_id: ChannelId(2),
            edited_axis: toniator_domain::DensityEditedAxis::AcrossX,
            value: 10_000.0,
        })
        .unwrap();
    let failed_ticket = scheduler
        .submit(EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(
                source_id.clone(),
                source_bytes.clone(),
                SourceFormatHint::Png,
            )
            .unwrap(),
        ))
        .unwrap();
    let failed = wait_for_latest(&scheduler);
    assert_eq!(failed.ticket(), failed_ticket);
    assert_eq!(failed.error().unwrap().path(), "coverage.candidate_limit");
    assert!(scheduler.accept_completion(&failed, &session).unwrap());

    session
        .apply(&DocumentCommand::SetDensityAxis {
            channel_id: ChannelId(2),
            edited_axis: toniator_domain::DensityEditedAxis::AcrossX,
            value: 9.0,
        })
        .unwrap();
    let completion = submit_and_accept(
        &scheduler,
        &session,
        EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(source_id, source_bytes, SourceFormatHint::Png).unwrap(),
        ),
    );
    let diagnostics = completion.cache_diagnostics().unwrap();
    assert_eq!(diagnostics.aggregate.decoded_source, CacheDisposition::Hit);
    assert_eq!(diagnostics.aggregate.family, CacheDisposition::Miss);
    assert_eq!(diagnostics.aggregate.realization, CacheDisposition::Miss);
    assert_eq!(diagnostics.channels[0].channel_id, ChannelId(1));
    assert_eq!(diagnostics.channels[0].family, CacheDisposition::Miss);
    assert_eq!(diagnostics.channels[0].realization, CacheDisposition::Miss);
    assert!(diagnostics.channels[1..].iter().all(|channel| {
        channel.family == CacheDisposition::Hit && channel.realization == CacheDisposition::Hit
    }));

    scheduler.shutdown().unwrap();
}

#[test]
fn scheduler_never_commits_a_success_that_becomes_stale_before_acceptance() {
    let mut session = session(HalftoneChannelModel::Rgb);
    let source_id = SourceReferenceId::new("fixture-source").unwrap();
    let source_bytes = std::fs::read("../../assets/raster-sample.png").unwrap();
    let scheduler = EvaluationScheduler::new().unwrap();

    submit_and_accept(
        &scheduler,
        &session,
        EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(
                source_id.clone(),
                source_bytes.clone(),
                SourceFormatHint::Png,
            )
            .unwrap(),
        ),
    );
    session
        .apply(&DocumentCommand::SetModeledMappingField {
            channel_id: ChannelId(1),
            edit: toniator_domain::ModeledMappingFieldEdit::Inverted(true),
        })
        .unwrap();
    let stale_ticket = scheduler
        .submit(EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(
                source_id.clone(),
                source_bytes.clone(),
                SourceFormatHint::Png,
            )
            .unwrap(),
        ))
        .unwrap();
    let stale_completion = wait_for_latest(&scheduler);
    assert_eq!(stale_completion.ticket(), stale_ticket);
    assert!(stale_completion.result().is_some());
    session
        .apply(&DocumentCommand::SetVisibility {
            channel_id: ChannelId(3),
            visible: false,
        })
        .unwrap();
    assert!(
        !scheduler
            .accept_completion(&stale_completion, &session)
            .unwrap()
    );

    let current = submit_and_accept(
        &scheduler,
        &session,
        EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(source_id, source_bytes, SourceFormatHint::Png).unwrap(),
        ),
    );
    let diagnostics = current.cache_diagnostics().unwrap();
    assert_eq!(diagnostics.aggregate.decoded_source, CacheDisposition::Hit);
    assert_eq!(diagnostics.aggregate.family, CacheDisposition::Hit);
    assert_eq!(diagnostics.aggregate.realization, CacheDisposition::Miss);
    assert_eq!(diagnostics.channels[0].realization, CacheDisposition::Miss);
    assert!(
        diagnostics.channels[1..]
            .iter()
            .all(|channel| channel.realization == CacheDisposition::Hit)
    );
    assert!(scheduler.accept_completion(&current, &session).unwrap());

    scheduler.shutdown().unwrap();
}

#[test]
fn scheduler_coalesces_rapid_document_submissions_to_the_newest_ticket() {
    let mut session = session(HalftoneChannelModel::Rgb);
    let source_id = SourceReferenceId::new("fixture-source").unwrap();
    let source_bytes = std::fs::read("../../assets/raster-sample.png").unwrap();
    let scheduler = EvaluationScheduler::new().unwrap();

    let first_ticket = scheduler
        .submit(EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(
                source_id.clone(),
                source_bytes.clone(),
                SourceFormatHint::Png,
            )
            .unwrap(),
        ))
        .unwrap();
    let stale_completion = wait_for_latest(&scheduler);
    assert_eq!(stale_completion.ticket(), first_ticket);

    session
        .apply(&DocumentCommand::SetModeledMappingField {
            channel_id: ChannelId(1),
            edit: toniator_domain::ModeledMappingFieldEdit::Inverted(true),
        })
        .unwrap();
    let second_ticket = scheduler
        .submit(EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(
                source_id.clone(),
                source_bytes.clone(),
                SourceFormatHint::Png,
            )
            .unwrap(),
        ))
        .unwrap();
    session
        .apply(&DocumentCommand::SetVisibility {
            channel_id: ChannelId(3),
            visible: false,
        })
        .unwrap();
    let newest_ticket = scheduler
        .submit(EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(
                source_id.clone(),
                source_bytes.clone(),
                SourceFormatHint::Png,
            )
            .unwrap(),
        ))
        .unwrap();
    assert!(!scheduler.is_latest(first_ticket));
    assert!(!scheduler.is_latest(second_ticket));
    assert!(scheduler.is_latest(newest_ticket));
    assert!(
        !scheduler
            .accept_completion(&stale_completion, &session)
            .unwrap()
    );

    let newest = wait_for_latest(&scheduler);
    assert_eq!(newest.ticket(), newest_ticket);
    assert_eq!(newest.token(), session.document_evaluation_token());
    assert!(scheduler.accept_completion(&newest, &session).unwrap());

    let repeated = submit_and_accept(
        &scheduler,
        &session,
        EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(source_id, source_bytes, SourceFormatHint::Png).unwrap(),
        ),
    );
    let diagnostics = repeated.cache_diagnostics().unwrap();
    assert_eq!(
        diagnostics.aggregate,
        toniator_engine::CacheDiagnostics {
            decoded_source: CacheDisposition::Hit,
            family: CacheDisposition::Hit,
            realization: CacheDisposition::Hit,
            scene: CacheDisposition::Hit,
            raster: CacheDisposition::Hit,
        }
    );
    assert!(diagnostics.channels.iter().all(|channel| {
        channel.family == CacheDisposition::Hit && channel.realization == CacheDisposition::Hit
    }));

    scheduler.shutdown().unwrap();
}
