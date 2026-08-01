# TON-010 Stage 5 Framework Restart — Substage A direct inset producer correction

- Repository: `/home/ricperry1/projects/Toniator`
- Timestamp: `2026-08-01T08:06:21-04:00`
- Git HEAD: `54a8e37d2433781eb4b11f1aa2e4cc989de385be` on `TON-010-Stage5-Framework-Restart`
- Working-tree assumption: pre-existing dirty files were `ISSUES.md`, `assets/CMYKexpected.png`, `assets/RGBexpected.png`, `nextPrompt.md`, and `.codex-work/evidence/ton-010-stage5-manual/`; all were preserved. This substage additionally modifies `src/lib.rs`, `src/pattern.rs`, and `src/weighted_voronoi.rs`, and adds this evidence entry.
- Producing agent: `desktop-implementer`.
- Task: TON-010 Stage 5 Framework Restart, Substage A only — emit the computed boundary-derived response inset as final canonical Weighted Voronoi geometry without altering compositor or SVG serializer implementation.

## Implementation decisions and reused abstractions

- `weighted_voronoi::generate_weighted_voronoi_cancellable` now emits exactly one `FilledRegion` per visible cell: the existing `inset_clipped_cell_for_response(...)` result, `FillRule::NonZero`, and `GeometryPolarity::Positive`.
- It no longer emits the raw clipped cell or a subtractive raw-to-inset construction ring. `WeightedVoronoiCellRelationship`/`relationships` is replaced by `WeightedVoronoiCellRegion`/`cell_regions`, which retains semantic channel, site index, stable deterministic ordering, and the final visible `RegionId` without claiming a subtractive boundary region exists.
- The correction reuses the existing neutral geometry boundary service unchanged: `voronoi_geometry::inset_clipped_cell_for_response`. Neither `src/site_distribution.rs` nor `src/voronoi_geometry.rs` was modified.
- Canonical `GeometryPolarity::Subtractive` remains a valid general-algebra operation; the focused pattern test still validates actual destination-out subtraction separately from Weighted Voronoi.

## Exact changed files and symbols

- `src/weighted_voronoi.rs`: `WeightedVoronoiCellRegion`, `WeightedVoronoiGeneratedOutput::cell_regions`, `generate_weighted_voronoi_cancellable`, `semantic_fields_and_weighted_channels_remain_distinct`, `final_cells_are_positive_boundary_derived_insets_without_construction_masks`, and `canonical_preview_png_svg_share_cells_without_a_perimeter_border`.
- `src/lib.rs`: public re-export changed from `WeightedVoronoiCellRelationship` to `WeightedVoronoiCellRegion`.
- `src/pattern.rs`: renamed the existing direct canonical-algebra coverage to `canonical_region_algebra_retains_genuine_subtractive_masks`.
- `.codex-work/agents/desktop-implementer/ton-010-stage5-framework-restart-substage-a-direct-inset.md`: this implementation evidence.

## Verified findings

- The focused direct-inset test enables only RGB Red with uniform deterministic eight-site placement, `response_strength = 0.0`, and `boundary_gap = 2.0`. It verifies exactly eight positive, single-ring, nonzero-fill regions; zero subtractive output regions; stable Red/site ordering; and metadata-to-region identity.
- The same test reconstructs the distribution and clipped diagram, then proves every emitted ring is exactly the result of `inset_clipped_cell_for_response` and differs from the raw cell polygon. This establishes nonzero-gap, boundary-derived behavior at the producer boundary without a centroid-scale fallback or hidden raw final geometry.
- The canonical preview/PNG/SVG route test passes with no weighted SVG even-odd fill or subtract mask identifier. No raster compositor or SVG serializer source was changed.
- General canonical subtraction remains verified by the pattern algebra test, which preserves a genuine subtractive destination-out region after positive geometry.

## Commands and results

- `cargo fmt --check` — passed.
- `cargo test --locked weighted_voronoi` — passed: 7 library tests, 0 failures; 0 binary/UI tests selected.
- `cargo test --locked canonical_region_algebra_retains_genuine_subtractive_masks` — passed: 1 library test, 0 failures.
- `cargo test --locked canonical_preview_png_svg_share_cells_without_a_perimeter_border` — passed: 1 library test, 0 failures.
- `cargo test --locked pattern::tests` — passed: 13 library tests, 0 failures.
- `cargo check --locked` — passed.
- `git diff --check` — passed.

## Artifacts, limitations, and follow-up review

- No screenshot, GTK launch, manual artifact, PNG export artifact, or SVG export artifact was produced; this is a bounded producer-only correction. The existing automated canonical preview/PNG/SVG route test was run.
- No durable documentation was updated. `docs/TON-010_STAGE_5_FRAMEWORK_RESTART.md` and `docs/TON-010_STAGE_5_ARCHITECTURE_MAP.md` still describe paired positive/subtractive relationships and require later documentation reconciliation after the parent accepts the correction.
- Follow-up review targets: inspect canonical output from multi-channel RGB/CMYK documents in the raster compositor and SVG serializer only in their authorized later substages; confirm the renamed public metadata API is appropriate for any downstream consumer.
- Invalidate this evidence if `src/weighted_voronoi.rs`, `src/pattern.rs`, `src/lib.rs`, `src/voronoi_geometry.rs`, canonical render/SVG consumers, current HEAD, or the listed dirty-worktree assumptions change.
