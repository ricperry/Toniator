mod scene_ready_tests {
    use super::*;

    /// Observes actual evaluator boundaries and optionally cancels directly after scene composition.
    struct SceneProbe {
        raster_calls: AtomicUsize,
        raster_context: Mutex<Option<toniator_patterns::ConstructionContext>>,
        scene_calls: AtomicUsize,
        cancel_after_scene: bool,
        cancelled: AtomicBool,
        progress: Mutex<Vec<EvaluationProgressStage>>,
    }
    impl SceneProbe {
        /// Creates a deterministic stage observer without timing-based absence assertions.
        fn new(cancel_after_scene: bool) -> Self {
            Self {
                raster_calls: AtomicUsize::new(0),
                raster_context: Mutex::new(None),
                scene_calls: AtomicUsize::new(0),
                cancel_after_scene,
                cancelled: AtomicBool::new(false),
                progress: Mutex::new(Vec::new()),
            }
        }
    }
    impl CancellationProbe for SceneProbe {
        /// Returns the deterministic cancellation state owned by this test invocation.
        fn is_cancelled(&self) -> bool {
            self.cancelled.load(Ordering::Acquire)
        }
        /// Records actual progress phases to prove scene-only work never reports rasterization.
        /// # Panics
        /// Panics if the test-owned progress mutex is poisoned.
        fn report_progress(&self, stage: EvaluationProgressStage, _: u16, _: u16) {
            self.progress.lock().unwrap().push(stage);
        }
        /// Counts real raster/scene entry and cancels at the selected actual scene completion.
        /// # Panics
        /// Panics if the test-owned raster-context mutex is poisoned.
        fn observe_stage(&self, stage: EvaluationStage, checkpoint: EvaluationCheckpoint) {
            if stage == EvaluationStage::Raster && checkpoint == EvaluationCheckpoint::Before {
                self.raster_calls.fetch_add(1, Ordering::Relaxed);
                *self.raster_context.lock().unwrap() =
                    Some(toniator_patterns::ConstructionContext::current());
            }
            if stage == EvaluationStage::Scene && checkpoint == EvaluationCheckpoint::After {
                self.scene_calls.fetch_add(1, Ordering::Relaxed);
                if self.cancel_after_scene {
                    self.cancelled.store(true, Ordering::Release);
                }
            }
        }
    }

    /// Proves the shared canonical phase is identical and enters no raster consumer or raster progress.
    /// # Panics
    /// Panics on changed canonical authority, missing family facts, fake raster work, or CPU byte drift.
    #[test]
    fn scene_ready_identical_authority_without_any_raster_call() {
        let measured = toniator_patterns::ConstructionContext::new(
            toniator_patterns::ConstructionMode::Complete,
            None,
            true,
        );
        let _measurement_guard = measured.enter();
        let session = scene_guard_session();
        let request = document_request(&session, valid_document_bytes());
        let probe = SceneProbe::new(false);
        let scene = evaluate_cached_scene(
            &request,
            EvaluationLimits::default(),
            &DocumentDerivedCache::default(),
            &probe,
            None,
            None,
        )
        .unwrap();
        assert_eq!(probe.raster_calls.load(Ordering::Relaxed), 0);
        assert_eq!(probe.scene_calls.load(Ordering::Relaxed), 1);
        assert!(
            !probe
                .progress
                .lock()
                .unwrap()
                .contains(&EvaluationProgressStage::RasterizingPreview)
        );
        assert!(scene.transaction.raster.is_none());
        let raster = evaluate_cached_document_impl(
            request.clone(),
            EvaluationLimits::default(),
            &DocumentDerivedCache::default(),
            &probe,
            None,
            None,
        )
        .unwrap();
        assert_eq!(probe.raster_calls.load(Ordering::Relaxed), 1);
        let context = probe.raster_context.lock().unwrap().clone().unwrap();
        assert!(Arc::ptr_eq(
            context.measurements.as_ref().unwrap(),
            measured.measurements.as_ref().unwrap()
        ));
        assert_eq!(context.mode, EvaluationLimits::default().construction_mode);
        assert_eq!(
            context.canvas,
            toniator_patterns::Bounds::new(
                toniator_patterns::Point2::new(0.0, 0.0),
                toniator_patterns::Point2::new(64.0, 48.0)
            )
        );
        assert_eq!(scene.result.token(), raster.result.token());
        assert_eq!(
            scene.result.source_identity(),
            raster.result.source_identity()
        );
        assert_eq!(scene.result.channels(), raster.result.channels());
        assert_eq!(scene.result.scene(), raster.result.scene());
        assert_eq!(scene.result.families, raster.result.families);
        let mut cache = DocumentDerivedCache::default();
        cache.commit(raster.transaction);
        let previous_raster = Arc::clone(&cache.raster.as_ref().unwrap().1);
        let canonical = evaluate_cached_scene(
            &request,
            EvaluationLimits::default(),
            &cache,
            &probe,
            None,
            None,
        )
        .unwrap();
        assert!(Arc::ptr_eq(&raster.result.scene, &canonical.result.scene));
        cache.commit(canonical.transaction);
        let again = evaluate_cached_document(
            request,
            EvaluationLimits::default(),
            &cache,
            &NeverCancelled,
        )
        .unwrap();
        assert_eq!(again.diagnostics.aggregate.raster, CacheDisposition::Hit);
        assert!(Arc::ptr_eq(&previous_raster, &again.result.raster));
        assert_eq!(raster.result.raster(), again.result.raster());
    }

    /// Proves received scene candidates remain private until latest session acceptance commits upstream work.
    /// # Panics
    /// Panics if receipt commits, acceptance drops upstream entries, or a scene candidate fabricates a raster.
    #[test]
    fn scene_ready_scheduler_acceptance_is_transactional() {
        let scheduler = EvaluationScheduler::new().unwrap();
        let session = scene_guard_session();
        let bytes = valid_document_bytes();
        scheduler
            .submit_scene(document_request(&session, Arc::clone(&bytes)))
            .unwrap();
        let candidate = wait_for_document_completion(&scheduler);
        assert!(candidate.result().is_none());
        assert!(candidate.scene_result().is_some());
        assert!(candidate.cache_diagnostics().is_none());
        assert!(scheduler.state.lock().unwrap().cache.scene.is_none());
        assert!(scheduler.accept_completion(&candidate, &session).unwrap());
        assert!(scheduler.accept_completion(&candidate, &session).unwrap());
        {
            let state = scheduler.state.lock().unwrap();
            assert!(state.cache.decoded_source.is_some());
            assert!(!state.cache.families.is_empty());
            assert!(state.cache.scene.is_some());
            assert!(state.cache.raster.is_none());
        }
        scheduler
            .submit_scene(document_request(&session, bytes))
            .unwrap();
        let warm = wait_for_document_completion(&scheduler);
        let diagnostics = warm.scene_cache_diagnostics().unwrap();
        assert_eq!(diagnostics.decoded_source, CacheDisposition::Hit);
        assert_eq!(diagnostics.family, CacheDisposition::Hit);
        assert_eq!(diagnostics.realization, CacheDisposition::Hit);
        assert_eq!(diagnostics.scene, CacheDisposition::Hit);
        scheduler.shutdown().unwrap();
    }

    /// Proves supersession, revision mismatch, cancellation and clear reject scene cache publication.
    /// # Panics
    /// Panics if an obsolete scene candidate installs upstream cache state.
    #[test]
    fn scene_ready_obsolete_candidates_never_commit() {
        let scheduler = EvaluationScheduler::new().unwrap();
        let mut session = scene_guard_session();
        let bytes = valid_document_bytes();
        scheduler
            .submit_scene(document_request(&session, Arc::clone(&bytes)))
            .unwrap();
        let old = wait_for_document_completion(&scheduler);
        scheduler
            .submit_scene(document_request(&session, Arc::clone(&bytes)))
            .unwrap();
        assert!(!scheduler.accept_completion(&old, &session).unwrap());
        let stale = wait_for_document_completion(&scheduler);
        let id = session.document().channel_topology().unwrap().channels()[0].id;
        session
            .apply(&DocumentCommand::SetOpacity {
                channel_id: id,
                opacity: 0.5,
            })
            .unwrap();
        assert!(!scheduler.accept_completion(&stale, &session).unwrap());
        assert!(scheduler.state.lock().unwrap().cache.scene.is_none());
        scheduler
            .submit_scene(document_request(&session, Arc::clone(&bytes)))
            .unwrap();
        let cancelled = wait_for_document_completion(&scheduler);
        scheduler.cancel_pending();
        assert!(!scheduler.accept_completion(&cancelled, &session).unwrap());
        scheduler
            .submit_scene(document_request(&session, bytes))
            .unwrap();
        let cleared = wait_for_document_completion(&scheduler);
        scheduler.cancel_and_clear();
        assert!(!scheduler.accept_completion(&cleared, &session).unwrap());
        assert!(scheduler.state.lock().unwrap().cache.scene.is_none());
        scheduler.shutdown().unwrap();
    }

    /// Proves cancellation at actual scene completion discards the entire candidate transaction.
    /// # Panics
    /// Panics if post-scene cancellation returns a publishable result or invokes the raster consumer.
    #[test]
    fn scene_ready_cancel_after_scene_has_no_partial_output() {
        let session = scene_guard_session();
        let probe = SceneProbe::new(true);
        let cache = DocumentDerivedCache::default();
        let outcome = evaluate_cached_scene(
            &document_request(&session, valid_document_bytes()),
            EvaluationLimits::default(),
            &cache,
            &probe,
            None,
            None,
        );
        assert!(matches!(outcome, Err(EvaluationRunError::Cancelled)));
        assert_eq!(probe.scene_calls.load(Ordering::Relaxed), 1);
        assert_eq!(probe.raster_calls.load(Ordering::Relaxed), 0);
        assert!(cache.scene.is_none());
    }

    /// Proves None invalidation rebinds the token while preserving canonical ownership; opacity rebuilds.
    /// # Panics
    /// Panics if document-only intent invalidates geometry or a real appearance edit reuses stale scene.
    #[test]
    fn scene_ready_none_invalidation_rebinds_token_with_same_scene() {
        let scheduler = EvaluationScheduler::new().unwrap();
        let mut history = DocumentHistory::new(scene_guard_session());
        let bytes = valid_document_bytes();
        scheduler
            .submit_scene(document_request(history.session(), Arc::clone(&bytes)))
            .unwrap();
        let first = wait_for_document_completion(&scheduler);
        assert!(
            scheduler
                .accept_completion(&first, history.session())
                .unwrap()
        );
        let first_scene = first.scene_result().unwrap().shared_scene();
        let base = history.document().print_preparation().clone();
        let settings = toniator_domain::PrintPreparationSettings::new(None, 0.2, 0.0).unwrap();
        history
            .apply(&DocumentCommand::SetPrintPreparation { base, settings })
            .unwrap();
        scheduler
            .submit_scene(document_request(history.session(), Arc::clone(&bytes)))
            .unwrap();
        let rebound = wait_for_document_completion(&scheduler);
        assert_ne!(first.token(), rebound.token());
        assert!(Arc::ptr_eq(
            &first_scene,
            &rebound.scene_result().unwrap().shared_scene()
        ));
        assert!(
            !scheduler
                .accept_completion(&first, history.session())
                .unwrap()
        );
        assert!(
            scheduler
                .accept_completion(&rebound, history.session())
                .unwrap()
        );
        let id = history.document().channel_topology().unwrap().channels()[0].id;
        history
            .apply(&DocumentCommand::SetOpacity {
                channel_id: id,
                opacity: 0.5,
            })
            .unwrap();
        scheduler
            .submit_scene(document_request(history.session(), bytes))
            .unwrap();
        let changed = wait_for_document_completion(&scheduler);
        assert_eq!(
            changed.scene_cache_diagnostics().unwrap().realization,
            CacheDisposition::Hit
        );
        assert!(!Arc::ptr_eq(
            &first_scene,
            &changed.scene_result().unwrap().shared_scene()
        ));
        scheduler.shutdown().unwrap();
    }
    /// Proves in-flight scene work cancelled at its actual composition gate cannot publish a cache.
    /// # Panics
    /// Panics if gated supersession publishes obsolete state or a replacement fails acceptance.
    #[test]
    fn scene_ready_inflight_supersession_uses_scene_gate() {
        let (gate, entered) =
            EvaluationStageGate::new(EvaluationStage::Scene, EvaluationCheckpoint::After);
        let scheduler =
            EvaluationScheduler::new_with_test_gate(EvaluationLimits::default(), Arc::clone(&gate))
                .unwrap();
        let session = scene_guard_session();
        let bytes = valid_document_bytes();
        let old = scheduler
            .submit_scene(document_request(&session, Arc::clone(&bytes)))
            .unwrap();
        entered.recv_timeout(GUARD).unwrap();
        let latest = scheduler
            .submit_scene(document_request(&session, bytes))
            .unwrap();
        gate.release();
        let completion = wait_for_document_completion(&scheduler);
        assert_ne!(old, completion.ticket());
        assert_eq!(latest, completion.ticket());
        assert!(scheduler.state.lock().unwrap().cache.scene.is_none());
        assert!(scheduler.accept_completion(&completion, &session).unwrap());
        let mut updates = Vec::new();
        while let Some(update) = scheduler.try_receive_latest_progress().unwrap() {
            updates.push(update);
        }
        assert!(
            updates
                .iter()
                .all(|progress| progress.stage() != EvaluationProgressStage::RasterizingPreview)
        );
        assert!(updates.iter().any(|progress| progress.stage()
            == EvaluationProgressStage::Complete
            && progress.fraction() == 1.0));
        scheduler.shutdown().unwrap();
    }
    /// Proves both immutable artworks share accepted canonical ownership across CPU targets,
    /// retain the ordinary byte/backdrop contract and enter no upstream evaluation stage.
    /// # Panics
    /// Panics on changed pixels, scene identity, cache retention, progress or source-decode count.
    #[test]
    fn preview_handoff_raster_only_matches_ordinary_cpu_for_both_artworks() {
        for (name, format) in [
            ("raster-sample.png", SourceFormatHint::Png),
            ("vector-sample.svg", SourceFormatHint::Svg),
        ] {
            let session = scene_guard_session();
            let bytes: Arc<[u8]> = std::fs::read(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../assets")
                    .join(name),
            )
            .unwrap()
            .into();
            let request =
                document_request_for_format(&session, "cancellation-test-source", bytes, format);
            let probe = SceneProbe::new(false);
            let canonical = evaluate_cached_scene(
                &request,
                EvaluationLimits::default(),
                &DocumentDerivedCache::default(),
                &NeverCancelled,
                None,
                None,
            )
            .unwrap();
            let scene = canonical.result.clone();
            let diagnostics = canonical.diagnostics.clone();
            let mut cache = DocumentDerivedCache::default();
            cache.commit(canonical.transaction);
            let family_count = cache.families.len();
            for target in [
                PreviewRasterTarget::new(32, 24).unwrap(),
                PreviewRasterTarget::for_viewport(24, 20, 3.25, 2.5, 1.5).unwrap(),
            ] {
                for background in [
                    RasterBackground::Transparent,
                    RasterBackground::OpaqueBlack,
                    RasterBackground::OpaqueWhite,
                ] {
                    let target = target.with_background(background);
                    let expected = evaluate_cached_document_impl(
                        request.clone().for_preview(target),
                        EvaluationLimits::default(),
                        &DocumentDerivedCache::default(),
                        &NeverCancelled,
                        None,
                        None,
                    )
                    .unwrap();
                    let output = evaluate_accepted_scene_raster(
                        target,
                        scene.clone(),
                        diagnostics.clone(),
                        EvaluationLimits::default(),
                        &cache,
                        &probe,
                    )
                    .unwrap();
                    assert!(Arc::ptr_eq(
                        &output.result.shared_scene(),
                        &scene.shared_scene()
                    ));
                    assert_eq!(output.result.raster(), expected.result.raster());
                    assert_eq!(output.result.source_identity(), scene.source_identity());
                    cache.commit(output.transaction);
                    assert_eq!(cache.families.len(), family_count);
                    let before = probe.raster_calls.load(Ordering::Relaxed);
                    let warm = evaluate_accepted_scene_raster(
                        target,
                        scene.clone(),
                        diagnostics.clone(),
                        EvaluationLimits::default(),
                        &cache,
                        &probe,
                    )
                    .unwrap();
                    assert_eq!(warm.diagnostics.aggregate.raster, CacheDisposition::Hit);
                    assert_eq!(probe.raster_calls.load(Ordering::Relaxed), before);
                }
            }
            assert_eq!(probe.scene_calls.load(Ordering::Relaxed), 0);
            assert!(probe.progress.lock().unwrap().iter().all(|stage| matches!(
                stage,
                EvaluationProgressStage::RasterizingPreview | EvaluationProgressStage::Finalizing
            )));
        }
    }

    /// Proves scene-only scheduler acceptance permits one source-free raster continuation and
    /// rejects unaccepted/cleared or superseded scene ownership, including equal-revision sources.
    /// # Panics
    /// Panics if stale candidates rasterize or accepted scene/source ownership drifts.
    #[test]
    fn preview_handoff_scheduler_rejects_stale_scene_and_recovers_after_restart() {
        let session = scene_guard_session();
        let scheduler = EvaluationScheduler::new().unwrap();
        let request = document_request(&session, valid_document_bytes());
        scheduler.submit_scene(request.clone()).unwrap();
        let canonical = wait_for_document_completion(&scheduler);
        let target = PreviewRasterTarget::new(16, 12)
            .unwrap()
            .with_background(RasterBackground::OpaqueBlack);
        scheduler
            .submit_scene_raster(target, &canonical)
            .unwrap()
            .unwrap();
        let unaccepted = wait_for_document_completion(&scheduler);
        assert_eq!(unaccepted.error().unwrap().path(), "evaluation.scene_stale");
        scheduler.submit_scene(request.clone()).unwrap();
        let canonical = wait_for_document_completion(&scheduler);
        assert!(scheduler.accept_completion(&canonical, &session).unwrap());
        scheduler
            .submit_scene_raster(target, &canonical)
            .unwrap()
            .unwrap();
        let first = wait_for_document_completion(&scheduler);
        assert!(Arc::ptr_eq(
            &first.result().unwrap().shared_scene(),
            &canonical.scene_result().unwrap().shared_scene()
        ));
        assert!(scheduler.accept_completion(&first, &session).unwrap());
        // A different source has the same document token; its accepted scene owns continuation.
        scheduler
            .submit_scene(document_request(&session, flat_document_bytes()))
            .unwrap();
        let other = wait_for_document_completion(&scheduler);
        assert_eq!(other.token(), canonical.token());
        assert!(scheduler.accept_completion(&other, &session).unwrap());
        scheduler
            .submit_scene_raster(target, &canonical)
            .unwrap()
            .unwrap();
        let wrong_source = wait_for_document_completion(&scheduler);
        assert_eq!(
            wrong_source.error().unwrap().path(),
            "evaluation.scene_stale"
        );
        scheduler.cancel_and_clear();
        scheduler
            .submit_scene_raster(target, &other)
            .unwrap()
            .unwrap();
        let cleared = wait_for_document_completion(&scheduler);
        assert_eq!(cleared.error().unwrap().path(), "evaluation.scene_stale");
        scheduler.shutdown().unwrap();
        let replacement = EvaluationScheduler::new().unwrap();
        replacement.submit_scene(request).unwrap();
        let recovered = wait_for_document_completion(&replacement);
        assert!(replacement.accept_completion(&recovered, &session).unwrap());
        replacement
            .submit_scene_raster(target, &recovered)
            .unwrap()
            .unwrap();
        let pixels = wait_for_document_completion(&replacement);
        assert!(replacement.accept_completion(&pixels, &session).unwrap());
        replacement.shutdown().unwrap();
    }

    /// Proves raster-only supersession uses the existing raster cancellation gate and rejects
    /// stale document revisions before pixel publication without regenerating canonical geometry.
    /// # Panics
    /// Panics if a canceled ticket or changed-revision raster can become accepted.
    #[test]
    fn preview_handoff_raster_cancellation_and_revision_rejection() {
        let (gate, entered) =
            EvaluationStageGate::new(EvaluationStage::Raster, EvaluationCheckpoint::Before);
        let scheduler =
            EvaluationScheduler::new_with_test_gate(EvaluationLimits::default(), Arc::clone(&gate))
                .unwrap();
        let mut session = scene_guard_session();
        scheduler
            .submit_scene(document_request(&session, valid_document_bytes()))
            .unwrap();
        let canonical = wait_for_document_completion(&scheduler);
        assert!(scheduler.accept_completion(&canonical, &session).unwrap());
        let target = PreviewRasterTarget::new(16, 12).unwrap();
        let old = scheduler
            .submit_scene_raster(target, &canonical)
            .unwrap()
            .unwrap();
        entered.recv_timeout(GUARD).unwrap();
        let latest = scheduler
            .submit_scene_raster(target.with_background(RasterBackground::OpaqueWhite), &canonical)
            .unwrap()
            .unwrap();
        gate.release();
        let completion = wait_for_document_completion(&scheduler);
        assert_ne!(old, completion.ticket());
        assert_eq!(latest, completion.ticket());
        let id = session.document().channel_topology().unwrap().channels()[0].id;
        session
            .apply(&DocumentCommand::SetOpacity {
                channel_id: id,
                opacity: 0.001,
            })
            .unwrap();
        assert!(!scheduler.accept_completion(&completion, &session).unwrap());
        scheduler.shutdown().unwrap();
    }
}
