# TON-010 pattern editor modal lifecycle fix

Date: 2026-08-01
Repository: `/home/ricperry1/projects/Toniator`
Git HEAD: `262c7e857446ded100d4a90fd23d651e52460665` (dirty worktree preserved)

## Finding

Repeated editor entry after Save As triggered Adwaita criticals because the
Blueprint-owned `pattern_editor_draft` box remained parented by the prior
`AdwAlertDialog`. A later `set_extra_child()` therefore failed, leaving the
modal without usable draft controls.

## Correction

`open_pattern_editor()` now detaches the draft box on every dialog response,
guarded by a parent check. This covers Apply, Save As, and Cancel and allows
the same controls to be attached to a subsequent editor dialog.

## Verification

- `cargo test --locked --bin toniator`: 50 passed.
- `cargo test --locked --lib`: 249 passed.
- `cargo clippy --locked --all-targets -- -D warnings` passed.
- `cargo fmt --all -- --check` and `git diff --check` passed.
- Bounded real GTK `--demo --show-controls --screenshot` launch completed and
  wrote `/tmp/toniator-ton010-editor-parent-fix-20260801.png`.
- The original console warning is specifically addressed; a human should
  still repeat Save As, return to the main window, and reopen the editor on
  GNOME/Wayland to confirm the absence of warnings interactively.
