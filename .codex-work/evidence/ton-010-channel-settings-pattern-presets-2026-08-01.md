# TON-010 Channel Settings and construction preset slice

Date: 2026-08-01
Repository: /home/ricperry1/projects/Toniator
Git HEAD: 262c7e857446ded100d4a90fd23d651e52460665 (dirty worktree preserved)

## Implemented

- Removed the standalone Channel Scope heading/note. The channel/ink selector
  now sits directly below Visible Inks and above the channel HEX entry. Curve
  controls expose the equivalent selector in the same Visible Inks → color
  order.
- Renamed the color control to `Channel HEX value (Color code)`, made the
  Entry explicitly editable/focusable, added activate-time validation, and
  surfaced invalid input instead of silently ignoring it.
- Custom embedded Shapes recipes now receive typed Channel Settings changes in
  their authoritative output-channel instance. Coverage, sampling detail,
  rotation, opacity, color, mark size, and channel site-constructor changes
  therefore reach the production recipe runtime after Apply/Save As.
- Mark width/height are moved into the Advanced expander at runtime; cutoff,
  opacity, and sampling detail remain basic channel controls.
- Replaced the visible legacy pattern-mode buttons with one pattern preset
  selector covering X/Y Grid, Triangular Grid (60°), Sine, Square, Spiral,
  Sawtooth, Uniform/Gaussian/Blue/Pink/Poisson random dispersion, Curves, and
  Weighted Voronoi. Math/random entries install real editable recipes.
- Spiral is a dedicated centered Archimedean path: its preset forces connected
  output, curve spacing controls the turn pitch, and it no longer routes
  through the generic X/Y grid warp. The authored path extends one pitch past
  the farthest rectangular corner so the canonical raster/SVG artboard clip,
  rather than an inscribed-circle cutoff, trims the visible result. Channel
  samplers do not replace Spiral's site topology, and thresholded samples mark
  new path endpoints instead of reconnecting across gaps.
- Added native triangular staggered placement and density-aware editor lattice
  construction. Site density now changes generated population even when X/Y
  spacing values are present; the old preview-only random count cap is gone.
- Added deterministic native tests for triangular axes, site-density response,
  distinct math-function warps, embedded channel propagation, and PNG/SVG
  exports for every math/random preset.
- Added console diagnostics for channel slider edits, site-constructor edits,
  named preset installation, and custom pattern Apply.
- Explicitly clear the AlertDialog extra-child slot when the pattern editor
  closes, so reopening it does not reuse a still-parented Blueprint widget or
  emit Adwaita parent assertions.
- The editor disables triangular-grid-only irrelevant controls (Y spacing,
  grid curve modes, and point-definition selection) and draws its preview with
  the same 60-degree row vector used by production placement.

## Verification

- `cargo test --locked --no-fail-fast`: 256 library tests, 54 binary/UI tests,
  and doc tests passed.
- Export exercise passed for Sine, Square, Spiral, Sawtooth, Uniform,
  Gaussian, Blue Noise, Pink Noise, and Poisson: each generated a non-empty
  PNG and an SVG containing a valid `<svg` root through the production export
  functions.
- `cargo clippy --locked --all-targets -- -D warnings` passed.
- `cargo fmt --all -- --check` and `git diff --check` passed.
- `cargo check --locked --release` passed after the final source changes.
- The triangular placement test verifies the derived inter-row vector is
  exactly 60 degrees from the horizontal axis.
- The focused math/random export exercise and Clippy pass were rerun after
  the final preview/sensitivity changes.
- The complete 256-library/54-binary test suite and release check were rerun
  after the final preview/sensitivity changes; the GTK layout screenshot was
  refreshed at the recorded path.
- Choosing `Custom Pattern` from the pattern selector now opens the editor
  instead of silently doing nothing; the focused realized-GTK, export, full
  suite, Clippy, and release checks were rerun after that fix.
- Focused and full checks now include rectangular-corner spiral coverage,
  sampler-independent ordered path geometry, and a connected-network
  discontinuity regression. The rotated-screen coverage regression also
  verifies retained candidates extend past every artboard edge for clipping;
  the complete suite is 256 library tests.
- GTK demo launch wrote
  `/tmp/toniator-ton010-channel-layout-final-20260801.png`. The realized surface
  shows the selector below Visible Inks, with no standalone Channel Scope
  heading. Human GNOME/Wayland interaction and screen-reader acceptance remain
  pending.
- A fresh artifact launch also completed with
  `/tmp/toniator-ton010-spiral-runtime.png`; the production GTK application
  exited cleanly after writing the actual window capture.
- Custom embedded mark/network recipes now project the active RGB/CMYK model
  into the ordinary channel inspector, including Red/Green/Blue visibility and
  an editable Channel HEX entry. Deferred GTK synchronization is ignored when
  its document/model fingerprint is stale, preventing a prior CMYK callback
  from restoring CMYK labels on an RGB surface.
