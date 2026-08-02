# TON-010 bundled recipes — Substage 3E3A2 parent review

- Repository/HEAD: `/home/ricperry1/projects/Toniator` at dirty `262c7e8`
- Date: 2026-08-01
- Producer: `desktop_implementer`; parent reviewed the exhaustive non-dispatched
  Curves canonical matrix and parameter coverage manifest.

## Accepted findings

- Named CMYK shared/full-width, CMYK per-channel/manual motif, RGB automatic
  motif with alpha source, legacy Crosshatch assignment, and response-boundary
  cases compare complete `CanonicalPatternOutput::Paths` structures between
  retained and recipe orchestration, including dimensions, layer order,
  semantic channels, colors, opacity, outlines, commands, and coordinates.
- Each case executes twice and compares deterministic output. The matrix covers
  every render-effective recipe parameter, including distinct resolution grids,
  custom cubic assets, closure/smoothing, coverage, transforms, response
  bounds, colors, opacity, and RGB/CMYK ordering.
- The checked 38-parameter manifest matches the current descriptor contract.
  `tile_spacing` and `show_background` are intentionally absent and have a
  separate retained/recipe no-op proof.
- A pathological manual motif case rejects native expansion before allocation;
  native resource limits are not weakened to accommodate legacy extremes.
- Production dispatch and preview/export consumers remain unchanged.

## Verification

- Parent coverage manifest, complete matrix, no-op, and pathological-expansion
  tests each passed individually.
- Writer full suite passed: 240 library and 48 binary/UI tests, locked release
  check, strict all-target Clippy, format, and diff checks.
- Writer launch smoke used an intentional timeout and is not human Stage 5
  GNOME/Wayland, Krita-reference, or Inkscape acceptance.

This closes canonical non-dispatched Curves parity. Invalidate when recipe
parameters, retained geometry, pipeline assignment/order, resource policy,
consumer routing, live dispatch, HEAD, or relevant dirty state changes.
