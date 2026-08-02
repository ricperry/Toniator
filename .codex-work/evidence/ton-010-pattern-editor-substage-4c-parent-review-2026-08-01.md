# TON-010 pattern editor — Substage 4C parent review

Date: 2026-08-01
Repository: `/home/ricperry1/projects/Toniator`
Git HEAD: `262c7e857446ded100d4a90fd23d651e52460665` (dirty worktree preserved)

## Accepted user-critical flow

The existing treatment inspector now exposes an accessible `Edit Pattern…`
action. Its modal draft edits a name, mark choice, density, and spacing without
mutating the document. Apply derives and validates a project-embedded custom
Shapes definition/instance, installs it through `DocumentEditor` as one undo
entry, clears the rendered cache, queues autosave, and requests the normal
cancellable production preview. Save As writes an atomic `.tnpattern` under
the XDG user pattern directory (or selected path) before applying the same
validated definition. A dedicated custom-pattern panel keeps the selected
pattern out of the Legacy/native presentation.

## Parent checks

- `ui::tests::pattern_editor_draft_is_nonmutating_until_one_install_edit`
  passed and proves non-empty canonical marks after install plus undo.
- `ui::tests::custom_pattern_paths_follow_xdg_data_home_and_tnpattern_extension`
  passed.
- Blueprint resource contract passed, including the five-panel treatment
  stack and editor controls.
- `cargo test --locked --lib` passed (249 tests).
- `cargo test --locked --bin toniator` passed (50 tests).
- `cargo clippy --locked --all-targets -- -D warnings` passed.
- Release/all-target checks, format, diff, Blueprint compilation, and bounded
  startup smoke passed.

## Limits

This is the minimum guided editor flow required for entering, creating, and
applying a rudimentary custom pattern. It intentionally does not yet provide
per-keystroke draft preview, graph editing, arbitrary recipe-family editing,
library import conflict resolution, or dedicated custom PNG/SVG parity
fixtures. Manual GNOME/Wayland interaction and reference-artifact acceptance
remain pending; no human acceptance is claimed.

## Invalidation

Invalidate if the editor action/modal, Blueprint object contract, custom recipe
construction, `DocumentEditor` installation, preview/autosave lifecycle, XDG
pattern path, or dirty-checkout assumptions change.
