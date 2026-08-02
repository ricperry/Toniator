# TON-010 bundled recipes — Substage 3E2C parent review

- Repository/HEAD: `/home/ricperry1/projects/Toniator` at dirty `262c7e8`
- Date: 2026-08-01
- Producer: `desktop_implementer`; parent reviewed width modulation, the
  retained/native helper seam, provenance, limits, tests, and dispatch.

## Accepted partial boundary

- `curves.width-modulation` consumes typed deformed paths and source samples,
  rejects placement/grid/artboard provenance or field-dimension mismatches,
  and validates every retained response/simplification parameter.
- Retained and native execution share one
  `modulate_curve_paths_cancellable` implementation for bicubic source
  interpolation, threshold/width response, active segmentation, clipping,
  simplification, and open/closed outline construction.
- `CurvesModulatedPaths` is narrow: artboard plus final `CurveOutline` values.
  It carries no layer, color, opacity, enabled state, `CurveGeometry`, or
  compatibility facade; those remain emit responsibilities.
- Native-only bounds cap paths at 20,000 points, total input at 1,000,000
  points, output at 10,000 outlines and 4,000,000 commands. Counts use checked
  arithmetic, and retained execution supplies no native bounds.
- Generic valid execution now completes the first five nodes and stops only at
  the explicit unavailable emit body. Production retained rendering invokes
  no Curves native operation.

## Verification

- Parent `cargo test --locked curves_native --lib`: 15 passed.
- Parent static dispatch/diff scan passed. Writer full suite passed: 229
  library and 48 binary/UI tests, locked release check, strict all-target
  Clippy, format, and diff checks.
- Writer launch smoke used an intentional timeout and is not human Stage 5
  GNOME/Wayland, Krita-reference, or Inkscape acceptance.

This is a partial 3E2 boundary. Invalidate when retained modulation helpers,
typed Curves provenance/runtime values, response parameters, resource policy,
emission, live dispatch, HEAD, or relevant dirty state change.
