//! Main-preview intent, terminal failure and last-success coordination.
//! GTK, pixels, documents, evaluation and cache publication remain caller-owned.

use toniator_engine::PreviewRasterTarget;

/// Captures authoritative frame identity and the complete exact renderer target.
/// Tickets and media epochs identify attempts and deliberately do not create new intent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PreviewIntent {
    pub(crate) frame: (u64, u64, u64),
    pub(crate) target: PreviewRasterTarget,
}

/// Pairs an engine ticket with the app workspace that submitted it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PreviewSubmission {
    pub(crate) workspace_generation: u64,
    pub(crate) ticket: u64,
}

/// Retains one terminal diagnostic independently of whether its overlay is dismissed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PreviewFailure {
    pub(crate) intent: PreviewIntent,
    pub(crate) diagnostic: String,
    pub(crate) visible: bool,
}

/// Coordinates queued/preparing/submitted intent while retaining last-success identity.
/// It owns no pixels, document, evaluator, GTK or cache; engine acceptance remains authoritative.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct PreviewCoordinator {
    refresh_queued: bool,
    intent: Option<PreviewIntent>,
    submission: Option<PreviewSubmission>,
    last_accepted_ticket: Option<u64>,
    failure: Option<PreviewFailure>,
}

impl PreviewCoordinator {
    /// Records an unallocated refresh for existing view-model consumers without inventing intent.
    /// A terminal failure remains latched until a concrete frame/target can be compared.
    #[cfg(test)]
    pub(crate) fn queue_refresh(&mut self) {
        self.clear_submission();
        self.refresh_queued = true;
    }

    /// Admits new exact intent before caller invalidation/decode side effects.
    /// Equal failed or already pending intent is inert; unavailable allocation remains queued.
    pub(crate) fn queue_intent(&mut self, intent: Option<PreviewIntent>) -> bool {
        if intent.is_none() && self.failure.is_some() {
            return false;
        }
        if intent.is_some_and(|value| {
            self.is_failed(value) || (self.is_pending() && self.intent == Some(value))
        }) {
            return false;
        }
        self.clear_submission();
        self.refresh_queued = true;
        self.intent = intent;
        self.failure = None;
        true
    }

    /// Captures concrete source-preparation ownership before cloning sources or starting a job.
    pub(crate) fn prepare(&mut self, intent: PreviewIntent) -> bool {
        if self.is_failed(intent) {
            return false;
        }
        self.refresh_queued = true;
        self.intent = Some(intent);
        self.failure = None;
        true
    }

    /// Records a submitted ticket without changing its captured source/frame/target authority.
    pub(crate) fn submit(&mut self, workspace_generation: u64, ticket: u64) {
        self.refresh_queued = false;
        self.submission = Some(PreviewSubmission {
            workspace_generation,
            ticket,
        });
    }

    /// Clears pending ownership without dismissing a terminal failure or losing accepted pixels.
    pub(crate) fn clear_submission(&mut self) {
        self.refresh_queued = false;
        self.submission = None;
        self.intent = None;
    }

    /// Reconciles concrete cache intent without creating an upstream ticket or pending work.
    /// A changed intent retires obsolete failure; only complete visible success clears a matching
    /// failure. Waiting or partial coverage preserves a genuine failure for that same intent.
    pub(crate) fn cache_view(&mut self, intent: PreviewIntent, complete: bool) {
        self.clear_submission();
        self.intent = Some(intent);
        if complete
            || self
                .failure
                .as_ref()
                .is_some_and(|failure| failure.intent != intent)
        {
            self.failure = None;
        }
    }

    /// Clears all workspace-owned coordination on Close/replacement; old callbacks stay stale.
    pub(crate) fn clear_workspace(&mut self) {
        *self = Self::default();
    }

    /// Reports queued/preparing/submitted work independently of retained accepted pixels.
    pub(crate) const fn is_pending(&self) -> bool {
        self.refresh_queued || self.submission.is_some()
    }

