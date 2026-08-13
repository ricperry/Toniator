//! Immutable artist-facing projections derived from application authority.

use crate::application_model::ApplicationModel;

/// Describes current document/lifecycle presentation without exposing mutation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LifecycleViewModel {
    /// Supplies the window title.
    pub(crate) title: String,
    /// States whether a document exists.
    pub(crate) has_workspace: bool,
    /// States whether current content differs from its savepoint.
    pub(crate) dirty: bool,
}

/// Projects document identity and revision for view-only consumers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DocumentViewModel {
    /// Supplies the artist-facing document name.
    pub(crate) title: String,
    /// Carries the authoritative revision as immutable presentation data.
    pub(crate) revision: Option<u64>,
    /// States whether the document differs from its savepoint.
    pub(crate) dirty: bool,
}

/// Projects the selected channel without surfacing internal identifiers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ChannelViewModel {
    /// Names the selected channel for artist-facing controls.
    pub(crate) name: Option<String>,
    /// States whether a compatible channel is currently selected.
    pub(crate) selected: bool,
    /// Names the active artist-facing pattern without exposing recipe internals.
    pub(crate) active_pattern: Option<String>,
}

/// Projects one stable bundled pattern catalog item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PatternCatalogViewModel {
    /// Shows the artist-facing pattern name.
    pub(crate) name: String,
    /// Shows the concise artist-facing pattern description.
    pub(crate) description: String,
}

/// Projects main-preview synchronization state without retaining pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PreviewViewModel {
    /// States whether a preview request is currently tracked.
    pub(crate) pending: bool,
    /// Identifies the last successfully accepted request for evidence only.
    pub(crate) accepted_ticket: Option<u64>,
}

/// Derives immutable document, channel, and preview projections from the model.
///
/// The function neither touches GTK nor mutates history, lifecycle, scheduler,
/// savepoint, or source state. It deliberately keeps internal channel IDs out
/// of artist-facing strings.
pub(crate) fn project_document(
    model: &ApplicationModel,
    channel_name: Option<String>,
    active_pattern: Option<String>,
) -> (DocumentViewModel, ChannelViewModel, PreviewViewModel) {
    let document = model.workspace.as_ref().map(|workspace| DocumentViewModel {
        title: workspace.display_name.clone(),
        revision: Some(workspace.history.session().revision().0),
        dirty: workspace.is_dirty(),
    });
    (
        document.unwrap_or(DocumentViewModel {
            title: "Toniator".to_owned(),
            revision: None,
            dirty: false,
        }),
        ChannelViewModel {
            selected: model.selected_channel.is_some(),
            name: channel_name,
            active_pattern,
        },
        PreviewViewModel {
            pending: model.preview_coordinator.submission().is_some(),
            accepted_ticket: model.preview_coordinator.last_accepted_ticket(),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Projects an empty application without manufacturing document authority.
    #[test]
    fn empty_model_projects_a_non_dirty_document_and_no_preview_ticket() {
        let model = ApplicationModel::new();
        let (document, channel, preview) = project_document(&model, None, None);
        assert_eq!(document.title, "Toniator");
        assert_eq!(document.revision, None);
        assert!(!document.dirty);
        assert!(!channel.selected);
        assert_eq!(channel.name, None);
        assert_eq!(channel.active_pattern, None);
        assert!(!preview.pending);
        assert_eq!(preview.accepted_ticket, None);
    }
}
