# TON-010 bundled recipes — Substage 3B parent review

- Repository/HEAD: `/home/ricperry1/projects/Toniator` at dirty `262c7e8`
- Date: 2026-08-01
- Producer: `desktop_implementer`; parent inspected six native bodies,
  adapters, provenance wrapper, equivalence/cancellation tests, and evidence.

## Accepted findings

- Six static native bodies implement the bundled Weighted stages atomically
  using existing distribution, Voronoi, inset, and canonical authorities; the
  old full generator is not wrapped as one recipe operation.
- `RecipeVoronoiDiagram` pairs the neutral diagram with its exact ordered
  construction sites solely for downstream response sampling. Tessellation
  remains owned by `voronoi_geometry.rs`.
- A strict one-way adapter derives recipe instances from authoritative current
  `pattern_state` Weighted settings, including exact seeds.
- Recipe RGB/CMYK canonical geometry exactly equals the shipping generator for
  shared/independent, uniform/source-weighted, zero/nonzero gap, response, and
  disabled-channel fixtures. Disabled channels are filtered before validation,
  field conversion, or native stages; instrumentation proves zero work.
- Renderer dispatch remains unchanged. Distribution and geometry service files
  have no diff.

## Verification

- Writer full suite: 187 library and 48 binary/UI; check, strict Clippy, fmt,
  and diff checks passed.
- Parent: 13 Weighted-focused and 14 recipe-framework tests passed;
  distribution/geometry diffs are empty and `git diff --check` passes.

Invalidate when Weighted settings/fields, bundled graph/native bodies,
distribution/Voronoi/inset authorities, canonical identity, renderer dispatch,
HEAD, or dirty state changes.
