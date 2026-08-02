# TON-010 bundled recipes — Substage 3D1 parent review

- Repository/HEAD: `/home/ricperry1/projects/Toniator` at dirty `262c7e8`
- Date: 2026-08-01
- Producer: `desktop_implementer`; parent inspected the corrected bundled
  Shapes definition, descriptors, adapter, tests, and evidence.

## Accepted findings

- Immutable `compat.shapes.v1` is parsed through the common strict bundled
  loader and registry and declares six atomic typed stages rather than a
  whole-renderer wrapper.
- All 28 declared parameters appear once in complete Placement, Motif,
  Modulation, Deformation, or Output authoring sections. Lattice resolution,
  grid rotation/pivot, and phase are owned by lattice placement; local mark
  deformation remains in the transform stage.
- The one-way adapter reads authoritative effective Shapes channels without
  duplicating artwork-pipeline selection or the inspector-only base record.
- Editable cubic paths are validated, deterministically projected to safe SVG,
  digest-identified, and added only to a transient derived definition. The
  bundled definition remains immutable and instances carry asset references.
- Crosshatch color is absent from the recipe contract. Its current compatibility
  setting must remain or move with output-assignment state at the later strict
  schema boundary.
- Live Shapes rendering remains on the existing compatibility branch; no native
  Shapes operation body or dispatch conversion is part of this boundary.

## Verification

- Parent bundled Shapes, adapter, malformed-path, and live-branch focused tests
  passed: 5 tests.
- Parent locked release check, strict all-target Clippy, format check, and diff
  check passed; distribution and Voronoi service diffs remain empty.
- Writer full suite passed: 194 library and 48 binary/UI tests.

Invalidate when Shapes settings/rendering semantics, recipe graph/descriptors,
authoring layout, custom-path/SVG validation, Crosshatch assignment authority,
bundled registry, HEAD, or dirty state changes.
