# TON-012 Stage 5 rendering/parity implementation evidence

- Repository absolute path: `/home/ricperry1/projects/Toniator`
- Git HEAD: `4161635d90ee81421ffa1f2dc52e2a381d18c6d7`
- Producing agent: `desktop_implementer`
- Timestamp: 2026-07-26
- Task: bounded Stage 5 output-surface cache, preview/export separation, and
  authoritative Curves SVG parity cleanup.

## Working-tree assumptions

- At start, `.codex-work/cache-index.md` was modified and `AGENTS.md`,
  `.codex-work/backups/`, and the Stage 5 rendering audit were untracked.
  They were preserved and not edited by this implementation.
- No other writer was active. Source changes listed below are this task's
  implementation and verification coverage only.

## Exact files changed

- `src/model.rs`
- `src/render.rs`
- `src/curve_render.rs`
- `src/svg_export.rs`
- `src/png_export.rs`
- `src/persistence.rs`
- `.codex-work/agents/desktop-implementer/ton-012-stage-5-rendering-parity-implementation.md`

## Verified implementation decisions

- `OutputTreatmentCache` now carries an optional, serde-default
  `preview_surface` snapshot. Newly serialized caches include it when present;
  old v6 caches without it load successfully.
- `DocumentAppearance.preview_surface` remains the active model's state.
  `switch_output_mode` snapshots the outgoing active surface into its treatment
  cache and restores the incoming cache surface. Missing snapshots resolve to
  CMYK opaque white or RGB opaque black. `export_background` remains active,
  document-wide export state and is not cached or changed by mode switches.
- The existing `DocumentEditor::set_output_mode` edit boundary remains intact;
  the full appearance/cache transition participates in one undo and redo state.
- Preview composition now uses only checkerboard plus `PreviewSurface`.
  The white-preview fast path depends only on the preview surface. Export
  background remains confined to `render_document_export_cancellable`, PNG
  Document background, and SVG output.
- Curves SVG Crosshatch layer labels now derive from
  `artwork_pipeline.assignment`, not `WebCurveSettings.value_mode`.
- Retained facade-derived Shapes/Curves entrypoints are explicitly marked as
  compatibility adapters. Document render/export paths continue to consume the
  authoritative pipeline entrypoints.

## Existing abstractions reused

- `DocumentAppearance`, `PreviewSurface`, `OutputTreatmentCache`,
  `Document::switch_output_mode`, and `TreatmentState` for persistence and
  undo/redo coverage.
- `composite_preview`, `composite_export_background`,
  `render_document_output`, and `render_document_export_cancellable` for
  separate presentation semantics.
- `generate_curve_geometry_for_pipeline`, `ArtworkPipelineSettings`, and
  `ChannelAssignment::LegacyCompatibility` for semantic Curves SVG behavior.

## Tests and checks

- `cargo fmt --check` — passed.
- Focused Stage 5 library tests — passed: output defaults/cache round trips,
  undo/redo, appearance persistence, old-v6 absent cache snapshot fallback,
  preview/export composition, PNG isolation, Crosshatch labels, and stale
  RGB Curves facade coverage.
- `cargo test --locked --lib` — passed: 116 tests.
- `cargo test --locked --bins` — passed: 43 tests.
- `cargo clippy --locked --all-targets -- -D warnings` — passed.
- `git diff --check` — passed.

## Artifacts and limitations

- No screenshots, exported fixture artifacts, GTK launch, or manual graphical
  verification were produced for this bounded non-UI rendering change.
- SVG and PNG behavior is covered through generated in-test outputs; a future
  milestone review may visually inspect CMYK/RGB preview surfaces in the live
  app if product review requires it.
- Durable documentation likely affected at milestone review: rendering/export
  behavior reference or release notes describing per-output preview surfaces.

## Follow-up review targets and invalidation

- Review any future changes to `DocumentAppearance`, output treatment
  transitions, render preview/export composition, PNG/SVG export, or legacy
  pipeline adapters against these cache and parity tests.
- This evidence is invalidated by changes to the files above, Git HEAD, or the
  recorded working-tree assumptions.
