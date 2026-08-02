# TON-010 bundled recipes — Substage 3D3A COMPLETE implementation evidence

Date: 2026-08-01

## Scope

- Repository: `/home/ricperry1/projects/Toniator`; HEAD assumption: `262c7e857446ded100d4a90fd23d651e52460665` with accepted/user/parent dirty work preserved.
- Implements and verifies a production-callable, non-dispatching Shapes recipe orchestrator and its existing PNG/SVG consumers. No live `WebShapeV1` dispatch, adapter removal, schema/preset/UI, Curves, site-distribution, or Voronoi-geometry change occurred.

## Changes

- Exact changed files for this substage: `src/shapes_native.rs`, `src/render.rs`, `src/lib.rs`, `src/svg_export.rs`, and this evidence entry. These Rust files have adjacent accepted Stage 5 work in the shared dirty tree; this substage adds only the non-dispatching Shapes orchestration, its consumer seam, and oracle coverage.
- `src/shapes_native.rs`
  - Adds `execute_bundled_shapes_recipe_cancellable(prepared, settings, pipeline, token)`.
  - Its `ShapesRecipeSourceProvider` resolves semantic fields only when a declared lattice requests `(columns, rows)`, caching repeated dimension requests with the supplied prepared-source generation and cancellation token.
  - It runs only enabled channels, then assembles all legacy/output-model layers in stable compatibility order. Crosshatch color stays at assembly from `crosshatch_color`, outside recipe parameters.
  - Corrects staged rotated-grid coverage by retaining the configured theoretical mapped upper bound, rather than deriving its coverage margin from current sampled values.
- `src/render.rs`
  - Exposes the retained compatibility Shapes generator to crate tests only and proves document dispatch enters it once while recipe orchestration remains zero at this boundary.
- `src/lib.rs`
  - Exposes the production-callable non-dispatching executor.
- `src/svg_export.rs`
  - Refactors the established `export_mark_svg` writer through `mark_svg_bytes_cancellable(mark_set, presentation, token)`. The seam accepts existing canonical Marks plus only title, artboard dimensions, output model, and export background; it neither regenerates geometry nor routes through the regions/network generic SVG helper.
  - `export_mark_svg` atomically writes the exact seam bytes through its prior path.

## Verified findings

- The full-struct canonical oracle matrix compares the orchestrator output with retained `generate_web_shape_marks_for_pipeline` followed by `adapt_legacy_shapes`, then repeats every case for deterministic equality. It exercises automatic RGB and CMYK; `AllChannels` and `ActiveChannel`; opaque and translucent alpha; disabled channels; threshold/min/max/size/color/opacity; transforms; circle, regular polygon, triangle, pentagon, rectangle, and hexagon.
- Shared and independent user-defined cubic paths use nontrivial handles. The matrix asserts that parsed recipe assets emit `ResolvedWebShape::Cubic` and that Cyan and Magenta receive distinct projected cubic starts when their anchors/handles differ.
- Crosshatch asserts every enabled compatibility layer has `crosshatch_color` `(0x34, 0x56, 0x78)`, while the immutable bundled `.tnpattern` bytes contain no `crosshatch-color` parameter.
- Disabled CMYK channels produce no provider/native operation work while retaining four disabled layers in stable order. Test-only instrumentation proves zero native nodes and zero provider cache misses for this case; equal lattice dimensions resolve once and distinct per-channel resolutions resolve separately.
- Deterministic cancellation coverage verifies both a pre-cancelled production executor and a direct recipe/provider seam that cancels during source resolution. The pure executor returns an error before producing a canonical output and has no output-install side effect; rendering-generation stale-output installation remains the outer rendering owner's responsibility.
- The earlier mismatch in rotated coverage was resolved by carrying `maximum_extent_factor` from mark-map into transform; the equality test passes after this correction.
- Existing live document rendering still takes the compatibility branch and records zero recipe-orchestration invocations.
- Recipe-produced Shapes canonical outputs render to pixels exactly equal to decoded `canonical_pattern_png_bytes` for transparent and white backgrounds in both automatic RGB and automatic CMYK modes. PNG bytes repeat deterministically; Preview Surface changes remain display-only while Export Background remains export-only.
- Retained-oracle and recipe MarkSets produce byte-identical SVG through the new seam. The RGB and CMYK/custom-cubic outputs parse with `usvg`, retain enabled semantic layer order and labels, use `screen`/`multiply` blend modes, contain editable paths (including a custom cubic), contain no raster image, preserve the 48 × 32 page/viewBox and existing no-clip mark behavior, and separate transparent output from an explicit Export Background layer.
- The actual `export_svg_cancellable` file bytes equal seam bytes for the still-live compatibility Shapes document; instrumentation confirms compatibility rendering once and recipe orchestration zero times.

## Checks

- `cargo fmt` and `cargo fmt --check` — passed.
- `cargo test --locked canonical_oracle_matrix_exercises_pipeline_shapes_paths_and_channel_settings --lib` — 1 passed.
- `cargo test --locked cancellation_stops_before_or_during_recipe_execution_without_output_installation --lib` — 1 passed.
- `cargo test --locked shapes_native --lib` — 11 focused tests passed.
- `cargo test --locked live_shapes_document_render_remains_on_compatibility_branch --lib` — passed.
- `cargo test --locked svg_export --lib` — 17 focused tests passed, including the three 3D3A consumer-parity regressions.
- `cargo test --locked --quiet` — 208 library tests and 48 binary/UI tests passed.
- `cargo check --locked`, `cargo check --locked --release`, and `cargo clippy --locked --all-targets -- -D warnings` — passed.
- `git diff --check` — passed. No GTK launch, screenshot, persisted PNG/SVG export artifact, or runtime UI inspection was needed or produced: 3D3A deliberately preserves live Shapes dispatch and has no GTK UI change.

## Remaining 3D3A review targets / 3D3B boundary

- 3D3A is complete. Only 3D3B live dispatch remains; it may switch live dispatch only after this parent-reviewed evidence is accepted. The compatibility renderer remains the oracle and live authority until then.
- Documentation likely affected only at milestone reconciliation: TON-010 Stage 5 architecture/progress records, not durable user documentation during this bounded substage.
- Invalidate this evidence if pipeline field resolution/caching, layer ordering/colors, grid coverage, canonical Marks semantics, SVG serialization/presentation metadata, PNG canonical rendering, or recipe dispatch changes.
