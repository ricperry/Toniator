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
}
