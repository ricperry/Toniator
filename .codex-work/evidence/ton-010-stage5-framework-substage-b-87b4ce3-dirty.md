# CACHE_UPDATE — TON-010 Stage 5 Framework Restart / Substage B

- Repository: `/home/ricperry1/projects/Toniator`
- Timestamp: 2026-07-29
- Git HEAD: `87b4ce37d633181df485728cb903c4ff15b9470a` on `TON-010-Stage5-Framework-Restart`
- Dirty-state assumption: preserved untracked `nextPrompt.md` and
  `ton-010-stage5-pre-framework-preservation-2026-07-29.md`; Substages A/B add
  the neutral modules, `src/weighted_voronoi.rs`, the listed integration edits,
  and Stage 5 evidence/agent records. No unrelated file was reverted.

## Changed implementation boundary

- `src/weighted_voronoi.rs` is the only Weighted Voronoi adapter. It maps
  validated persisted settings and each resolved semantic field to neutral
  distribution requests, clipped geometry, source-response insets, canonical
  positive/subtractive paired regions, relationships, and bounded cache
  metadata. It does not duplicate distribution or clipping algorithms.
- `src/site_distribution.rs` remains the centralized owner of max-site/candidate
  limits and seeded shared/independent candidate placement. `src/voronoi_geometry.rs`
  remains the geometry owner; `inset_clipped_cell_for_response` now owns the
  neutral response-inset calculation.
- `src/model.rs` adds typed, strict Weighted Voronoi channel settings under
  `Document.pattern_state`. `RenderVariant::WeightedVoronoiCanonicalV1` is a
  derived dispatch marker only. The registered generator is version 2; version
  1 is rejected. The document/preset format versions remain unchanged.
- `src/pattern.rs`, `src/render.rs`, `src/preset.rs`, `src/png_export.rs`, and
  `src/svg_export.rs` register/validate/route the pattern through the existing
  canonical output path. The minimal `src/ui.rs` changes only cover exhaustive
  dimension/name matches; no Weighted Voronoi editor UI was added.

## Verified findings

- Enabled resolved fields are generated per semantic channel; strong RGB source
  fields produce distinct weighted geometry fingerprints.
- Uniform distribution ignores source field values; shared uniform requests use
  the same ordered arrangement, while independent requests include channel
  identity and differ.
- Region pairs explicitly relate one positive raw cell to its following
  even-odd subtractive seam. Artboard supports remain omitted from seam insets.
- Preview and canonical PNG pixel output matched in the focused test; SVG is
  produced from the same canonical regions and the test verifies even-odd seam
  geometry without a perimeter stroke.
- Persistence save/reopen, model undo/redo, treatment/channel/complete presets,
  strict old generator-version rejection, and current bundled preset
  applicability passed focused coverage.

## Commands and artifacts

- Passed `cargo fmt --check`, `cargo check --locked`, and `git diff --check`.
- Passed `cargo test --locked site_distribution` (5 tests),
  `cargo test --locked voronoi_geometry` (4),
  `cargo test --locked weighted_voronoi` (5), and
  `cargo test --locked every_runtime_bundled_preset_is_current_and_applicable` (1).
- No screenshots or external export artifacts were created; this substage used
  canonical byte/pixel assertions and no new UI workflow.

## Uncertainty and invalidation

- No interactive Weighted Voronoi inspector is implemented; the registry/model
  integration is programmatic until a later UI handoff.
- SVG parity is structural/canonical in this substage; a later graphical review
  should rasterize/inspect representative CMYK and RGB exports.
- Revalidate after changes to `site_distribution.rs`, `voronoi_geometry.rs`,
  `weighted_voronoi.rs`, model/pattern/persistence/preset/render/export paths,
  `cancel.rs`, or the HEAD/dirty-state assumptions. Cache reuse boundaries are
  source generation, resolved field generation, distribution fingerprint,
  geometry fingerprint, channel identity, and view-only canonical consumption.