- Connected RGB recipe layers now declare Screen blending; CMYK connected
  layers retain Multiply. Square-wave deformation expands/contracts each grid
  side around the artboard center rather than translating the whole grid.
- Added regressions for custom RGB channel labels/HEX editing, RGB network
  Screen blending, and symmetric square-wave expansion. `cargo test --locked`
  now passes 258 library tests, 54 binary/UI tests, and doc tests; the focused
  realized GTK test, `cargo clippy --locked --all-targets --all-features
  -- -D warnings`, formatting, and the GTK demo launch also pass.
- Editor-authored rotated X/Y and triangular grid sites are wrapped into the
  artboard after screen-angle rotation; compatibility Shapes retain their
  historical out-of-artboard centers for parity. A focused native test covers
  wrapped editor sites and triangular points now honor the channel rotation,
  while the canonical compatibility oracle remains exact. The final full
  suite passes 258 library tests, 54 binary/UI tests, and doc tests.
- Selecting Curve or Math Function in the editor now defaults the draft to
  connected/full-curve geometry, without rewriting an existing saved draft on
  modal reopen; Grid, Triangular Grid, and Random retain mark-oriented defaults.

## Connected stroke contour regression follow-up

- Added `NetworkStroke` canonical geometry carrying the authored centerline,
  per-sample widths, and one reusable cubic filled outline per contiguous run.
  Existing network nodes/edges remain available for topology inspection, but
  raster and SVG consumers render the smooth contour instead of independently
  stroking every short edge.
- Threshold discontinuities remain hard stroke boundaries; no outline is
  generated across a missing continuation marker. Maze connections insert an
  elbow into the same continuous outline rather than emitting separate caps.
- SVG export emits one editable filled path per positive stroke and one filled
  path per subtractive mask. The old edge fallback is retained only for
  canonical fixtures that do not provide stroke contours.
- Canonical network validation now checks stroke IDs, finite centerlines and
  widths, outline commands, and bounded stroke/command counts.

## Verification (connected contours)

- Focused connected-network test verifies discontinuity splitting, variable
  widths, cubic-only outline commands, and SVG output without per-edge
  `stroke-width` segments.
- `cargo test --locked --no-fail-fast`: 258 library tests, 54 binary/UI tests,
  and doc tests passed.
- `cargo clippy --locked --all-targets --all-features -- -D warnings`,
  `cargo fmt --all`, `cargo check --locked --release`, and `git diff --check`
  passed.

Manual zoomed GNOME/Wayland inspection of the connected spiral and exported SVG
remains a human acceptance step; this pass establishes the shared canonical
geometry and automated parity guard.

## Spiral arc-length follow-up

- Replaced uniform angle/radius spiral sampling with deterministic inversion of
  the Archimedean spiral's closed-form arc-length function. Node spacing is now
  derived from travelled curve distance, avoiding the excessive center density
  caused by uniform radians while preserving corner-overflow clipping.
- Updated the pattern-editor spiral preview to use the same arc-length
  parameterization as production/export geometry.
- Added production and realized-editor regressions for positive centerline
  distances, deterministic arc-length increments, and cubic connected outlines.

Verification after this change: 258 library tests, 54 binary/UI tests, strict
Clippy, release check, formatting/diff checks, and a GTK demo screenshot launch
all passed.

## Invalidation

Invalidate if `src/ui.rs`, `src/model.rs`, `src/shapes_native.rs`, or
`resources/toniator-window.blp` changes without rerunning the focused/full
checks and a GTK launch.

## Random point distribution controls follow-up

Repository: `/home/ricperry1/projects/Toniator`
Git HEAD: `262c7e857446ded100d4a90fd23d651e52460665` with the existing dirty TON-010 worktree preserved.
Producing agent: `/root`
Timestamp: 2026-08-01

### Verified findings

- Added the pattern-scoped `random-size-response` parameter and a Pattern Editor horizontal slider labelled “Random shape size response”. Zero selects a uniform mark extent; one preserves the existing source-responsive size mapping. The control is disabled for non-random placement and round-trips through the custom recipe draft.
- Versioned the editor lattice operation for this new parameter: existing v1 embedded editor recipes remain executable with the default response, while newly authored drafts use v2 and persist the slider value.
- Random placement uses the channel `resolution-scale` to derive its bounded site grid. A focused native test confirms a higher sampling-detail value produces a larger random site population.
- Weighted random site selection no longer receives a second independent cell-sized dispersion offset. A focused high-contrast field test confirms the selected sites remain clustered in the high-value half of the source field.
- Uniform random mark sizing retains threshold-controlled site eligibility and the channel minimum/maximum/max-size bounds, replacing only source-response variation at slider value zero. A focused native transform test confirms equal extents at zero and differentiated extents at one.

### Exact files and symbols

