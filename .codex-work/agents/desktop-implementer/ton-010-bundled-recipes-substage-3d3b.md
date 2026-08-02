# TON-010 bundled recipes — Substage 3D3B implementation evidence

Date: 2026-08-01

## Scope and checkout

- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `262c7e857446ded100d4a90fd23d651e52460665`
- Working-tree assumption: the accepted Stage 5/user dirty files already present before this substage remain preserved. This writer changed only `src/render.rs`, `src/lib.rs`, `src/svg_export.rs`, and this evidence entry; adjacent Stage 5 changes in those files remain owned by their accepted substages.
- Task: switch only the live `RenderVariant::WebShapeV1` document branch to the accepted Shapes recipe. Curves, schema/UI, `site_distribution`, and `voronoi_geometry` were not changed.

## Implementation decisions and reused abstractions

- `generate_document_pattern_output_cancellable` now matches `RenderVariant::WebShapeV1 { .. }` only as the temporary family switch, then calls `execute_bundled_shapes_recipe_cancellable` with `canonical.pattern_state.shape_settings()`, the canonical artwork pipeline, prepared source, and cancellation token. It returns that canonical output directly; it does not regenerate Marks or create an export-specific branch.
- Reused the accepted 3D3A executor, `CanonicalPatternOutput::Marks` consumers, canonical PNG renderer, and `mark_svg_bytes_cancellable` seam. Preview, PNG, and editable SVG continue through the single output returned by document generation.
- The retained Shapes whole generator and its placement/primitive helpers are `#[cfg(test)]` only. Its public production re-export was removed because no production code or workspace consumer referenced it; retained direct oracle tests still compile under the test target. `legacy_pipeline_from_facade` remains production code because Curves uses it.
- Crosshatch remains external artwork-pipeline assignment compatibility. `RenderVariant` remains in place as the temporary pattern-family branch.

## Verified findings

- Real automatic CMYK and RGB document renders each invoke recipe orchestration exactly once and invoke the retained Shapes whole generator zero times.
- The live CMYK custom-cubic SVG file export writes the established byte seam exactly once, with retained generator zero and recipe orchestration one after instrumentation reset.
- Existing live RGB preview/PNG/SVG, custom-cubic PNG, cancellation/stale-generation, and 3D3A canonical RGB/CMYK transparent/white parity regressions pass on the switched path. Semantic `screen` and `multiply` SVG blends, transparent versus export-background separation, deterministic encodes, and editable cubic SVG paths remain covered.
- A brief Wayland GTK startup smoke check built and launched `target/debug/toniator`; the eight-second non-interactive timeout terminated it and no process remained. No screenshot was captured because no visual hierarchy or interaction changed.

## Checks and artifacts

- `cargo test --locked live_shapes_document_render_enters_recipe_not_retained_oracle --lib` — 1 passed (two semantic modes).
- `cargo test --locked live_shapes_file_export_writes_the_mark_svg_bytes_seam_unchanged --lib` — 1 passed.
- `cargo test --locked rgb_shapes_keep_channel_independence_through_raster_png_and_svg --lib` — 1 passed.
- `cargo test --locked nonstraight_cubic_png_decodes_to_canonical_preview_pixels --lib` — 1 passed.
- `cargo test --locked cancelled_rgb_shapes_preview_is_discarded_before_stale_install --lib` — 1 passed.
- `cargo test --locked --quiet` — 208 library tests and 48 binary/UI tests passed.
- `cargo check --locked`, `cargo check --locked --release`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo fmt --check`, and `git diff --check` — passed.
- Runtime artifact: transient `target/debug/toniator` startup only; no persisted screenshot, PNG, or SVG artifact.

## Remaining work and invalidation

- 3D3B is complete. Remaining global work is later all-built-in adapter cleanup (including Curves conversion and only then any `RenderVariant`/schema cleanup); it is intentionally out of scope here.
- Follow-up review targets: the document branch must keep sourcing Shapes settings from `PatternDocumentState`, not `RenderVariant`; recipe orchestration must remain once per document generation; the retained whole-generator must remain test-only until its planned removal; and SVG/PNG consumers must continue consuming canonical Marks without rerendering geometry.
- Documentation likely affected at later Stage 5 milestone reconciliation only; no durable user documentation was changed here.
- Invalidate this evidence if Shapes document dispatch, recipe execution/cancellation, Shapes canonical output, retained-generator compilation boundary, PNG/SVG consumer seams, or pattern-state/facade projection changes.
