# TON-010 Stage 5 Framework Restart — Substage C canonical semantic SVG

- Repository: `/home/ricperry1/projects/Toniator`
- Timestamp: `2026-08-01T08:24:19-04:00`
- Git HEAD: `54a8e37d2433781eb4b11f1aa2e4cc989de385be` on `TON-010-Stage5-Framework-Restart`.
- Producing agent: `desktop-implementer`.
- Task: TON-010 Stage 5 Framework Restart, Substage C only — serialize direct-positive semantic regions as editable canonical SVG compound paths.
- Working-tree assumption: preserved dirty state includes `ISSUES.md`, expected-image assets, `nextPrompt.md`, manual evidence, accepted Substage A/B source/evidence, and their parent review entries. This substage modifies only `src/svg_export.rs` and adds this evidence entry. `src/render.rs` remains dirty only from accepted Substage B and was not edited by this substage; PNG export, UI, persistence, presets, site distribution, Voronoi geometry, and Shapes/Curves generation were not modified.

## Implementation decisions and reused abstractions

- `is_model_aware_semantic_region_output` detects region sets whose layers have same-model `OutputChannelId` identities. `write_region_layers_svg` serializes a semantic layer's positive regions as one compound `<path>` when all have the same fill rule and transform, which covers direct-positive Weighted Voronoi output.
- The compound path contains one subpath for each final visible cell, so Inkscape Path -> Break Apart recovers cells. It uses a deterministic layer ID ending in `-regions`, `fill-rule="nonzero"`, and the existing semantic layer label/color/screen-or-multiply blending.
- The artboard clip remains on each layer because canonical region coordinates may legally be outside the artboard; it is a domain/page constraint, not a cell-sizing operation. No clipping mask was introduced.
- Real `GeometryPolarity::Subtractive` regions retain their existing per-layer SVG mask. Semantic layers can still compound compatible positive geometry while that local mask applies genuine subtraction.
- `export_svg_cancellable` now supplies `Document.appearance.export_background` to a private canonical serializer helper. Public `canonical_pattern_svg_bytes*` remain transparent synthetic-output helpers. Preview Surface remains absent from SVG.

## Exact changed files and symbols

- `src/svg_export.rs`: `canonical_pattern_svg_bytes_with_background_cancellable`, `is_model_aware_semantic_region_output`, compound serialization in `write_region_layers_svg`, `compound_region_path_data`, canonical export-background route, `weighted_rgb_document`, `weighted_svg_uses_one_compound_final_path_per_semantic_channel_without_masks`, and `genuine_semantic_region_subtraction_retains_a_layer_local_svg_mask`.
- `.codex-work/agents/desktop-implementer/ton-010-stage5-framework-restart-substage-c-canonical-svg.md`: this implementation evidence.

## Verified findings and structural fixture counts

- Deterministic 48x32 Weighted RGB fixture: uniform six cells per Red/Green/Blue channel, response strength zero, boundary gap 0.5, transparent export. Before this serializer change the direct-positive output would have emitted 18 individual positive paths (six per layer); after it emits 3 positive compound paths, 18 final `M` subpaths, 3 named Inkscape groups, 0 masks, and 2,412 SVG bytes. No pre-change byte measurement was materialized; the path/object count derives from the same fixture's canonical regions.
- The matching CMYK fixture emits four multiply-blended compound paths and zero masks. RGB emits three screen-blended compound paths and zero masks.
- Weighted direct-positive SVG has no `-region-` per-cell paths, no even-odd cell-sizing paths, no subtraction mask, and no raw-cell construction geometry. The SVG parses with `usvg` and has mean channel drift at most 2.0 against the model-aware raster fixture.
- A real semantic subtractive fixture still emits one layer-local mask and an editable compound positive path. Generic existing canonical subtraction coverage remains green.
- Canonical document SVG exports with `ExportBackground::None` omit the background layer; a configured color creates the named bottom background layer. Preview Surface does not enter SVG output.

## Commands and results

- `cargo fmt --check` — passed.
- `cargo clippy --locked --all-targets --all-features -- -D warnings` — passed.
- `cargo test --locked svg_export` — passed: 14 library tests, 0 failures.
- `cargo test --locked weighted_voronoi` — passed: 7 library tests, 0 failures.
- `cargo test --locked render::tests` — passed: 40 library tests, 0 failures.
- `cargo test --locked png_export` — passed: 7 library tests, 0 failures.
- `cargo check --locked` and `git diff --check` — passed.

## Artifacts, limitations, and follow-up review

- No GTK/manual screenshot or exported fixture file was retained; the reproducible fixture is the in-source `weighted_rgb_document` helper and test. The 2,412-byte measurement was observed through a temporary diagnostic removed before final validation.
- SVG/raster parity is automated on the deterministic Weighted RGB fixture with mean per-byte drift <= 2.0. Manual Inkscape Break Apart and the supplied Krita-reference visual comparison remain unperformed.
- Documentation likely affected after milestone review: Stage 5 framework/architecture/manual acceptance records should state direct-positive compound semantic SVG geometry, clip rationale, and canonical export-background handling. No durable documentation was changed.
- Invalidate this evidence if `src/svg_export.rs`, canonical region/layer/polarity definitions, render compositor, Weighted producer, export-background behavior, Git HEAD, or the listed dirty-worktree assumptions change.
