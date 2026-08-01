# TON-010 Stage 5 Framework Restart — Substage A parent review

- Repository: `/home/ricperry1/projects/Toniator`
- Timestamp: `2026-08-01T08:06:21-04:00`
- Git HEAD: `54a8e37d2433781eb4b11f1aa2e4cc989de385be`
- Branch: `TON-010-Stage5-Framework-Restart`
- Preservation tag: `TON-010-stage5-framework-pre-compositor-fix` at `1051a6d2`
- Producing agent: `desktop_implementer` (`019fbd35-0140-7b51-ae65-fc5bbbe13d0c`)
- Parent review: inspected the worker diff and implementation evidence; no
  changes to `src/site_distribution.rs` or `src/voronoi_geometry.rs`.

## Scope

Substage A corrected the Weighted Voronoi canonical producer only. Raster
composition and SVG serializer changes were intentionally not started.

## Verified findings

- `generate_weighted_voronoi_cancellable` emits exactly one
  `FilledRegion` per visible cell, using the existing
  `inset_clipped_cell_for_response(...)` polygon.
- Final regions are `GeometryPolarity::Positive`, `FillRule::NonZero`, and
  contain one ring. Raw clipped cells and raw-to-inset subtractive rings are
  absent from the Weighted Voronoi output.
- `WeightedVoronoiCellRelationship`/`relationships` was replaced by
  `WeightedVoronoiCellRegion`/`cell_regions`, preserving channel, site index,
  deterministic region identity, and avoiding a false subtraction relationship.
- General canonical subtraction remains covered by
  `canonical_region_algebra_retains_genuine_subtractive_masks`.
- Parent-visible focused checks reported passing: `cargo fmt --check`,
  `cargo test --locked weighted_voronoi` (7), `cargo test --locked
  pattern::tests` (13), the named genuine-subtraction and canonical preview/
  PNG/SVG tests, `cargo check --locked`, and `git diff --check`.

## Changed files in this substage

- `src/weighted_voronoi.rs`
- `src/lib.rs`
- `src/pattern.rs`
- Worker evidence:
  `.codex-work/agents/desktop-implementer/ton-010-stage5-framework-restart-substage-a-direct-inset.md`

## Preserved unrelated dirty state

`ISSUES.md`, `assets/CMYKexpected.png`, `assets/RGBexpected.png`,
`nextPrompt.md`, and `.codex-work/evidence/ton-010-stage5-manual/` remain
preserved.

## Inference and uncertainty

The direct positive polygons remove the Weighted Voronoi cell-sizing mask
construction at the canonical boundary and are ready for isolated
model-aware channel composition. SVG still needs compound per-channel paths.
No GTK/manual compositing inspection or reference-artifact comparison was
performed in this substage.

## Invalidation conditions

Invalidate this record if the Weighted Voronoi producer, canonical region
algebra, response-inset geometry, renderer/SVG consumers, Git HEAD, or listed
dirty-worktree assumptions change.
