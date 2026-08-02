# TON-010 pattern construction controls and custom channel surface

Date: 2026-08-01
Repository: /home/ricperry1/projects/Toniator
Git HEAD: 262c7e857446ded100d4a90fd23d651e52460665 (dirty worktree preserved)

## Implemented

- The Pattern Editor exposes the pattern-level construction vocabulary:
  placement (Grid, Curve, Random, Math Function), X/Y grid mode and spacing,
  curved-track bend/function (Sine, Square, Spiral, Sawtooth), curve spacing,
  random dispersion family, point definition, points versus connected preview,
  linear versus maze connection choice, jitter factor, and a local curve-shape
  editor. Values are persisted in the embedded recipe instance and restored on
  reopen.
- The editor lattice graph now passes placement strategy, curve spacing, and
  random dispersion into the versioned native operation; a deterministic test
  proves switching from a curved grid to random/Poisson placement changes the
  generated canvas output.
- Mode-sensitive controls stay visible but are disabled when their value cannot
  affect the selected placement. Channel sampler, source-weight influence,
  random seeds, mark shape, fill, opacity, and rotations remain outside the
  modal in Channel Settings.
- Custom embedded recipes now select the normal channel styling stack rather
  than a summary-only page. This keeps density/fill/rotation/opacity/mark and
  the unified/per-channel sampler and seed controls reachable after Apply or
  Save As.
- Connected Points now selects a bounded native network-emission operation;
  Linear and Maze connection modes produce canonical Network output with
  deterministic nodes/edges for canvas, PNG, and SVG consumers.
- Sawtooth values are parsed on reopen, and the realized GTK regression installs
  a custom recipe, switches to a channel scope, and verifies the channel page
  and per-channel sampler remain sensitive.

## Verification

- `cargo test --locked --no-fail-fast`: 251 library tests, 53 binary/UI tests,
  and doc tests passed.
- Focused realized GTK regression for custom channel controls passed.
- `cargo clippy --locked --all-targets -- -D warnings` passed.
- `cargo check --locked --release`, `cargo fmt --all -- --check`, and
  `git diff --check` passed.
- GTK demo launch with `--demo --show-controls --screenshot` wrote
  `/tmp/toniator-ton010-channel-controls-custom-fixed-20260801.png`; the
  normal launch completed without Adwaita critical output. The screenshot
  capture path emitted one environment-side GDK frame-clock critical while
  taking the artifact.

## Unresolved boundary

Human GNOME/Wayland modal interaction, screen-reader traversal, and reference
canvas acceptance remain pending.

## Invalidation

Invalidate if `src/ui.rs`, `resources/toniator-window.blp`, or
`src/shapes_native.rs` changes without rerunning the focused/full checks.
