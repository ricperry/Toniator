use std::{
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use toniator_domain::{
    CanvasSpec, ChannelAppearance, ChannelId, ChannelPatternInstance, ChannelPatternLayoutDelta,
    ChannelSourceMapping, ChannelState, ColorValue, CoveragePolicy, DensityEditedAxis,
    DensityMetric2D, Document, DocumentCommand, DocumentHistory, DocumentId,
    DocumentPatternSettings, DocumentSession, MarkGeometryFieldEdit, MarkGeometryResponse,
    PatternDefinition, PatternDefinitionId, PatternGeometryResponse, PatternMechanismId,
    PatternOutputLayerId, SourceComponent, SourcePlacement, SourceReference, SourceReferenceId,
};
use toniator_engine::{
    CacheDiagnostics, CacheDisposition, ChannelDiagnosticCompletion, ChannelDiagnosticRequest,
    ChannelDiagnosticScheduler, ResolvedSource, SourceFormatHint, evaluate_channel_diagnostic,
    srgb_to_linear,
};

const CHANNEL_ID: ChannelId = ChannelId(1);
const COMPLETION_GUARD: Duration = Duration::from_secs(15);

/// Builds a scheduler fixture with document-owned pattern settings and one
/// channel that stores only explicit inheritable deltas and presentation data.
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
            vec![PatternDefinition::supported_straight_grid(
                PatternDefinitionId(1),
                "straight-grid",
                PatternMechanismId(1),
                PatternMechanismId(2),
                PatternOutputLayerId(1),
                CoveragePolicy {
                    guard_steps: 2,
                    additional_margin: 4.5,
                },
            )],
            DocumentPatternSettings {
                definition_id: PatternDefinitionId(1),
                density: DensityMetric2D {
                    across_x: 90.0,
                    across_y: 60.0,
                    aspect_locked: true,
                },
                pattern_rotation_degrees: 17.0,
                shape_rotation_degrees: 0.0,
                geometry_response: PatternGeometryResponse::Marks(MarkGeometryResponse {
                    minimum_fill: 0.2,
                    maximum_fill: 0.9,
                }),
            },
            vec![ChannelState {
                id: CHANNEL_ID,
                pattern_instance: ChannelPatternInstance {
                    definition_override: None,
                    layout_delta: ChannelPatternLayoutDelta {
                        density: None,
                        rotation_degrees: None,
                        translation_x: 3.25,
                        translation_y: -4.5,
                    },
                    shape_rotation_delta_degrees: None,
                    geometry_response_delta: None,
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
) -> ChannelDiagnosticRequest {
    ChannelDiagnosticRequest::new(
        session.evaluation_snapshot(CHANNEL_ID).unwrap(),
        ResolvedSource::new(
            SourceReferenceId::new("scheduler-source").unwrap(),
            bytes,
            format,
        )
        .unwrap(),
    )
}

fn wait_for_latest(scheduler: &ChannelDiagnosticScheduler) -> ChannelDiagnosticCompletion {
    let deadline = Instant::now() + COMPLETION_GUARD;
    loop {
        match scheduler.try_receive_latest().unwrap() {
            Some(completion) => return completion,
            None if Instant::now() < deadline => thread::yield_now(),
            None => panic!("evaluation worker did not complete before hang guard"),
        }
    }
}

fn assert_diagnostics(completion: &ChannelDiagnosticCompletion, expected: CacheDiagnostics) {
    assert_eq!(completion.cache_diagnostics(), Some(&expected));
    assert_eq!(*completion.cache_diagnostics().unwrap(), expected);
}

fn submit_and_accept(
    scheduler: &ChannelDiagnosticScheduler,
    session: &DocumentSession,
    request: ChannelDiagnosticRequest,
) -> ChannelDiagnosticCompletion {
    let ticket = scheduler.submit(request).unwrap();
    let completion = wait_for_latest(scheduler);
    assert_eq!(completion.ticket(), ticket);
    assert!(scheduler.accept_completion(&completion, session).unwrap());
    completion
}

/// Confirms scheduled diagnostics match synchronous results for both immutable
/// source formats without granting test code a second evaluation authority.
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
        let synchronous =
            evaluate_channel_diagnostic(request(&session, Arc::clone(&bytes), format)).unwrap();
        assert_eq!(
            synchronous.source_identity().content_hash,
            format!("sha256:{source_hash}")
        );
        let scheduler = ChannelDiagnosticScheduler::new().unwrap();
        let completion = submit_and_accept(
            &scheduler,
            &session,
            request(&session, Arc::clone(&bytes), format),
        );
        let ChannelDiagnosticCompletion::Completed { result, .. } = completion else {
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

/// Confirms site interchange leaves the diagnostic scheduler's accepted-cache
/// and latest-ticket authority unchanged for both immutable source formats.
#[test]
fn stage20a_site_interchange_keeps_diagnostic_scheduler_cache_and_latest_ticket_behavior() {
    let scheduler = ChannelDiagnosticScheduler::new().unwrap();
    for (path, format) in [
        ("../../assets/raster-sample.png", SourceFormatHint::Png),
        ("../../assets/vector-sample.svg", SourceFormatHint::Svg),
    ] {
        let bytes: Arc<[u8]> = std::fs::read(path).unwrap().into();
        let session = session();
        let first = submit_and_accept(
            &scheduler,
            &session,
            request(&session, Arc::clone(&bytes), format),
        );
        let first_result = first.result().unwrap().clone();
        let repeated = submit_and_accept(&scheduler, &session, request(&session, bytes, format));
        assert_diagnostics(
            &repeated,
            CacheDiagnostics {
                decoded_source: CacheDisposition::Hit,
                family: CacheDisposition::Hit,
                realization: CacheDisposition::Hit,
                scene: CacheDisposition::Hit,
                raster: CacheDisposition::Hit,
            },
        );
        assert_eq!(repeated.result(), Some(&first_result));
    }
    scheduler.shutdown().unwrap();
}

/// Confirms the accepted diagnostic cache invalidates at each current domain
/// boundary while preserving exact hits when no authoritative input changes.
#[test]
fn accepted_cache_obeys_the_complete_reuse_matrix_and_keeps_outputs_exact() {
    let bytes: Arc<[u8]> = std::fs::read("../../assets/raster-sample.png")
        .unwrap()
        .into();
    let mut session = session();
    let scheduler = ChannelDiagnosticScheduler::new().unwrap();
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
        .apply(&DocumentCommand::SetColorComponent {
            channel_id: CHANNEL_ID,
            component: toniator_domain::ColorComponent::Red,
            value: 0.3,
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

    let response_edit = session
        .document()
        .set_channel_mark_response_field_for_effective(
            CHANNEL_ID,
            MarkGeometryFieldEdit::MaximumFill(0.8),
        )
        .expect("response edit builds");
    session.apply(&response_edit).unwrap();
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

    let density_edit = session
        .document()
        .set_channel_density_for_effective(
            CHANNEL_ID,
            DensityEditedAxis::AcrossX,
            DensityMetric2D {
                across_x: 80.0,
                across_y: 60.0,
                aspect_locked: true,
            },
        )
        .expect("density edit builds");
    session.apply(&density_edit).unwrap();
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

    let mut history = DocumentHistory::new(session);
    let aspect_unlock = history
        .document()
        .set_document_density_aspect_lock(false)
        .expect("document aspect-lock command builds");
    history.apply(&aspect_unlock).unwrap();
    let unlocked = submit_and_accept(
        &scheduler,
        history.session(),
        request(history.session(), Arc::clone(&bytes), SourceFormatHint::Png),
    );
    assert_diagnostics(
        &unlocked,
        CacheDiagnostics {
            decoded_source: CacheDisposition::Hit,
            family: CacheDisposition::Miss,
            realization: CacheDisposition::Miss,
            scene: CacheDisposition::Miss,
            raster: CacheDisposition::Miss,
        },
    );

    let replacement_id = SourceReferenceId::new("scheduler-source-replacement").unwrap();
    history
        .apply(&DocumentCommand::SetSourceReference {
            source: SourceReference::Assigned(replacement_id.clone()),
        })
        .unwrap();
    let source_change = submit_and_accept(
        &scheduler,
        history.session(),
        ChannelDiagnosticRequest::new(
            history.session().evaluation_snapshot(CHANNEL_ID).unwrap(),
            ResolvedSource::new(replacement_id, Arc::clone(&bytes), SourceFormatHint::Png).unwrap(),
        ),
    );
    assert_diagnostics(
        &source_change,
        CacheDiagnostics {
            decoded_source: CacheDisposition::Miss,
            family: CacheDisposition::Hit,
            realization: CacheDisposition::Hit,
            scene: CacheDisposition::Hit,
            raster: CacheDisposition::Hit,
        },
    );
    scheduler.shutdown().unwrap();
}

/// Confirms cancelled, failed, superseded, and stale completions cannot
/// replace the last accepted cache transaction.
#[test]
fn unaccepted_stale_cancelled_and_failed_work_never_replaces_accepted_cache() {
    let bytes: Arc<[u8]> = std::fs::read("../../assets/raster-sample.png")
        .unwrap()
        .into();
    let mut session = session();
    let scheduler = ChannelDiagnosticScheduler::new().unwrap();
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
    let rotation_edit = session
        .document()
        .set_channel_pattern_rotation_for_effective(CHANNEL_ID, 18.0)
        .expect("rotation edit builds");
    session.apply(&rotation_edit).unwrap();
    assert!(
        !scheduler
            .accept_completion(&stale_document, &session)
            .unwrap()
    );
    let reset_rotation = session
        .document()
        .set_channel_pattern_rotation_for_effective(CHANNEL_ID, 17.0)
        .expect("rotation reset builds");
    session.apply(&reset_rotation).unwrap();

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
    let scheduler = ChannelDiagnosticScheduler::new().unwrap();
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
    let scheduler = ChannelDiagnosticScheduler::new().unwrap();
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

    let scheduler = ChannelDiagnosticScheduler::new().unwrap();
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

/// Confirms a completion requires both latest-ticket and current-document
/// revision gates before it can be presented.
#[test]
fn presentation_requires_both_scheduler_and_document_revision_gates() {
    let bytes: Arc<[u8]> = std::fs::read("../../assets/raster-sample.png")
        .unwrap()
        .into();
    let mut session = session();
    let scheduler = ChannelDiagnosticScheduler::new().unwrap();
    let ticket = scheduler
        .submit(request(&session, bytes, SourceFormatHint::Png))
        .unwrap();
    let completion = wait_for_latest(&scheduler);
    assert_eq!(completion.ticket(), ticket);
    assert!(scheduler.is_latest(ticket));
    assert!(session.accepts_evaluation(completion.token()));

    let rotation_edit = session
        .document()
        .set_channel_pattern_rotation_for_effective(CHANNEL_ID, 15.0)
        .expect("rotation edit builds");
    session.apply(&rotation_edit).unwrap();
    assert!(scheduler.is_latest(ticket));
    assert!(!session.accepts_evaluation(completion.token()));
    scheduler.shutdown().unwrap();
}

#[test]
fn explicit_shutdown_and_drop_join_the_worker_without_a_hang() {
    ChannelDiagnosticScheduler::new()
        .unwrap()
        .shutdown()
        .unwrap();
    drop(ChannelDiagnosticScheduler::new().unwrap());
}
