//! Typed asynchronous completions delivered onto GTK's main context.

use crate::{ExportFormat, LifecycleAction, SavedContent, Workspace};
use toniator_engine::EvaluationCompletion;

/// Carries worker results without granting workers GTK or document authority.
pub(crate) enum AppEvent {
    /// Finishes one generation-scoped open request.
    Load {
        generation: u64,
        result: Box<Result<Workspace, String>>,
    },
    /// Finishes one immutable save snapshot.
    Save {
        generation: u64,
        path: std::path::PathBuf,
        snapshot: SavedContent,
        after: Option<LifecycleAction>,
        result: Result<(), String>,
    },
    /// Finishes one generation-scoped export request.
    Export {
        generation: u64,
        workspace_generation: u64,
        format: ExportFormat,
        result: Result<(), String>,
    },
    /// Delivers the scheduler's newest candidate for main-context acceptance.
    Preview(EvaluationCompletion),
    /// Delivers a private Pattern Editor scheduler candidate for one editor epoch.
    DraftPreview {
        epoch: u64,
        completion: EvaluationCompletion,
    },
}
