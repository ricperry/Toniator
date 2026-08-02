# TON-010 Pattern editor ownership and preview — 2026-08-01

## Scope

Follow-up to the pattern-editor channel-boundary review. The user required the
editor to expose pattern construction only, keep channel styling and site
sampling controls in the main inspector, move curve authoring into the editor,
and show the constructed pattern before Apply.

## Implemented

- Removed the pattern-editor mark selector. Mark shape remains authoritative in
  Channel Settings and the custom recipe no longer overwrites it on Apply.
- Kept sampler, random seed, and weight influence out of the editor. Those
  controls remain in the per-channel/aggregate Channel Settings surfaces.
- Added a neutral live `Pattern Preview` drawing area. It renders deterministic
  structure only (tracks/points, spacing, curve modes/functions, point
  definition, and jitter); it does not read source pixels or channel styling.
- Added a local Bézier curve-shape editor with drag, keyboard nudge, double-click
  split, delete-anchor, and reset interactions. Draft edits are isolated until
  Apply; Cancel does not mutate the document.
- Persisted the authored curve path as a bounded recipe text parameter and use
  its authored bend when the native Shapes operation consumes numeric curve
  parameters. Reopening the editor therefore restores the saved draft values.
- Hid the legacy main-window curve authoring widgets while retaining their
  document/runtime compatibility wiring for existing curve treatments.
- Renamed ambiguous choices to `Curved Track` and `Full Curve Sampling · Marks`
  so the current renderer is not presented as emitting connected strokes.

## Review handoff

The request was passed to both the UX reviewer and creative-output reviewer.
Both agreed that the modal needed a pattern-only preview, that marks and source
response belong to channel controls, and that the prior “connected curves” copy
over-promised the current marks renderer. The UX reviewer recommended keeping
the old curve canvas channel-scoped; the user’s explicit direction to move it
into Pattern Editor took precedence, so the implementation uses a local,
Cancel-safe editor instead.

## Verification

- `cargo test --locked --no-fail-fast`: 251 library, 53 binary/UI, 0 doc tests.
- `cargo clippy --locked --all-targets -- -D warnings`: passed.
- `cargo check --locked --release`: passed.
- `cargo fmt --all -- --check` and `git diff --check`: passed.
- Real GTK demo launch and screenshot completed without Adwaita critical output:
  `/tmp/toniator-ton010-pattern-editor-preview-20260801.png`.

GNOME/Wayland manual interaction of the new modal (including screen-reader
focus traversal and drag gestures) remains a human acceptance step.
