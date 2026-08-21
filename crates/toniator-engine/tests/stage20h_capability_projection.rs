use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};

use toniator_domain::{
    CanvasSpec, ChannelId, DensityModulationKind, DispersionCapabilityProjection, Document,
    DocumentHistory, DocumentSession, ExclusionKind, GeneratorCapabilities, MarkOrientationKind,
    MarkOutputCapabilityProjection, MarkPrototypeKind, PatternCapabilityProjection,
    PatternCapabilityScope, PatternFamilyCapabilityProjection, PatternOutputCapabilityProjection,
    RandomCharacterKind, SourceReference, SourceReferenceId,
};
use toniator_engine::{
    CacheDisposition, EvaluationCompletion, EvaluationRequest, EvaluationScheduler, ResolvedSource,
    SourceFormatHint,
};

/// Builds a small assigned-source history for cache-inert capability-query witnesses.
fn history(source_id: &str) -> DocumentHistory {
    let document = Document::new_default_document(
        CanvasSpec {
            width: 90.0,
            height: 60.0,
        },
        SourceReference::Assigned(SourceReferenceId::new(source_id).expect("valid ID")),
    )
    .expect("document validates");
    DocumentHistory::new(DocumentSession::new(document).expect("session validates"))
}

/// Builds one authoritative evaluation request from the current immutable document snapshot.
fn request(
    history: &DocumentHistory,
    source_id: &str,
    fixture_name: &str,
    format: SourceFormatHint,
) -> EvaluationRequest {
    EvaluationRequest::new(
        history.session().document_evaluation_snapshot(),
        ResolvedSource::new(
            SourceReferenceId::new(source_id).expect("valid ID"),
            fs::read(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../assets")
                    .join(fixture_name),
            )
            .expect("immutable source reads"),
            format,
        )
        .expect("resolved source validates"),
    )
}

/// Waits for the scheduler's latest completion without making timing an authority.
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

/// Publishes one current authoritative evaluation so its accepted cache transaction can be reused.
fn submit_and_accept(
    scheduler: &EvaluationScheduler,
    history: &DocumentHistory,
    source_id: &str,
    fixture_name: &str,
    format: SourceFormatHint,
) -> EvaluationCompletion {
    let ticket = scheduler
        .submit(request(history, source_id, fixture_name, format))
        .expect("scheduler submits");
    let completion = wait_for_latest(scheduler);
    assert_eq!(completion.ticket(), ticket);
    assert!(
        scheduler
            .accept_completion(&completion, history.session())
            .expect("acceptance check succeeds")
    );
    completion
}

/// Proves projection queries neither evaluate nor change authoritative cache identity or reuse.
#[test]
fn capability_queries_are_cache_inert_before_and_after_authoritative_evaluation() {
    for (source_id, fixture_name, format) in [
        (
            "stage20h-raster",
            "raster-sample.png",
            SourceFormatHint::Png,
        ),
        (
            "stage20h-vector",
            "vector-sample.svg",
            SourceFormatHint::Svg,
        ),
    ] {
        let history = history(source_id);
        let before = history
            .document()
            .pattern_capabilities(PatternCapabilityScope::Channel(ChannelId(1)))
            .expect("projection resolves before evaluation");
        let scheduler = EvaluationScheduler::new().expect("scheduler starts");
        let first = submit_and_accept(&scheduler, &history, source_id, fixture_name, format);
        let after = history
            .document()
            .pattern_capabilities(PatternCapabilityScope::Channel(ChannelId(1)))
            .expect("projection resolves after evaluation");
        assert_eq!(before, after);
        let second = submit_and_accept(&scheduler, &history, source_id, fixture_name, format);
        assert_eq!(first.result(), second.result());
        let diagnostics = second
            .cache_diagnostics()
            .expect("cache diagnostics present");
        assert!(diagnostics.channels.iter().all(|channel| {
            channel.family == CacheDisposition::Hit && channel.realization == CacheDisposition::Hit
        }));
        assert_eq!(diagnostics.aggregate.decoded_source, CacheDisposition::Hit);
        assert_eq!(diagnostics.aggregate.family, CacheDisposition::Hit);
        assert_eq!(diagnostics.aggregate.realization, CacheDisposition::Hit);
        assert_eq!(diagnostics.aggregate.scene, CacheDisposition::Hit);
        assert_eq!(diagnostics.aggregate.raster, CacheDisposition::Hit);
        scheduler.shutdown().expect("scheduler stops");
    }
}

/// Proves the current Holiday v4 fixture resolves divergent effective definitions without a cache or UI path.
#[test]
fn holiday_v4_channels_project_independently_from_their_effective_definitions() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/HolidayMugs_2024_2025.toniator");
    let loaded = toniator_io::load(&path).expect("Holiday v4 fixture opens");
    let projections = [ChannelId(1), ChannelId(2), ChannelId(3)].map(|channel_id| {
        loaded
            .document()
            .pattern_capabilities(PatternCapabilityScope::Channel(channel_id))
            .expect("Holiday effective channel projects")
    });
    let effective_rotations = [ChannelId(1), ChannelId(2), ChannelId(3)].map(|channel_id| {
        loaded
            .document()
            .effective_channel_pattern(channel_id)
            .expect("Holiday effective channel resolves")
            .pattern_rotation_degrees
    });
    assert_eq!(
        projections
            .iter()
            .map(|projection| projection.definition_id.0)
            .collect::<Vec<_>>(),
        vec![4, 3, 2]
    );
    assert_eq!(effective_rotations, [0.0, 30.0, 60.0]);
    let expected = PatternCapabilityProjection {
        definition_id: projections[0].definition_id,
        family: PatternFamilyCapabilityProjection::Dispersion(DispersionCapabilityProjection {
            generator: GeneratorCapabilities {
                density: true,
                seed: true,
            },
            character: RandomCharacterKind::Even,
            density_modulation: DensityModulationKind::Uniform,
            exclusion: ExclusionKind::None,
        }),
        outputs: vec![PatternOutputCapabilityProjection::Marks(
            MarkOutputCapabilityProjection {
                prototype: MarkPrototypeKind::Circle,
                orientation: MarkOrientationKind::Fixed,
                fill_range: true,
            },
        )],
    };
    assert!(projections.iter().all(|projection| {
        projection.family == expected.family && projection.outputs == expected.outputs
    }));
}
