//! Main-preview identity and last-success acceptance coordination.
//!
//! This module has no GTK or evaluator implementation dependency. It records
//! only application-side identity so the engine remains authoritative for
//! document-token and scheduler-ticket acceptance.

/// Pairs an engine ticket with the app workspace that submitted it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PreviewSubmission {
    /// Identifies the workspace that submitted the request.
    pub(crate) workspace_generation: u64,
    /// Identifies the scheduler request inside that workspace.
    pub(crate) ticket: u64,
}

/// Tracks the current request and the last result accepted by the application.
///
/// The coordinator preserves a last accepted preview while a newer request is
/// pending or rejected. It never owns pixels, GTK paintables, a document, or
/// scheduler cache; callers install those only after engine acceptance.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct PreviewCoordinator {
    submission: Option<PreviewSubmission>,
    last_accepted_ticket: Option<u64>,
}

impl PreviewCoordinator {
    /// Records one submitted scheduler ticket for the current workspace.
    pub(crate) fn submit(&mut self, workspace_generation: u64, ticket: u64) {
        self.submission = Some(PreviewSubmission {
            workspace_generation,
            ticket,
        });
    }

    /// Clears only the pending identity when a workspace is replaced or closed.
    pub(crate) fn clear_submission(&mut self) {
        self.submission = None;
    }

    /// Returns the current request identity for observational event evidence.
    pub(crate) const fn submission(&self) -> Option<PreviewSubmission> {
        self.submission
    }

    /// Accepts only the ticket submitted by this exact workspace generation.
    ///
    /// A rejected ticket leaves `last_accepted_ticket` unchanged so callers can
    /// keep rendering the prior successful preview. Accepted tickets become the
    /// new last-success identity without clearing a future pending submission.
    pub(crate) fn accept(&mut self, workspace_generation: u64, ticket: u64) -> bool {
        if !accepts_submission(workspace_generation, self.submission, ticket) {
            return false;
        }
        self.last_accepted_ticket = Some(ticket);
        true
    }

    /// Returns the last scheduler ticket accepted by application identity rules.
    pub(crate) const fn last_accepted_ticket(&self) -> Option<u64> {
        self.last_accepted_ticket
    }
}

/// Rejects results that originated from a replaced app workspace.
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

    /// Preserves the accepted ticket while stale or superseded completions arrive.
    #[test]
    fn coordinator_rejects_stale_generation_without_losing_last_success() {
        let mut coordinator = PreviewCoordinator::default();
        coordinator.submit(4, 11);
        assert!(coordinator.accept(4, 11));
        coordinator.submit(5, 12);
        assert!(!coordinator.accept(5, 11));
        assert_eq!(coordinator.last_accepted_ticket(), Some(11));
        assert!(coordinator.accept(5, 12));
        assert_eq!(coordinator.last_accepted_ticket(), Some(12));
    }
}
