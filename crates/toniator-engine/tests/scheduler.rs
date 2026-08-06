use std::{
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use toniator_domain::{
    CanvasSpec, ChannelAppearance, ChannelId, ChannelPatternLayout, ChannelSourceMapping,
    ChannelState, ColorValue, DensityMetric2D, Document, DocumentCommand, DocumentId,
    DocumentSession, MarkGeometryResponse, PatternDefinition, PatternDefinitionId, PatternOutput,
    PatternStructure, SourceComponent, SourcePlacement, SourceReference, SourceReferenceId,
};
use toniator_engine::{
    EvaluationCompletion, EvaluationRequest, EvaluationScheduler, ResolvedSource, SourceFormatHint,
    evaluate, srgb_to_linear,
};

const CHANNEL_ID: ChannelId = ChannelId(1);
const COMPLETION_GUARD: Duration = Duration::from_secs(15);

fn session() -> DocumentSession {
    let source_id = SourceReferenceId::new("scheduler-source").unwrap();
    DocumentSession::new(
        Document::with_source(
            DocumentId(1),
            CanvasSpec {
                width: 900.0,
                height: 600.0,
            },
            SourceReference::Assigned(source_id),
            vec![PatternDefinition {
                id: PatternDefinitionId(1),
                name: "straight-grid".to_owned(),
                structure: PatternStructure::StraightGrid,
                output: PatternOutput::CircularMarks,
                guard_steps: 2,
            }],
            vec![ChannelState {
                id: CHANNEL_ID,
                pattern_definition_id: PatternDefinitionId(1),
                layout: ChannelPatternLayout {
                    density: DensityMetric2D {
                        across_x: 90.0,
                        across_y: 60.0,
                        aspect_locked: true,
                    },
                    rotation_degrees: 17.0,
                    translation_x: 3.25,
                    translation_y: -4.5,
                },
                appearance: ChannelAppearance {
                    visible: true,
                    color: ColorValue {
                        red: 0.0,
                        green: srgb_to_linear(183.0 / 255.0),
                        blue: 1.0,
                        alpha: 1.0,
                    },
                    opacity: 0.72,
                },
                mark_geometry_response: MarkGeometryResponse {
                    minimum_size: 2.0,
                    maximum_size: 9.0,
                },
                source_mapping: ChannelSourceMapping {
                    component: SourceComponent::Luminance,
                    placement: SourcePlacement::StretchToCanvas,
                },
            }],
        )
        .unwrap(),
    )
    .unwrap()
}

fn request(
    session: &DocumentSession,
    bytes: Arc<[u8]>,
    format: SourceFormatHint,
) -> EvaluationRequest {
    EvaluationRequest::new(
        session.evaluation_snapshot(CHANNEL_ID).unwrap(),
        ResolvedSource::new(
            SourceReferenceId::new("scheduler-source").unwrap(),
            bytes,
            format,
        )
        .unwrap(),
    )
}

fn wait_for_latest(scheduler: &EvaluationScheduler) -> EvaluationCompletion {
    let deadline = Instant::now() + COMPLETION_GUARD;
    loop {
        match scheduler.try_receive_latest().unwrap() {
            Some(completion) => return completion,
            None if Instant::now() < deadline => thread::yield_now(),
            None => panic!("evaluation worker did not complete before hang guard"),
        }
    }
}

#[test]
fn scheduled_raster_and_svg_baselines_match_synchronous_results_exactly() {
    for (path, format, source_hash) in [
        (
            "../../assets/raster-sample.png",
            SourceFormatHint::Png,
            "324ac232e319002a13fbcfac46538ca5d7e8ba8a127eea2eaf20e8ddb3ed2ef2",
        ),
        (
            "../../assets/vector-sample.svg",
            SourceFormatHint::Svg,
            "42eb5e23111a5dbad66f2b1802a7cc06391c7ede829b99eb28aeb1ac91596e2e",
        ),
    ] {
        let bytes: Arc<[u8]> = std::fs::read(path).unwrap().into();
        let session = session();
        let synchronous = evaluate(request(&session, Arc::clone(&bytes), format)).unwrap();
        assert_eq!(
            synchronous.source_identity().content_hash,
            format!("sha256:{source_hash}")
        );
        let scheduler = EvaluationScheduler::new().unwrap();
        let ticket = scheduler.submit(request(&session, bytes, format)).unwrap();
        let EvaluationCompletion::Completed {
            ticket: completed_ticket,
            result,
        } = wait_for_latest(&scheduler)
        else {
            panic!("valid baseline must complete")
        };
        assert_eq!(completed_ticket, ticket);
        assert_eq!(*result, synchronous);
        assert_eq!(
            result.token(),
            session.evaluation_token(CHANNEL_ID).unwrap()
        );
        scheduler.shutdown().unwrap();
    }
}

#[test]
fn only_the_newest_of_rapid_submissions_is_retrievable() {
    let bytes: Arc<[u8]> = std::fs::read("../../assets/raster-sample.png")
        .unwrap()
        .into();
    let session = session();
    let scheduler = EvaluationScheduler::new().unwrap();
    let first = scheduler
        .submit(request(&session, Arc::clone(&bytes), SourceFormatHint::Png))
        .unwrap();
    let second = scheduler
        .submit(request(&session, Arc::clone(&bytes), SourceFormatHint::Png))
        .unwrap();
    let newest = scheduler
        .submit(request(&session, bytes, SourceFormatHint::Png))
        .unwrap();
    assert!(first < second && second < newest);
    let completion = wait_for_latest(&scheduler);
    assert_eq!(completion.ticket(), newest);
    assert!(scheduler.is_latest(newest));
    assert_eq!(scheduler.try_receive_latest().unwrap(), None);
    scheduler.shutdown().unwrap();
}

#[test]
fn superseded_failures_are_silent_but_the_newest_failure_keeps_its_ticket_and_token() {
    let bytes: Arc<[u8]> = std::fs::read("../../assets/raster-sample.png")
        .unwrap()
        .into();
    let session = session();
    let scheduler = EvaluationScheduler::new().unwrap();
    let stale = scheduler
        .submit(request(
            &session,
            Arc::<[u8]>::from(vec![1_u8, 2, 3]),
            SourceFormatHint::Png,
        ))
        .unwrap();
    let newest = scheduler
        .submit(request(&session, bytes, SourceFormatHint::Png))
        .unwrap();
    let completion = wait_for_latest(&scheduler);
    assert_eq!(completion.ticket(), newest);
    assert_ne!(completion.ticket(), stale);
    assert!(completion.result().is_some());
    scheduler.shutdown().unwrap();

    let scheduler = EvaluationScheduler::new().unwrap();
    let ticket = scheduler
        .submit(request(
            &session,
            Arc::<[u8]>::from(vec![1_u8, 2, 3]),
            SourceFormatHint::Png,
        ))
        .unwrap();
    let completion = wait_for_latest(&scheduler);
    assert_eq!(completion.ticket(), ticket);
    assert_eq!(
        completion.token(),
        session.evaluation_token(CHANNEL_ID).unwrap()
    );
    assert_eq!(completion.error().unwrap().path(), "source.format");
    scheduler.shutdown().unwrap();
}

#[test]
fn presentation_requires_both_scheduler_and_document_revision_gates() {
    let bytes: Arc<[u8]> = std::fs::read("../../assets/raster-sample.png")
        .unwrap()
        .into();
    let mut session = session();
    let scheduler = EvaluationScheduler::new().unwrap();
    let ticket = scheduler
        .submit(request(&session, bytes, SourceFormatHint::Png))
        .unwrap();
    let completion = wait_for_latest(&scheduler);
    assert_eq!(completion.ticket(), ticket);
    assert!(scheduler.is_latest(ticket));
    assert!(session.accepts_evaluation(completion.token()));

    session
        .apply(&DocumentCommand::SetRotation {
            channel_id: CHANNEL_ID,
            rotation_degrees: 15.0,
        })
        .unwrap();
    assert!(scheduler.is_latest(ticket));
    assert!(!session.accepts_evaluation(completion.token()));
    scheduler.shutdown().unwrap();
}

#[test]
fn explicit_shutdown_and_drop_join_the_worker_without_a_hang() {
    EvaluationScheduler::new().unwrap().shutdown().unwrap();
    drop(EvaluationScheduler::new().unwrap());
}
