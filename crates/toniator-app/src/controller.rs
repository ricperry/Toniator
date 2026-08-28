//! Typed intent vocabulary at the GTK-to-application boundary.
//!
//! Widgets emit these values; application/controller code dispatches existing
//! `DocumentCommand`, history, lifecycle, preset, and private-draft authority.

use toniator_domain::ChannelId;

use crate::LifecycleAction;

/// Enumerates bounded artist actions without carrying GTK widgets or values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum UiIntent {
    /// Requests one main-history undo transition.
    Undo,
    /// Requests one main-history redo transition.
    Redo,
    /// Requests a lifecycle operation through its existing worker/event route.
    Lifecycle(LifecycleAction),
    /// Selects a stable channel identity rather than a GTK list index.
    SelectChannel(ChannelId),
    /// Opens the isolated private Grid Pattern Editor for the selected channel.
    OpenGridPatternEditor,
    /// Opens the isolated private Mark Editor for the selected channel.
    OpenMarkEditor,
    /// Discards the private Pattern Editor only through its confirmation route.
    DiscardPatternEditor,
}

/// Reduces typed history intents to the one boolean needed by existing history.
///
/// This pure helper has no GTK, document, scheduler, or persistence side
/// effects and makes undo/redo dispatch testable independently of widgets.
pub(crate) const fn history_redo(intent: &UiIntent) -> Option<bool> {
    match intent {
        UiIntent::Undo => Some(false),
        UiIntent::Redo => Some(true),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Keeps only history intents eligible for a history transition.
    #[test]
    fn history_intents_are_atomic_and_do_not_alias_lifecycle_actions() {
        assert_eq!(history_redo(&UiIntent::Undo), Some(false));
        assert_eq!(history_redo(&UiIntent::Redo), Some(true));
        assert_eq!(
            history_redo(&UiIntent::Lifecycle(LifecycleAction::New)),
            None
        );
    }
}
