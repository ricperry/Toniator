# TON-010 bundled recipes — Substage 3C parent review

- Repository/HEAD: `/home/ricperry1/projects/Toniator` at dirty `262c7e8`
- Date: 2026-08-01
- Producer: `desktop_implementer`; parent inspected live dispatch, production
  exports, test-only oracle boundaries, regression tests, and evidence.

## Accepted findings

- The sole live `RenderVariant::WeightedVoronoiCanonicalV1` branch now invokes
  the bundled Weighted recipe executor after the existing semantic-field
  resolution path.
- The former whole-pattern generator and its cache metadata compile only under
  `cfg(test)` as a temporary equivalence oracle; production exports and release
  builds cannot dispatch through it.
- A thread-local test seam proves a real document render enters the recipe once
  and the retained oracle zero times without adding production instrumentation.
- Canonical output remains the single preview, PNG, and SVG authority. The live
  CMYK regression confirms deterministic output, preview/PNG equality, four
  editable multiply layers, and no SVG masks.
- Field resolution, output validation, cancellation flow, site distribution,
  and Voronoi geometry algorithms remain unchanged.

## Verification

- Parent focused live RGB dispatch and CMYK canonical-consumer tests passed.
- Parent Weighted-focused suite passed: 14 tests.
- Parent locked release check, strict all-target Clippy, format check, and diff
  check passed.
- Writer full suite passed: 189 library and 48 binary/UI tests.

Interactive GNOME/Wayland, Krita-reference, and Inkscape Break Apart acceptance
remains pending and is not implied by this automated recipe-dispatch review.

Invalidate when Weighted renderer dispatch, bundled definition/executor,
semantic field resolution, canonical consumers, retained oracle boundary,
HEAD, or dirty state changes.
