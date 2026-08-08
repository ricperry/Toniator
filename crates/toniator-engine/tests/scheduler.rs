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
    CacheDiagnostics, CacheDisposition, EvaluationCompletion, EvaluationRequest,
    EvaluationScheduler, ResolvedSource, SourceFormatHint, evaluate, srgb_to_linear,
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
                maximum_support_radius: 4.5,
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

fn assert_diagnostics(completion: &EvaluationCompletion, expected: CacheDiagnostics) {
    assert_eq!(completion.cache_diagnostics(), Some(&expected));
    assert_eq!(*completion.cache_diagnostics().unwrap(), expected);
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
        let completion = submit_and_accept(
            &scheduler,
            &session,
            request(&session, Arc::clone(&bytes), format),
        );
        let EvaluationCompletion::Completed { result, .. } = completion else {
            panic!("valid baseline must complete")
        };
        assert_eq!(*result, synchronous);
        assert_eq!(
            result.token(),
            session.evaluation_token(CHANNEL_ID).unwrap()
        );
        let cached = submit_and_accept(&scheduler, &session, request(&session, bytes, format));
        assert_diagnostics(
            &cached,
            CacheDiagnostics {
                decoded_source: CacheDisposition::Hit,
                family: CacheDisposition::Hit,
                realization: CacheDisposition::Hit,
                scene: CacheDisposition::Hit,
                raster: CacheDisposition::Hit,
            },
        );
        assert_eq!(cached.result(), Some(&synchronous));
        assert_eq!(
            cached.result().unwrap().raster().pixels(),
            synchronous.raster().pixels(),
        );
        assert_eq!(
            toniator_engine::write_svg(cached.result().unwrap().scene()),
            toniator_engine::write_svg(synchronous.scene()),
        );
        scheduler.shutdown().unwrap();
    }
}

