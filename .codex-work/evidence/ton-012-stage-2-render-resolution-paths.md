# TON-012 Stage 2 render-resolution path evidence

- Repository absolute path: `/home/ricperry1/projects/Toniator`
- Git HEAD: `32022df28e6e746b44fb4f5db4427fd197ee2739`
- Relevant working-tree assumptions: `src/render.rs`, `src/curve_render.rs`,
  `src/svg_export.rs`, `src/model.rs`, and `src/artwork_pipeline.rs` were clean;
  existing dirty files are documentation, issue/configuration guidance, and
  `nextPrompt.txt`.
- Producing agent: `codebase_explorer` Plato, read-only
- Task: map live Stage 2 sampling, separation, alpha, consumer, and export paths
- Verified paths:
  - `render::decode_source` and `decode_svg` own source decode and long-edge
    preparation.
  - `generate_web_shape_marks_for_output_mode` samples via
    `cached_web_samples`/`sample_web_image_cancellable`.
  - `generate_curve_geometry_for_output_mode` resamples through the same sampler
    once per enabled channel.
  - `map_web_pixel` owns current CMYK, RGB, brightness, scalar routing, and
    Crosshatch interpretation.
  - Document preview/output project legacy fields and decode independently;
    SVG Curves independently decodes and generates geometry.
  - `ArtworkPipelineSettings.alpha_policy` is validated and persisted but has no
    runtime effect; Shapes and Curves differ in partial-alpha brightness output.
- Recommended Stage 2 seam: an owned, validated resolved source/channel-field
  representation in `src/artwork_pipeline.rs`, consumed by `render.rs`,
  `curve_render.rs`, and the existing SVG paths without moving pattern geometry.
- Stage 2 decisions for implementation: `Preserve` uses source alpha as one
  coverage mask; `Ignore` samples stored RGB even where alpha is zero and uses
  full source-bounds coverage; `Alpha` samples alpha as content and does not
  multiply it again. `LegacyCurrentV1` preserves current reachable behavior.
- Invalidation: changes to the listed source files, HEAD, or Stage 2 alpha,
  separation, projection, or export semantics.
- Timestamp: `2026-07-26`
