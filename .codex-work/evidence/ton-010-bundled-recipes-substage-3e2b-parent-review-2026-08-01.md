# TON-010 bundled recipes — Substage 3E2B parent review

- Repository/HEAD: `/home/ricperry1/projects/Toniator` at dirty `262c7e8`
- Date: 2026-08-01
- Producer: `desktop_implementer`; parent reviewed Curves deformation, shared
  retained helper extraction, limits, tests, and retained dispatch.

## Accepted partial boundary

- `curves.deformation` consumes typed placement and motif values plus every
  declared sampling, coverage, maximum-width guard, and transform parameter.
  It returns only placement provenance, repeated point paths, and the close/open
  decision needed downstream.
- Retained rendering and the native operation both call the same borrowed
  `deform_curve_paths_cancellable` seam. Full-width and motif sampling,
  baseline/repetition, normalization, automatic/manual coverage, tiling,
  transforms, and resampling math were not duplicated.
- Compatibility settings/channel facades are constructed only as temporary
  helper arguments and never stored in recipe runtime values.
- Native-only bounds reject more than 10,000 paths, 20,000 points per path, or
  1,000,000 total points. Checked path/count products are validated before the
  corresponding full-width, tile-chain, row, or resampling expansion.
- Cancellation remains checked around native execution and inside retained
  repeat/row/tile loops. The first three native bodies remain working;
  modulation and emission remain explicit unavailable stubs.
- Production Curves rendering still uses the retained whole pipeline and does
  not invoke the native registry.

## Verification

- Parent `cargo test --locked curves_native --lib`: 11 passed.
- Parent static dispatch/diff scan passed. Writer full suite passed: 225
  library and 48 binary/UI tests, locked release check, strict all-target
  Clippy, format, and diff checks.
- Writer launch smoke used an intentional timeout and is not human Stage 5
  GNOME/Wayland, Krita-reference, or Inkscape acceptance.

This is a partial 3E2 boundary. Invalidate when retained deformation helpers,
Curves placement/motif/runtime values, coverage parameter bindings, limits,
downstream native bodies, live dispatch, HEAD, or relevant dirty state change.