#[test]
fn accepted_cache_obeys_the_complete_reuse_matrix_and_keeps_outputs_exact() {
    let bytes: Arc<[u8]> = std::fs::read("../../assets/raster-sample.png")
        .unwrap()
        .into();
    let mut session = session();
    let scheduler = EvaluationScheduler::new().unwrap();
    let miss = CacheDiagnostics {
        decoded_source: CacheDisposition::Miss,
        family: CacheDisposition::Miss,
        realization: CacheDisposition::Miss,
        scene: CacheDisposition::Miss,
        raster: CacheDisposition::Miss,
    };
    let hit = CacheDiagnostics {
        decoded_source: CacheDisposition::Hit,
        family: CacheDisposition::Hit,
        realization: CacheDisposition::Hit,
        scene: CacheDisposition::Hit,
        raster: CacheDisposition::Hit,
    };
    let first = submit_and_accept(
        &scheduler,
        &session,
        request(&session, Arc::clone(&bytes), SourceFormatHint::Png),
    );
    assert_diagnostics(&first, miss);
    assert!(
        scheduler.accept_completion(&first, &session).unwrap(),
        "acceptance is idempotent"
    );

    let exact = submit_and_accept(
        &scheduler,
        &session,
        request(&session, Arc::clone(&bytes), SourceFormatHint::Png),
    );
    assert_diagnostics(&exact, hit);
    assert_eq!(first.result(), exact.result());

    session
        .apply(&DocumentCommand::SetColor {
            channel_id: CHANNEL_ID,
            color: ColorValue {
                red: 0.3,
                green: 0.2,
                blue: 0.1,
                alpha: 1.0,
            },
        })
        .unwrap();
    let presentation = submit_and_accept(
        &scheduler,
        &session,
        request(&session, Arc::clone(&bytes), SourceFormatHint::Png),
    );
    assert_diagnostics(
        &presentation,
        CacheDiagnostics {
            decoded_source: CacheDisposition::Hit,
            family: CacheDisposition::Hit,
            realization: CacheDisposition::Hit,
            scene: CacheDisposition::Miss,
            raster: CacheDisposition::Miss,
        },
    );

    session
        .apply(&DocumentCommand::SetMarkGeometryResponse {
            channel_id: CHANNEL_ID,
            response: MarkGeometryResponse {
                minimum_size: 1.0,
                maximum_size: 8.0,
            },
        })
        .unwrap();
    let response = submit_and_accept(
        &scheduler,
        &session,
        request(&session, Arc::clone(&bytes), SourceFormatHint::Png),
    );
    assert_diagnostics(
        &response,
        CacheDiagnostics {
            decoded_source: CacheDisposition::Hit,
            family: CacheDisposition::Hit,
            realization: CacheDisposition::Miss,
            scene: CacheDisposition::Miss,
            raster: CacheDisposition::Miss,
        },
    );

    session
        .apply(&DocumentCommand::SetDensity {
            channel_id: CHANNEL_ID,
            density: DensityMetric2D {
                across_x: 80.0,
                across_y: 60.0,
                aspect_locked: true,
            },
        })
        .unwrap();
    let family = submit_and_accept(
        &scheduler,
        &session,
        request(&session, Arc::clone(&bytes), SourceFormatHint::Png),
    );
    assert_diagnostics(
        &family,
        CacheDiagnostics {
            decoded_source: CacheDisposition::Hit,
            family: CacheDisposition::Miss,
            realization: CacheDisposition::Miss,
            scene: CacheDisposition::Miss,
            raster: CacheDisposition::Miss,
        },
    );

    session
        .apply(&DocumentCommand::SetDensity {
            channel_id: CHANNEL_ID,
            density: DensityMetric2D {
                across_x: 80.0,
                across_y: 60.0,
                aspect_locked: false,
            },
        })
        .unwrap();
    let aspect_only = submit_and_accept(
        &scheduler,
        &session,
        request(&session, Arc::clone(&bytes), SourceFormatHint::Png),
    );
    assert_diagnostics(
        &aspect_only,
        CacheDiagnostics {
            decoded_source: CacheDisposition::Hit,
            family: CacheDisposition::Miss,
            realization: CacheDisposition::Miss,
            scene: CacheDisposition::Miss,
            raster: CacheDisposition::Miss,
        },
    );

    let replacement_id = SourceReferenceId::new("scheduler-source-replacement").unwrap();
    session
        .apply(&DocumentCommand::SetSourceReference {
            source: SourceReference::Assigned(replacement_id.clone()),
        })
        .unwrap();
    let source_change = submit_and_accept(
        &scheduler,
        &session,
        EvaluationRequest::new(
            session.evaluation_snapshot(CHANNEL_ID).unwrap(),
            ResolvedSource::new(replacement_id, Arc::clone(&bytes), SourceFormatHint::Png).unwrap(),
        ),
    );
    assert_diagnostics(&source_change, miss);
    scheduler.shutdown().unwrap();
}

