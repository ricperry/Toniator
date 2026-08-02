# TON-010 Pattern Editor point modes and RGB channel reopen fix

Date: 2026-08-01
Repository: /home/ricperry1/projects/Toniator
Git HEAD: 262c7e857446ded100d4a90fd23d651e52460665 (dirty worktree preserved)

## Verified changes

- `ShapesLattice::requires_explicit_placements` now routes curved grids and
  non-intersection point definitions through the explicit placement path.
- Intersections retain the lattice placement transform; Curve Spacing samples
  each grid track at axis spacing; Full Curves uses a denser deterministic
  sampled track set. All placements are bounded to 8192 points and retain
  source-field mapping, jitter, cancellation, and deterministic ordering.
- Pattern Editor reopen selects the active document output model. The four
  channel controls are labeled for CMYK or RGB at runtime, with the unused RGB
  slot disabled; RGB sampler and seed values are persisted independently for R,
  G, and B.

## Verification commands

- `cargo test --locked --bin toniator` — 52 binary/UI tests passed (including RGB channel persistence).
- `cargo test --locked --lib` — 249 library tests passed.
- `cargo clippy --locked --all-targets -- -D warnings` — passed.
- `cargo check --locked --release` — passed.
- `cargo fmt --all -- --check` and `git diff --check` — passed.
- `timeout 20s cargo run --locked -- --demo --show-controls --screenshot /tmp/toniator-ton010-pattern-editor-final-20260801.png` — real GTK launch completed; screenshot inspected and shows the live Edit Pattern entry point.

## Remaining uncertainty

The curve controls are numeric bend controls rather than a freehand curve
canvas. Full Curves is represented by bounded sampled marks in the Shapes
mark-output recipe, not a continuous canonical path output. Human GNOME/
Wayland modal interaction and screen-reader acceptance remain unclaimed.

## Invalidation

Revalidate if `src/ui.rs`, `src/shapes_native.rs`, the pattern definition
registry, or the tracked editor Blueprint changes, or if the working-tree
authority or output-model behavior changes.
