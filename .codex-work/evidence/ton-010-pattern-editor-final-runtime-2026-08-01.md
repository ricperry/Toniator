# TON-010 pattern editor final runtime check

Date: 2026-08-01
Repository: `/home/ricperry1/projects/Toniator`
Git HEAD: `262c7e857446ded100d4a90fd23d651e52460665` (dirty worktree preserved)

## Automated/runtime evidence

- `cargo test --locked`: 249 library tests, 50 binary/UI tests, and doc tests
  passed.
- `cargo clippy --locked --all-targets -- -D warnings` passed.
- `cargo fmt --all -- --check`, `git diff --check`, and
  `cargo check --locked --release` passed.
- `timeout 20s cargo run --locked -- --demo --show-controls --screenshot
  /tmp/toniator-ton010-editor-controls-final-20260801.png` launched the real
  GTK application and wrote a 879265-byte window capture. The rendered window
  visibly exposes the Treatment Settings `Edit Pattern…` action beside the
  Shapes/Curves/Weighted Voronoi selector and renders the production halftone
  canvas.
- The focused editor test proves draft creation is non-mutating until Apply,
  Apply installs non-empty canonical marks through one undo edit, and undo
  restores the prior document.

## Manual acceptance still required

No human GNOME/Wayland acceptance is claimed. The user should open the demo,
expand Treatment Settings, activate `Edit Pattern…`, change name/mark/density/
spacing, choose `Apply`, and verify the custom summary and canvas update; then
try Cancel and Ctrl+Z. Pointer, keyboard, focus, narrow-layout, and
screen-reader checks remain the final manual gate.