    /// Preserves a matching ordinary fallback job across cache-only refinement notifications.
    /// Different artwork/target or an idle/failed intent never suppresses a new ordinary submission.
    pub(crate) fn preserve_ordinary(&self, intent: PreviewIntent, required: bool) -> bool {
        required && self.is_pending() && self.intent == Some(intent)
    }
    /// Returns the captured current intent while preparation or evaluation owns it.
    pub(crate) const fn intent(&self) -> Option<PreviewIntent> {
        self.intent
    }

    /// Returns the current engine submission identity for stale-result acceptance.
    pub(crate) const fn submission(&self) -> Option<PreviewSubmission> {
        self.submission
    }

    /// Tests exact failed creative/presentation intent without considering attempt identity.
    pub(crate) fn is_failed(&self, intent: PreviewIntent) -> bool {
        self.failure
            .as_ref()
            .is_some_and(|failure| failure.intent == intent)
    }

    /// Returns terminal failure including its independent overlay visibility.
    pub(crate) fn failure(&self) -> Option<&PreviewFailure> {
        self.failure.as_ref()
    }

    /// Records accepted success only for the matching current workspace/ticket.
    pub(crate) fn accept(&mut self, workspace_generation: u64, ticket: u64) -> bool {
        if !accepts_submission(workspace_generation, self.submission, ticket) {
            return false;
        }
        self.clear_submission();
        self.last_accepted_ticket = Some(ticket);
        self.failure = None;
        true
    }

    /// Latches a matching submitted failure while preserving the previous accepted ticket.
    pub(crate) fn fail(
        &mut self,
        workspace_generation: u64,
        ticket: u64,
        diagnostic: String,
    ) -> bool {
        if !accepts_submission(workspace_generation, self.submission, ticket) {
            return false;
        }
        let intent = self.intent;
        self.clear_submission();
        if let Some(intent) = intent {
            self.failure = Some(PreviewFailure {
                intent,
                diagnostic,
                visible: true,
            });
        }
        true
    }

    /// Latches only the captured matching pre-ticket preparation/submission failure.
    pub(crate) fn fail_preparation(&mut self, intent: PreviewIntent, diagnostic: String) -> bool {
        if self.intent != Some(intent) || self.submission.is_some() {
            return false;
        }
        self.clear_submission();
        self.failure = Some(PreviewFailure {
            intent,
            diagnostic,
            visible: true,
        });
        true
    }

    /// Clears a matching canceled/stale ticket without turning cancellation into a failure.
    pub(crate) fn cancel(&mut self, workspace_generation: u64, ticket: u64) -> bool {
        if !accepts_submission(workspace_generation, self.submission, ticket) {
            return false;
        }
        self.clear_submission();
        true
    }

    /// Hides only the current diagnostic presentation; equal automatic intent remains terminal.
    pub(crate) fn dismiss(&mut self) {
        if let Some(failure) = self.failure.as_mut() {
            failure.visible = false;
        }
    }

    /// Admits one explicit new attempt only for the still-current failed exact intent.
    pub(crate) fn retry(&mut self, intent: PreviewIntent) -> bool {
        if !self.is_failed(intent) {
            return false;
        }
        self.failure = None;
        self.clear_submission();
        true
    }

    /// Returns the last application-accepted ticket while pixels remain externally owned.
    pub(crate) const fn last_accepted_ticket(&self) -> Option<u64> {
        self.last_accepted_ticket
    }
}