- `src/ui.rs`: `PatternEditorDraft`, `pattern_editor_recipe`, draft loading/modal response, editor widget wiring, sensitivity, and resource-ID tests.
- `resources/toniator-window.blp`: `pattern_editor_random_size_response` slider row.
- `src/pattern_definition.rs`: editor lattice operation descriptor includes `random-size-response`.
- `src/shapes_native.rs`: `ShapesLattice.random_size_response`, `ShapesMappedValues.uniform_extent_factor`, random placement sampling, weighted dispersion guard, mark-size blending, and focused tests.

### Commands run

- `cargo check`
- `cargo test --lib` (261 tests passed, including the three new focused tests)
- `cargo test --bin toniator` (54 tests passed)
- Focused tests: weighted clustering, random sampling-detail population, and uniform-size blending passed.
- `cargo fmt --all`
- `cargo clippy --locked --all-targets --all-features -- -D warnings`
- `cargo check --locked --release`, `cargo test --doc`, `git diff --check`
- GTK artifact launch completed at `/tmp/toniator-ton010-random-controls-final.png`.

### Unresolved uncertainty

- Human GNOME/Wayland inspection of the new slider and visual clustering remains outstanding; automated GTK resource realization and the binary test suite pass.

### Invalidation

Invalidate this entry if `src/ui.rs`, `src/pattern_definition.rs`, `src/shapes_native.rs`, or `resources/toniator-window.blp` changes without rerunning the focused random-placement tests, the complete library/binary suites, and a GTK launch.

## Per-channel weighted random default follow-up

- Random placement now maps the neutral channel `Grid` sampler to
  source-weighted random sites. `Uniform Random` remains an explicit channel
  choice; this makes the uniform mark-size response meaningful by default.
- The production UI-to-recipe test confirms a selected channel's weighted
  random centroid moves toward its high-volume source area relative to the
  explicit uniform sampler.
- The native sampling-detail test confirms the fallback sampler is weighted at
  both sparse and dense resolutions.

Verification after this change: focused weighted-site tests passed. The full
261-library/55-binary suite, strict Clippy, release check, formatting, doc
tests, and GTK artifact launch passed. Latest capture:
`/tmp/toniator-ton010-random-weighted-final.png`.
## Sampling Detail / Triangular Grid follow-up

- The realized GTK callback test now selects one channel, changes `Sampling Detail`, and verifies the typed channel value, embedded output-channel recipe value, and canonical mark population all change together.
- Triangular Grid construction now scales its 60-degree site population from the channel resolution multiplier; the native sampling-detail regression covers both Random and Triangular Grid placement.
- Focused verification: `cargo test --bin toniator ui::tests::realized_numeric_controls_leave_continuous_scroll_to_parent -- --nocapture` and `cargo test --lib editor_random_sampling_detail_changes_site_population -- --nocapture` passed.
- Final verification after the triangular-grid fix: `cargo fmt --all -- --check`, `git diff --check`, `cargo test --all-targets` (261 library + 55 binary/UI), `cargo clippy --locked --all-targets --all-features -- -D warnings`, `cargo check --locked --release`, `cargo test --doc`, and a GTK launch with `/tmp/toniator-ton010-sampling-detail.png` all passed.

## Channel shape-size response and random-preset edit follow-up

- Moved the Shape Size Response control out of the Pattern Editor draft and into the Channel Settings Advanced section. It is now an output-channel parameter, so each ink/channel can blend uniform mark extents (0) with source-responsive size (1) independently.
- Added the output-channel `random-size-response` parameter to the bundled Shapes recipe and to the typed `WebShapeChannel` authority, embedded synchronization, validation, and recipe adaptation. The editor graph still binds the value to the native lattice operation, but the pattern editor no longer owns the control.
- Sampling Detail changes on a newly authored `Random · Gaussian` embedded recipe now update the selected channel's typed and embedded `resolution-scale` values without rejection. A regression test covers the reported 0.9734 value.
- Rejection diagnostics now distinguish pre-validation sync errors, full pattern validation errors, and document-state commit failures instead of reporting only the generic UI message.

### Verification

- `cargo test --all-targets` passed after the channel-scope change (261 library tests and 56 binary/UI tests).
- Focused random preset regression passed: `cargo test random_preset_sampling_detail_updates_embedded_channel_without_rejection -- --nocapture`.
- Realized Blueprint/control regression passed: `cargo test ui::tests::realized_numeric_controls_leave_continuous_scroll_to_parent -- --nocapture`.
- `cargo fmt --all`, `git diff --check`, strict Clippy, locked release check, and locked doc tests passed.
- GTK demo launch completed and produced `/tmp/toniator-ton010-shape-size-final.png`.

### Unresolved uncertainty

- Human GNOME/Wayland inspection of the relocated control and live random Gaussian slider remains outstanding; automated resource realization and the focused production path pass.
