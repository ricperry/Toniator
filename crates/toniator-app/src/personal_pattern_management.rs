//! Widget-independent policy for personal Pattern management.
//!
//! The GTK surface owns presentation and the `PersonalLibrary` owns filesystem
//! publication.  This module keeps the small decisions shared by those
//! boundaries explicit and testable without creating another Pattern model.

use std::path::Path;

use toniator_patterns::PresetOrigin;

/// Identifies the explicit write operation requested from Pattern management.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PatternSaveAction {
    /// Creates a new personal record from a built-in, current, or new draft.
    AsNew,
    /// Replaces the selected personal record after an exact fingerprint check.
    Changes,
    /// Performs the stable-ID replacement after the GTK confirmation dialog.
    ChangesConfirmed,
    /// Creates a fresh personal record without replacing the selected record.
    Copy,
}

/// Returns the write actions allowed by the captured catalog origin.
///
/// A built-in or current/new draft never receives a stable personal update
/// route.  The personal origin exposes both an explicit stable-ID update and
/// an explicit fresh-ID copy route.
pub(crate) const fn save_actions_for_origin(
    origin: Option<PresetOrigin>,
) -> &'static [PatternSaveAction] {
    match origin {
        Some(PresetOrigin::Personal) => &[PatternSaveAction::Changes, PatternSaveAction::Copy],
        Some(PresetOrigin::BuiltIn) | None => &[PatternSaveAction::AsNew],
    }
}

/// Reports whether a display name is available in a combined catalog.
///
/// Names compare with Unicode lowercase projection, matching the catalog's
/// existing identity policy.  One existing ID may retain its current name
/// during a stable-ID rename or Save Changes operation.
pub(crate) fn name_is_available<'a, I>(names: I, requested: &str, except_id: Option<&str>) -> bool
where
    I: IntoIterator<Item = (&'a str, &'a str)>,
{
    let normalized = requested.to_lowercase();
    !names
        .into_iter()
        .any(|(id, name)| Some(id) != except_id && name.to_lowercase() == normalized)
}

/// Describes why a captured Pattern write cannot use its original target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PatternSaveConflict {
    /// The configured active library root changed while the draft was open.
    RootChanged,
    /// The observed bytes differ from the captured bytes.
    FingerprintChanged,
}

/// Checks that a captured personal target still points at the same active root
/// and exact bytes before a stable-ID write.
///
/// Fresh-ID saves intentionally do not use this check; they remain available as
/// the recovery route after an external edit or root switch.
pub(crate) fn captured_target_is_current(
    captured_root: Option<&Path>,
    current_root: Option<&Path>,
    captured_fingerprint: Option<&str>,
    current_fingerprint: Option<&str>,
) -> Result<(), PatternSaveConflict> {
    if captured_root != current_root {
        return Err(PatternSaveConflict::RootChanged);
    }
    if captured_fingerprint != current_fingerprint {
        return Err(PatternSaveConflict::FingerprintChanged);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Proves origin policy keeps immutable/current drafts on Save as New and
    /// gives personal records both explicit stable and fresh-ID routes.
    #[test]
    fn save_actions_follow_captured_origin() {
        assert_eq!(
            save_actions_for_origin(Some(PresetOrigin::BuiltIn)),
            &[PatternSaveAction::AsNew]
        );
        assert_eq!(save_actions_for_origin(None), &[PatternSaveAction::AsNew]);
        assert_eq!(
            save_actions_for_origin(Some(PresetOrigin::Personal)),
            &[PatternSaveAction::Changes, PatternSaveAction::Copy,]
        );
    }

    /// Proves combined names reject case-only collisions while allowing the
    /// captured stable ID to retain its existing display name.
    #[test]
    fn names_are_case_insensitively_unique_with_stable_id_exception() {
        let names = [
            ("builtin-grid", "Straight Grid Circles"),
            ("user-one", "My Pattern"),
        ];
        assert!(!name_is_available(
            names.iter().copied(),
            "straight grid circles",
            None
        ));
        assert!(!name_is_available(
            names.iter().copied(),
            "MY PATTERN",
            None
        ));
        assert!(name_is_available(
            names.iter().copied(),
            "my pattern",
            Some("user-one")
        ));
        assert!(name_is_available(
            names.iter().copied(),
            "Another Pattern",
            None
        ));
    }

    /// Proves either active-root changes or exact-byte changes fail closed and
    /// leave Save a Copy as the only fresh-ID recovery route.
    #[test]
    fn captured_target_rejects_root_or_fingerprint_changes() {
        let root = Path::new("/tmp/toniator-library");
        let other = Path::new("/tmp/toniator-other-library");
        assert_eq!(
            captured_target_is_current(Some(root), Some(other), Some("first"), Some("first")),
            Err(PatternSaveConflict::RootChanged)
        );
        assert_eq!(
            captured_target_is_current(Some(root), Some(root), Some("first"), Some("second")),
            Err(PatternSaveConflict::FingerprintChanged)
        );
        assert_eq!(
            captured_target_is_current(Some(root), Some(root), Some("first"), Some("first")),
            Ok(())
        );
    }
}
