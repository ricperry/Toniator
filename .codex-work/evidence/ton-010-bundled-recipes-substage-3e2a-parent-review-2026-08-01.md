# TON-010 bundled recipes — Substage 3E2A parent review

- Repository/HEAD: `/home/ricperry1/projects/Toniator` at dirty `262c7e8`
- Date: 2026-08-01
- Producer: `desktop_implementer`; parent reviewed the first three Curves
  native bodies, runtime types, asset preflight, tests, and retained boundary.

## Accepted partial boundary

- `curves.placement`, `curves.source-sample`, and
  `curves.motif-selection` have bounded typed native bodies in the Curves
  registry. Deformation, modulation, and emission remain explicit unavailable
  stubs and whole-recipe Curves execution is not claimed.
- `CurvesPlacement` carries only artboard, grid, and required transform
  scalars; it does not retain a defaulted `WebCurveSettings` or
  `WebCurveChannel` compatibility facade.
- Placement preserves retained grid calculation and rejects source grids over
  one million cells before sampling. Sampling preserves exact RGB/CMYK
  semantic channel identity and validates field dimensions.
- Motif decoding accepts only the adapter-emitted, digest-backed single-path
  SVG subset: exact wrapper, `M`, 1..=64 finite cubic `C` segments, and no
  transforms or alternate structure.
- Native preflight walks actual motif nodes and resolves literal or scoped
  pattern/output-channel bindings. Malformed custom literal and selected CMYK
  assets fail before any native node invocation.
- Production `RenderVariant::WebCurveV1` still calls the retained
  `curve_render::generate_curve_geometry_for_pipeline`; native recipe dispatch
  remains zero.

## Verification

- Parent `cargo test --locked curves_native --lib`: 6 passed.
- Parent static live-dispatch scan found no Curves-native or recipe execution
  reference in `src/render.rs` or `src/curve_render.rs`.
- Parent `git diff --check` passed. Writer full suite passed: 220 library and
  48 binary/UI tests, locked release check, strict all-target Clippy, format,
  and diff checks.
- Writer launch smoke used an intentional timeout and is not human Stage 5
  GNOME/Wayland, Krita-reference, or Inkscape acceptance.

This is a partial 3E2 boundary. Invalidate when Curves runtime types, asset
serialization/decoder/preflight, source-field resolution, retained atomic
helpers, downstream native bodies, live dispatch, HEAD, or dirty state change.
