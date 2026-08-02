# TON-010 bundled recipes — Substage 3E3A2 implementation evidence

Date: 2026-08-01
Repository: `/home/ricperry1/projects/Toniator`
Git HEAD inspected: `262c7e857446ded100d4a90fd23d651e52460665`
Producing agent: `desktop-implementer`

## Scope completed

Added exhaustive, named retained-vs-recipe canonical-output evidence for the
non-dispatched Curves orchestrator. The tests compare complete
`CanonicalPatternOutput::Paths` values directly, not hashes or rasterized
approximations, and run each recipe case twice to prove deterministic repeats.

- `cmyk-shared-full-response-grid` covers modern CMYK, opaque prepared source,
  shared authored cubic SVG asset, full-width layout, artboard dimensions,
  density, mark range, and distinct output grids.
- `cmyk-per-channel-manual-motif` covers modern CMYK with alpha prepared
  source; per-channel authored cubic assets; all close/smooth combinations;
  manual coverage; bleed, tile/stack counts, angles, offsets and spacing; all
  alternate transforms; pivots, rotation, offsets, response controls, colors,
  opacity, quality, and distinct field grids/cache keys.
- `rgb-auto-motif-alpha` covers modern RGB semantic ordering, RGB colors and
  opacity, alpha source treatment, automatic coverage, bleed, and distinct
  resolutions.
- `legacy-crosshatch-external-color` covers legacy compatibility assignment
  and the external Crosshatch monochrome color policy.
- `response-boundaries-zero-width` exercises bounded small artboards and
  accepted zero-size/scale and maximum threshold response boundaries.

A checked descriptor manifest maps every accepted Curves recipe parameter id
to one or more named matrix cases. A separate no-op test proves that retained
and recipe canonical Paths are unchanged by `show_background` and
`tile_spacing`, which intentionally remain outside the recipe contract. A
separate pathological manual-motif case proves recipe-only expansion limits
reject before allocation.

## Exact files changed for this substage

- `src/curves_native.rs` (test-only matrix, manifest, no-op, and resource-limit evidence)
- `.codex-work/agents/desktop-implementer/ton-010-bundled-recipes-substage-3e3a2.md`

No production dispatch, consumer, persistence, preset, UI, export, or
retained-renderer behavior changed. Shared dirty/untracked Stage 5 work was
preserved.

## Important decisions and reused abstractions

- Reused `execute_bundled_curves_recipe_cancellable` as the candidate and
  `generate_curve_geometry_for_pipeline` only as the retained oracle,
  wrapping the latter in the same canonical Paths output before exact equality.
- Reused `PreparedSource`, `legacy_pipeline_from_facade`, authoritative Curves
  settings, standard CMYK/RGB channel ordering, direct `CurvePath` cubic
  assets, and existing orchestration/provider instrumentation.
- The manifest enumerates the 38 recipe descriptor ids accepted by
  `adapt_curves_settings_to_recipe`; it deliberately excludes `tile_spacing`
  and `show_background` because their established retained no-op status is
  tested separately rather than misrepresenting them as recipe inputs.
- Exact output equality covers geometry dimensions, semantic layer order,
  colors/opacity, outlines, coordinates, and channel identity. The existing
  requested-field-cache instrumentation remains exercised by its prior
  orchestration test; the matrix deliberately varies resolution grids and
  semantic channel order under the same provider contract.

## Verification and artifacts

- Focused matrix checks passed:
  - descriptor coverage manifest
  - complete canonical CMYK/RGB/Crosshatch/boundary matrix
  - retained no-op proof
  - pathological expansion rejection before allocation
- `cargo test --locked` — 240 library tests and 48 binary/UI tests passed.
- `cargo check --locked --release` — passed.
- `cargo clippy --locked --all-targets -- -D warnings` — passed.
- `cargo fmt --check` and `git diff --check` — passed.
- `timeout 12s cargo run --locked` — built and launched `target/debug/toniator`
  without a startup failure.

No screenshot, preview/PNG/SVG export artifact, or live consumer parity
artifact was generated: Curves production dispatch remains intentionally on
the retained pipeline, and this substage modifies tests only. The startup smoke
is not human GNOME/Wayland acceptance.

## Limitations, review targets, documentation, and invalidation

- This proves only the non-dispatched orchestrator against the retained
  geometry oracle. Full document dispatch, preview/PNG/SVG parity through
  consumers, cache installation, retained-code removal, persistence/schema,
  presets, UI, and Stage 6 remain out of scope.
- Exact equality confirms current retained semantics; it does not make the
  recipe public/live. Future dispatch work must retain the explicit live
  retained-pipeline guard until the dispatch boundary is intentionally changed.
- Future review should re-run the manifest and matrix whenever Curves adapter
  parameter ids, native ports, retained geometry, pipeline ordering, prepared
  source semantics, source-field cache keys, cancellation behavior, resource
  caps, or no-op ownership change. It should also add actual preview/PNG/SVG
  artifacts at the consumer-dispatch substage.
- Durable documentation likely affected: the Stage 5 architecture map and
  recipe-contract material, but reconciliation is deferred to the
  documentation-maintainer milestone review.

## Working-tree assumptions

The repository was materially dirty on the HEAD above from accepted TON-010
work and unrelated user edits. Those changes were neither reverted nor staged.
No reset, clean, commit, push, publication, deployment, or destructive
operation was performed. This evidence is invalid if HEAD or those
working-tree assumptions change.
