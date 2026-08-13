//! Widget-free application ownership for the Toniator desktop frontend.
//!
//! GTK surfaces project and dispatch against this model; they never become a
//! second document, history, lifecycle, or scheduler authority.

use std::sync::Arc;

use toniator_domain::ChannelId;
use toniator_engine::EvaluationScheduler;

use crate::{PreviewModel, Workspace, preview_coordinator::PreviewCoordinator};

/// Owns mutable application authority that is independent of GTK widget life.
///
/// `Workspace` transitively owns `DocumentHistory`, source/savepoint state and
/// location. This model owns lifecycle generations, pending-operation state,
/// selected artist channel and scheduler identity. GTK may borrow it only long
/// enough to project immutable view models or dispatch existing typed commands.
pub(crate) struct ApplicationModel {
    /// Evaluates immutable snapshots and never receives GTK widgets.
    pub(crate) scheduler: Arc<EvaluationScheduler>,
    /// Holds the sole main document/history/savepoint authority when open.
    pub(crate) workspace: Option<Workspace>,
    /// Marks a generation-scoped source load awaiting an event completion.
    pub(crate) pending_load: bool,
    /// Marks an immutable save snapshot awaiting an event completion.
    pub(crate) pending_save: bool,
    /// Marks an immutable export snapshot awaiting an event completion.
    pub(crate) pending_export: bool,
    /// Rejects stale lifecycle completions after a newer request begins.
    pub(crate) generation: u64,
    /// Distinguishes independent workspaces with the same document revision.
    pub(crate) workspace_generation: u64,
    /// Retains main-preview submission and last-success identity without pixels.
    pub(crate) preview_coordinator: PreviewCoordinator,
    /// Projects the artist-selected channel model for canvas/sidebar views.
    pub(crate) model: PreviewModel,
    /// Stores the stable selected channel independently of GTK list positions.
    pub(crate) selected_channel: Option<ChannelId>,
}

impl ApplicationModel {
    /// Creates an empty model with a running scheduler and no document authority.
    ///
    /// # Panics
    ///
    /// Panics only when the required in-process evaluation scheduler cannot be
    /// constructed; without it the desktop application cannot render previews.
    pub(crate) fn new() -> Self {
        Self {
            scheduler: Arc::new(
                EvaluationScheduler::new().expect("failed to start evaluation scheduler"),
            ),
            workspace: None,
            pending_load: false,
            pending_save: false,
            pending_export: false,
            generation: 0,
            workspace_generation: 0,
            preview_coordinator: PreviewCoordinator::default(),
            model: PreviewModel::Rgb,
            selected_channel: None,
        }
    }
}