#[test]
fn unaccepted_stale_cancelled_and_failed_work_never_replaces_accepted_cache() {
    let bytes: Arc<[u8]> = std::fs::read("../../assets/raster-sample.png")
        .unwrap()
        .into();
    let mut session = session();
    let scheduler = EvaluationScheduler::new().unwrap();
    let accepted = submit_and_accept(
        &scheduler,
        &session,
        request(&session, Arc::clone(&bytes), SourceFormatHint::Png),
    );

    // A completed presentation edit is deliberately not accepted. A later
    // return to the accepted document proves its staged scene/raster were not installed.
    session
        .apply(&DocumentCommand::SetOpacity {
            channel_id: CHANNEL_ID,
            opacity: 0.4,
        })
        .unwrap();
    let unaccepted = {
        let ticket = scheduler
            .submit(request(&session, Arc::clone(&bytes), SourceFormatHint::Png))
            .unwrap();
        let completion = wait_for_latest(&scheduler);
        assert_eq!(completion.ticket(), ticket);
        completion
    };
    assert!(unaccepted.result().is_some());
    session
        .apply(&DocumentCommand::SetOpacity {
            channel_id: CHANNEL_ID,
            opacity: 0.72,
        })
        .unwrap();
    let returned = submit_and_accept(
        &scheduler,
        &session,
        request(&session, Arc::clone(&bytes), SourceFormatHint::Png),
    );
    assert_diagnostics(
        &returned,
        CacheDiagnostics {
            decoded_source: CacheDisposition::Hit,
            family: CacheDisposition::Hit,
            realization: CacheDisposition::Hit,
            scene: CacheDisposition::Hit,
            raster: CacheDisposition::Hit,
        },
    );
    assert_eq!(
        returned.result().unwrap().source_identity(),
        accepted.result().unwrap().source_identity()
    );
    assert_eq!(
        returned.result().unwrap().scene(),
        accepted.result().unwrap().scene()
    );
    assert_eq!(
        returned.result().unwrap().raster(),
        accepted.result().unwrap().raster()
    );

    // A document-stale completion is rejected even while its ticket is latest.
    let ticket = scheduler
        .submit(request(&session, Arc::clone(&bytes), SourceFormatHint::Png))
        .unwrap();
    let stale_document = wait_for_latest(&scheduler);
    assert_eq!(stale_document.ticket(), ticket);
    session
        .apply(&DocumentCommand::SetRotation {
            channel_id: CHANNEL_ID,
            rotation_degrees: 18.0,
        })
        .unwrap();
    assert!(
        !scheduler
            .accept_completion(&stale_document, &session)
            .unwrap()
    );
    session
        .apply(&DocumentCommand::SetRotation {
            channel_id: CHANNEL_ID,
            rotation_degrees: 17.0,
        })
        .unwrap();

    // Submitting a new ticket makes a retained completion ticket-stale and
    // discards its unaccepted transaction.
    let older = {
        let ticket = scheduler
            .submit(request(&session, Arc::clone(&bytes), SourceFormatHint::Png))
            .unwrap();
        let completion = wait_for_latest(&scheduler);
        assert_eq!(completion.ticket(), ticket);
        completion
    };
    let newest = submit_and_accept(
        &scheduler,
        &session,
        request(&session, Arc::clone(&bytes), SourceFormatHint::Png),
    );
    assert!(!scheduler.accept_completion(&older, &session).unwrap());
    assert_diagnostics(
        &newest,
        CacheDiagnostics {
            decoded_source: CacheDisposition::Hit,
            family: CacheDisposition::Hit,
            realization: CacheDisposition::Hit,
            scene: CacheDisposition::Hit,
            raster: CacheDisposition::Hit,
        },
    );

    // Superseding an active valid request cancels or coalesces it before it can
    // commit; the current failure is accepted for presentation but owns no
    // transaction. A valid later request still reuses the last accepted cache.
    let cancelled_ticket = scheduler
        .submit(request(&session, Arc::clone(&bytes), SourceFormatHint::Png))
        .unwrap();
    let failed = {
        let ticket = scheduler
            .submit(request(
                &session,
                Arc::<[u8]>::from(vec![1_u8, 2, 3]),
                SourceFormatHint::Png,
            ))
            .unwrap();
        assert_ne!(ticket, cancelled_ticket);
        let completion = wait_for_latest(&scheduler);
        assert_eq!(completion.ticket(), ticket);
        assert!(completion.error().is_some());
        completion
    };
    assert!(scheduler.accept_completion(&failed, &session).unwrap());
    assert_eq!(failed.cache_diagnostics(), None);
    let after_failure = submit_and_accept(
        &scheduler,
        &session,
        request(&session, bytes, SourceFormatHint::Png),
    );
    assert_diagnostics(
        &after_failure,
        CacheDiagnostics {
            decoded_source: CacheDisposition::Hit,
            family: CacheDisposition::Hit,
            realization: CacheDisposition::Hit,
            scene: CacheDisposition::Hit,
            raster: CacheDisposition::Hit,
        },
    );
    scheduler.shutdown().unwrap();
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
