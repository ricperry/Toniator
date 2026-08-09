use std::time::{Duration, Instant};
use toniator_domain::{
    CanvasSpec, ChannelAppearance, ChannelId, ChannelPatternLayout, ChannelSourceMapping,
    ChannelState, ChannelTopology, ChannelTopologyTemplate, ColorValue, DensityMetric2D, Document,
    DocumentCommand, DocumentId, DocumentSession, HalftoneChannelModel, HalftoneChannelRole,
    MarkGeometryResponse, PatternDefinition, PatternDefinitionId, PatternOutput, PatternStructure,
    SourceComponent, SourceMapping, SourceMappingComponent, SourcePlacement, SourceReference,
    SourceReferenceId,
};
use toniator_engine::{
    CacheDisposition, EvaluationCompletion, EvaluationLimits, EvaluationRequest,
    EvaluationScheduler, PreviewRasterTarget, ResolvedSource, SourceFormatHint, evaluate,
    evaluate_with_limits, write_svg,
};

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
    let definition = PatternDefinition {
        id: PatternDefinitionId(1),
        name: "grid".into(),
        structure: PatternStructure::StraightGrid,
        output: PatternOutput::CircularMarks,
        guard_steps: 2,
        maximum_support_radius: 4.5,
    };
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
