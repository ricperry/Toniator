use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};

use toniator_domain::{
    CanvasSpec, ChannelAppearance, ChannelId, ChannelPatternInstance, ChannelPatternLayoutDelta,
    ChannelSourceMapping, ChannelState, ColorValue, DensityEditedField, DensityMetric2D, Document,
    DocumentCommand, DocumentHistory, DocumentId, DocumentSession, PatternGeometryResponse,
    SourceComponent, SourcePlacement, SourceReference, SourceReferenceId,
};
use toniator_engine::{
    CacheDisposition, ChannelDiagnosticRequest, EvaluationCompletion, EvaluationRequest,
    EvaluationScheduler, ResolvedSource, SourceFormatHint, evaluate_channel_diagnostic,
};

/// Loads the immutable raster source used by bounded engine-cache witnesses.
fn source_bytes() -> Vec<u8> {
    fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/raster-sample.png"))
        .expect("immutable raster source is present")
}

/// Builds the current modeled document with an assigned source and history authority.
fn history() -> DocumentHistory {
    let document = Document::new_default_document(
        CanvasSpec {
            width: 90.0,
            height: 60.0,
        },
        SourceReference::Assigned(SourceReferenceId::new("stage20g-source").expect("valid ID")),
    )
    .expect("default modeled document is valid");
    DocumentHistory::new(DocumentSession::new(document).expect("valid session"))
}

/// Builds a complete evaluation request from the current history revision.
fn request(history: &DocumentHistory) -> EvaluationRequest {
    EvaluationRequest::new(
        history.session().document_evaluation_snapshot(),
        ResolvedSource::new(
            SourceReferenceId::new("stage20g-source").expect("valid ID"),
            source_bytes(),
            SourceFormatHint::Png,
        )
        .expect("source resolves"),
    )
}

