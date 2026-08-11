use sha2::{Digest, Sha256};
use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};
use toniator_domain::{
    ArtworkWeightResponse, CanvasSpec, ChannelAppearance, ChannelId, ChannelPatternLayout,
    ChannelSourceMapping, ChannelState, ChannelTopology, ChannelTopologyTemplate, ColorValue,
    CoveragePolicy, DensityMetric2D, Document, DocumentCommand, DocumentHistory, DocumentId,
    DocumentSession, GeneralizedSiteProduct, GuideDimensionId, HalftoneChannelModel,
    HalftoneChannelRole, MarkGeometryResponse, MarkOrientation, PatternDefinition,
    PatternDefinitionEdit, PatternDefinitionId, PatternMechanism, PatternMechanismId,
    PatternOutputLayer, PatternOutputLayerId, RandomSiteCharacter, SiteDensityModulation,
    SiteExclusionPolicy, SourceComponent, SourceMapping, SourceMappingComponent, SourcePlacement,
    SourceReference, SourceReferenceId, StraightGuideDimension, StraightGuideRepetition,
    VisibleMarkSizingPolicy,
};
use toniator_engine::{
    CacheDisposition, EvaluationCompletion, EvaluationLimits, EvaluationRequest,
    EvaluationScheduler, PreviewRasterTarget, ResolvedSource, SourceFormatHint, encode_png,
    evaluate, evaluate_with_limits, write_svg,
};
use toniator_io::{EmbeddedSource, EmbeddedSourceFormat, SourceBundle, load, save};

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
            maximum_support_radius: 4.5,
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
            minimum_size: 2.0,
            maximum_size: 9.0,
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
            maximum_support_radius: 4.5,
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
            minimum_size: 2.0,
            maximum_size: 9.0,
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
            edit: PatternDefinitionEdit::SetCoverage {
                coverage: CoveragePolicy {
                    guard_steps: 3,
                    maximum_support_radius: 4.5,
                },
            },
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
            maximum_support_radius: 4.5,
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

fn assert_nonempty_random_native_output(result: &toniator_engine::EvaluationResult, label: &str) {
    let mut positive_marks = 0usize;
    for layer in result.scene().layers() {
        assert!(layer.visible(), "{label}: enabled channel was hidden");
        let toniator_engine::GeometryOutput::CircularMarks(marks) = layer.geometry();
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
        .apply(&DocumentCommand::SetTopologySourceMapping {
            channel_id: ChannelId(1),
            mapping: SourceMapping {
                component: SourceMappingComponent::Red,
                placement: SourcePlacement::StretchToCanvas,
                inverted: true,
                gain: 1.0,
                bias: 0.0,
            },
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
        .apply(&DocumentCommand::SetColor {
            channel_id: ChannelId(1),
            color: ColorValue {
                red: 0.25,
                green: 0.5,
                blue: 0.75,
                alpha: 1.0,
            },
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
        .apply(&DocumentCommand::SetTranslation {
            channel_id: ChannelId(2),
            translation_x: 1.0,
            translation_y: 0.0,
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
        .apply(&DocumentCommand::SetTranslation {
            channel_id: ChannelId(1),
            translation_x: 1.0,
            translation_y: 0.0,
        })
        .unwrap();
    session
        .apply(&DocumentCommand::SetDensity {
            channel_id: ChannelId(2),
            density: DensityMetric2D {
                across_x: 10_000.0,
                across_y: 10_000.0,
                aspect_locked: true,
            },
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
        .apply(&DocumentCommand::SetDensity {
            channel_id: ChannelId(2),
            density: DensityMetric2D {
                across_x: 9.0,
                across_y: 6.0,
                aspect_locked: true,
            },
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
        .apply(&DocumentCommand::SetTopologySourceMapping {
            channel_id: ChannelId(1),
            mapping: SourceMapping {
                component: SourceMappingComponent::Red,
                placement: SourcePlacement::StretchToCanvas,
                inverted: true,
                gain: 1.0,
                bias: 0.0,
            },
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
        .apply(&DocumentCommand::SetTopologySourceMapping {
            channel_id: ChannelId(1),
            mapping: SourceMapping {
                component: SourceMappingComponent::Red,
                placement: SourcePlacement::StretchToCanvas,
                inverted: true,
                gain: 1.0,
                bias: 0.0,
            },
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
