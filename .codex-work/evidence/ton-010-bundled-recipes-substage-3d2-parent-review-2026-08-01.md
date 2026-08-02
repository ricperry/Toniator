# TON-010 bundled recipes — Substage 3D2 parent review

- Repository/HEAD: `/home/ricperry1/projects/Toniator` at dirty `262c7e8`
- Date: 2026-08-01
- Producer: `desktop_implementer`; parent inspected corrected native operation
  bodies, runtime typing, preflight, provider boundary, tests, and evidence.

## Accepted findings

- Six bounded Shapes native stages consume their declared typed inputs and
  parameters; none wraps or dispatches through the whole compatibility
  renderer, which remains the live oracle.
- A generic `RecipeSourceFieldProvider` lets source sampling request the
  recipe-declared lattice dimensions and semantic channel, avoiding a hidden
  Shapes-specific lattice prepass in later orchestration.
- Threshold zero, nonzero Minimum Mark, and channel Maximum Size preserve the
  current operation ordering. Lattice/candidate work is cancellable and capped
  at one million elements with checked extreme-range arithmetic.
- Custom motif execution verifies digest identity and accepts only the bounded
  editable single-path M/L/C/Z SVG subset; unsupported safe SVG fails
  actionably before native node execution.
- Semantic preflight is a trusted registry hook, not a Shapes-ID branch in the
  generic executor. Shapes sample/mapped ports are distinct from Weighted
  field ports, and cross-family wiring is rejected during graph validation.
- Live Shapes dispatch, persistence, presets, UI, distribution, and Voronoi
  algorithms remain unchanged.

## Verification

- Parent focused native, bundled graph, adapter, and live-branch suites passed:
  11 tests.
- Parent locked release check, strict all-target Clippy, format check, and diff
  check passed; generic executor contains no Shapes-ID dispatch and neutral
  distribution/Voronoi service diffs remain empty.
- Writer full suite passed: 200 library and 48 binary/UI tests.

Exhaustive oracle equivalence, enabled-channel orchestration, and live dispatch
remain 3D3 work. Invalidate when Shapes grid/mapping/primitive/transform/output
semantics, runtime port types, source provider, native preflight, resource
limits, custom SVG subset, HEAD, or dirty state changes.