/// Waits for the latest scheduler completion without assuming thread timing.
fn wait_for_latest(scheduler: &EvaluationScheduler) -> EvaluationCompletion {
    let deadline = Instant::now() + Duration::from_secs(15);
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

/// Submits one current request and publishes its cache transaction.
fn submit_and_accept(
    scheduler: &EvaluationScheduler,
    history: &DocumentHistory,
) -> EvaluationCompletion {
    let ticket = scheduler.submit(request(history)).expect("submit succeeds");
    let completion = wait_for_latest(scheduler);
    assert_eq!(completion.ticket(), ticket);
    assert!(
        scheduler
            .accept_completion(&completion, history.session())
            .expect("accept check succeeds")
    );
    completion
}

/// Proves a document density edit misses every family while a selected-channel edit misses only one.
#[test]
fn document_and_selected_density_edits_have_distinct_family_cache_scope() {
    let scheduler = EvaluationScheduler::new().expect("scheduler starts");
    let mut history = history();
    submit_and_accept(&scheduler, &history);

    let base_edit = history
        .document()
        .set_document_density_field(DensityEditedField::Density, 12.0)
        .expect("base density command builds");
    history.apply(&base_edit).expect("base edit applies");
    let completion = submit_and_accept(&scheduler, &history);
    let diagnostics = completion.cache_diagnostics().expect("cache diagnostics");
    assert!(
        diagnostics
            .channels
            .iter()
            .all(|channel| channel.family == CacheDisposition::Miss)
    );

    let current = history
        .document()
        .effective_channel_pattern(ChannelId(2))
        .expect("channel resolves")
        .density;
    let selected_edit = history
        .document()
        .set_channel_density_for_effective(
            ChannelId(2),
            DensityMetric2D {
                density: current.density + 1.0,
                aspect: current.aspect,
            },
        )
        .expect("selected density command builds");
    history
        .apply(&selected_edit)
        .expect("selected edit applies");
    let completion = submit_and_accept(&scheduler, &history);
    let diagnostics = completion.cache_diagnostics().expect("cache diagnostics");
    assert_eq!(diagnostics.channels[0].family, CacheDisposition::Hit);
    assert_eq!(diagnostics.channels[1].family, CacheDisposition::Miss);
    assert_eq!(diagnostics.channels[2].family, CacheDisposition::Hit);
    scheduler.shutdown().expect("scheduler stops");
}

/// Proves shape rotation and mark response bypass family rebuilds and invalidate only realization.
#[test]
fn shape_and_fill_changes_are_selected_channel_realization_only() {
    let scheduler = EvaluationScheduler::new().expect("scheduler starts");
    let mut history = history();
    submit_and_accept(&scheduler, &history);

    let shape = history
        .document()
        .set_channel_shape_rotation_for_effective(ChannelId(1), 17.0)
        .expect("shape command builds");
    history.apply(&shape).expect("shape edit applies");
    let completion = submit_and_accept(&scheduler, &history);
    let diagnostics = completion.cache_diagnostics().expect("cache diagnostics");
    assert!(
        diagnostics
            .channels
            .iter()
            .all(|channel| channel.family == CacheDisposition::Hit)
    );
    assert_eq!(diagnostics.channels[0].realization, CacheDisposition::Miss);
    assert!(
        diagnostics.channels[1..]
            .iter()
            .all(|channel| channel.realization == CacheDisposition::Hit)
    );

    let output_layer_id =
        history.document().pattern_definition_bundles()[0].output_settings[0].output_layer_id;
    let fill = history
        .document()
        .set_channel_output_response_for_effective(
            ChannelId(2),
            output_layer_id,
            PatternGeometryResponse::Marks(toniator_domain::MarkGeometryResponse {
                minimum_fill: 0.1,
                maximum_fill: 1.2,
            }),
        )
        .expect("fill command builds");
    history.apply(&fill).expect("fill edit applies");
    let completion = submit_and_accept(&scheduler, &history);
    let diagnostics = completion.cache_diagnostics().expect("cache diagnostics");
    assert!(
        diagnostics
            .channels
            .iter()
            .all(|channel| channel.family == CacheDisposition::Hit)
    );
    assert_eq!(diagnostics.channels[1].realization, CacheDisposition::Miss);
    scheduler.shutdown().expect("scheduler stops");
}

/// Proves equal-output authority and reset reuse effective content rather than persisted intent shape.
#[test]
fn authority_only_delta_and_reset_reuse_effective_cache_content() {
    let scheduler = EvaluationScheduler::new().expect("scheduler starts");
    let mut history = history();
    submit_and_accept(&scheduler, &history);
    let explicit_zero = history
        .document()
        .set_channel_pattern_rotation_for_effective(ChannelId(1), 0.0)
        .expect("zero delta builds");
    history
        .apply(&explicit_zero)
        .expect("authority-only edit applies");
    let completion = submit_and_accept(&scheduler, &history);
    let diagnostics = completion.cache_diagnostics().expect("cache diagnostics");
    assert!(diagnostics.channels.iter().all(|channel| {
        channel.family == CacheDisposition::Hit && channel.realization == CacheDisposition::Hit
    }));

    let changed = history
        .document()
        .set_channel_pattern_rotation_for_effective(ChannelId(1), 11.0)
        .expect("rotation command builds");
    history.apply(&changed).expect("rotation edit applies");
    submit_and_accept(&scheduler, &history);
    let reset = DocumentCommand::ResetChannelPatternRotationDelta {
        base: history.document().pattern_settings().clone(),
        channel_id: ChannelId(1),
    };
    history.apply(&reset).expect("reset applies");
    let completion = submit_and_accept(&scheduler, &history);
    let diagnostics = completion.cache_diagnostics().expect("cache diagnostics");
    assert_eq!(diagnostics.channels[0].family, CacheDisposition::Hit);
    scheduler.shutdown().expect("scheduler stops");
}

/// Proves stale results cannot publish and both retained channel configurations consume the resolver.
#[test]
fn stale_publication_rejects_and_legacy_diagnostic_uses_effective_authority() {
    let scheduler = EvaluationScheduler::new().expect("scheduler starts");
    let mut history = history();
    let ticket = scheduler
        .submit(request(&history))
        .expect("submit succeeds");
    let completion = wait_for_latest(&scheduler);
    assert_eq!(completion.ticket(), ticket);
    let edit = history
        .document()
        .set_channel_shape_rotation_for_effective(ChannelId(1), 9.0)
        .expect("shape command builds");
    history.apply(&edit).expect("revision advances");
    assert!(
        !scheduler
            .accept_completion(&completion, history.session())
            .expect("stale acceptance check succeeds")
    );
    submit_and_accept(&scheduler, &history);

    let modeled = history.document();
    let legacy = Document::with_source(
        DocumentId(20),
        modeled.canvas().clone(),
        modeled.source().clone(),
        vec![modeled.pattern_definition_bundles()[0].clone()],
        modeled.pattern_settings().clone(),
        vec![ChannelState {
            id: ChannelId(71),
            pattern_instance: ChannelPatternInstance {
                definition_override: None,
                layout_delta: ChannelPatternLayoutDelta {
                    density: None,
                    rotation_degrees: Some(5.0),
                    translation_x: 0.0,
                    translation_y: 0.0,
                },
                shape_rotation_delta_degrees: Some(3.0),
                output_response_deltas: Vec::new(),
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
            source_mapping: ChannelSourceMapping {
                component: SourceComponent::Luminance,
                placement: SourcePlacement::StretchToCanvas,
            },
        }],
    )
    .expect("legacy document is valid");
    let session = DocumentSession::new(legacy).expect("legacy session is valid");
    let diagnostic = ChannelDiagnosticRequest::new(
        session
            .evaluation_snapshot(ChannelId(71))
            .expect("legacy snapshot"),
        ResolvedSource::new(
            SourceReferenceId::new("stage20g-source").expect("valid ID"),
            source_bytes(),
            SourceFormatHint::Png,
        )
        .expect("source resolves"),
    );
    evaluate_channel_diagnostic(diagnostic).expect("legacy diagnostic evaluates");
    scheduler.shutdown().expect("scheduler stops");
}