/// Rejects results originating from superseded tickets or replaced workspaces.
pub(crate) fn accepts_submission(
    current_workspace_generation: u64,
    submission: Option<PreviewSubmission>,
    completion_ticket: u64,
) -> bool {
    submission.is_some_and(|submission| {
        submission.workspace_generation == current_workspace_generation
            && submission.ticket == completion_ticket
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a checked target witness without creating any document or GTK state.
    /// # Panics
    /// Panics only if the positive witness extent is rejected by renderer target validation.
    fn intent(revision: u64, width: u32) -> PreviewIntent {
        PreviewIntent {
            frame: (1, revision, 0),
            target: PreviewRasterTarget::new(width, 80).expect("target"),
        }
    }

    /// Proves failed equal refresh/dismiss remain terminal and Retry admits exactly one attempt.
    /// # Panics
    /// Panics if attempt identity creates intent or dismissed failure permits automatic work.
    #[test]
    fn terminal_intent_dismissal_and_one_retry_preserve_last_success() {
        let mut coordinator = PreviewCoordinator::default();
        coordinator.prepare(intent(0, 100));
        coordinator.submit(1, 1);
        assert!(coordinator.accept(1, 1));
        let failed = intent(1, 100);
        assert!(coordinator.queue_intent(Some(failed)));
        coordinator.prepare(failed);
        coordinator.submit(1, 2);
        assert!(coordinator.fail(1, 2, "typed failure".into()));
        assert!(!coordinator.queue_intent(Some(failed)));
        assert!(!coordinator.prepare(failed));
        coordinator.dismiss();
        assert!(!coordinator.failure().unwrap().visible);
        assert!(!coordinator.queue_intent(Some(failed)));
        assert_eq!(coordinator.last_accepted_ticket(), Some(1));
        assert!(coordinator.retry(failed));
        assert!(!coordinator.retry(failed));
        assert!(coordinator.queue_intent(Some(failed)));
        coordinator.prepare(failed);
        coordinator.submit(1, 3);
        assert!(coordinator.accept(1, 3));
        assert!(coordinator.failure().is_none());
    }

    /// Proves frame/revision/complete target changes create legitimate intent independently of family.
    /// # Panics
    /// Panics if real presentation/authored changes are suppressed by an older failure.
    #[test]
    fn changed_revision_frame_viewport_and_backdrop_recover_normally() {
        let original = intent(1, 100);
        for changed in [
            intent(2, 100),
            intent(1, 101),
            PreviewIntent {
                frame: (1, 1, 9),
                ..original
            },
            PreviewIntent {
                target: original
                    .target
                    .with_background(toniator_engine::RasterBackground::OpaqueBlack),
                ..original
            },
        ] {
            let mut coordinator = PreviewCoordinator::default();
            coordinator.prepare(original);
            assert!(coordinator.fail_preparation(original, "preparation".into()));
            assert!(coordinator.queue_intent(Some(changed)));
            assert!(coordinator.failure().is_none());
        }
    }

    /// Distinguishes crop origin, exact scale and workspace while ignoring attempt epochs.
    /// # Panics
    /// Panics if an actual raster transform or workspace change aliases a failed intent.
    #[test]
    fn exact_crop_scale_and_workspace_are_failure_identity() {
        let original = intent(1, 100);
        for changed in [
            PreviewIntent {
                target: PreviewRasterTarget::for_viewport(100, 80, 1.0, 0.0, 1.0).expect("crop"),
                ..original
            },
            PreviewIntent {
                target: PreviewRasterTarget::for_viewport(100, 80, 0.0, 0.0, 2.0).expect("scale"),
                ..original
            },
            PreviewIntent {
                frame: (2, 1, 0),
                ..original
            },
        ] {
            let mut coordinator = PreviewCoordinator::default();
            coordinator.prepare(original);
            assert!(coordinator.fail_preparation(original, "failed".into()));
            assert!(coordinator.queue_intent(Some(changed)));
            coordinator.prepare(changed);
            coordinator.submit(changed.frame.0, 9);
            assert!(!coordinator.accept(99, 9));
            assert!(coordinator.accept(changed.frame.0, 9));
        }
    }

    /// Proves stale preparation/tickets cannot clear newer pending intent or create failure.
    /// # Panics
    /// Panics if stale completion owns the newer request or changes accepted identity.
    #[test]
    fn stale_and_cancelled_completions_never_latch_terminal_failure() {
        let mut coordinator = PreviewCoordinator::default();
        let old = intent(1, 100);
        let new = intent(2, 100);
        coordinator.prepare(old);
        coordinator.submit(1, 4);
        coordinator.queue_intent(Some(new));
        coordinator.prepare(new);
        assert!(!coordinator.fail_preparation(old, "stale".into()));
        assert_eq!(coordinator.intent(), Some(new));
        coordinator.submit(1, 5);
        assert!(!coordinator.fail(1, 4, "stale".into()));
        assert!(!coordinator.cancel(1, 4));
        assert!(coordinator.cancel(1, 5));
        assert!(!coordinator.is_pending());
        assert!(coordinator.failure().is_none());
    }

    /// Proves pre-ticket failure clears queued progress and replacement revokes its ownership.
    /// # Panics
    /// Panics if source failures lack terminal ownership or survive workspace replacement.
    #[test]
    fn preparation_failure_and_close_are_atomic() {
        let mut coordinator = PreviewCoordinator::default();
        let current = intent(1, 100);
        assert!(coordinator.queue_intent(Some(current)));
        coordinator.prepare(current);
        assert!(coordinator.fail_preparation(current, "source.decode".into()));
        assert!(!coordinator.is_pending());
        assert!(coordinator.is_failed(current));
        coordinator.clear_workspace();
        assert!(coordinator.failure().is_none());
        assert!(!coordinator.fail_preparation(current, "late".into()));
        assert!(coordinator.queue_intent(Some(current)));
    }

    /// Retires obsolete cache-view errors without tickets and requires complete success for equal intent.
    /// # Panics
    /// Panics if partial cache coverage hides a matching failure or creates pending upstream work.
    #[test]
    fn cached_intent_and_visible_success_reconcile_terminal_failure() {
        let mut coordinator = PreviewCoordinator::default();
        let failed = intent(1, 100);
        coordinator.prepare(failed);
        assert!(coordinator.fail_preparation(failed, "failed A".into()));
        coordinator.cache_view(failed, false);
        assert!(coordinator.is_failed(failed));
        assert!(!coordinator.is_pending());
        assert!(coordinator.submission().is_none());
        coordinator.cache_view(failed, true);
        assert!(coordinator.failure().is_none());
        coordinator.prepare(failed);
        assert!(coordinator.fail_preparation(failed, "failed A again".into()));
        coordinator.cache_view(intent(1, 101), false);
        assert!(coordinator.failure().is_none());
        assert!(!coordinator.is_pending());
        coordinator.cache_view(intent(1, 101), true);
        assert!(coordinator.failure().is_none());
        assert_eq!(coordinator.last_accepted_ticket(), None);
    }

    /// Keeps failure terminal while GTK has no allocation to establish changed exact intent.
    /// # Panics
    /// Panics if an unallocated refresh clears failure or manufactures pending decode work.
    #[test]
    fn unavailable_allocation_preserves_terminal_failure_until_concrete_intent() {
        let mut coordinator = PreviewCoordinator::default();
        let current = intent(1, 100);
        coordinator.prepare(current);
        assert!(coordinator.fail_preparation(current, "source.decode".into()));
        assert!(!coordinator.queue_intent(None));
        assert!(coordinator.is_failed(current));
        assert!(!coordinator.is_pending());
        assert!(!coordinator.queue_intent(Some(current)));
        assert!(coordinator.queue_intent(Some(intent(2, 100))));
    }
    /// Keeps source/evaluation ownership across coarse notifications without admitting another request.
    /// # Panics
    /// Panics if equal pending fallback can resubmit or a new target is incorrectly suppressed.
    #[test]
    fn coarse_cache_notifications_preserve_one_matching_ordinary_submission() {
        let mut coordinator = PreviewCoordinator::default();
        let current = intent(0, 100);
        assert!(coordinator.prepare(current));
        assert!(coordinator.preserve_ordinary(current, true));
        assert!(!coordinator.queue_intent(Some(current)));
        coordinator.submit(1, 7);
        assert!(coordinator.preserve_ordinary(current, true));
        assert_eq!(coordinator.submission().unwrap().ticket, 7);
        assert!(!coordinator.preserve_ordinary(intent(0, 101), true));
        assert!(!coordinator.preserve_ordinary(current, false));
        coordinator.clear_submission();
        assert!(!coordinator.preserve_ordinary(current, true));
    }
}
