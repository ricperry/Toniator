# TON-010 Stage 3 canonical output evidence

Date: 2026-07-28
Worktree: `/home/ricperry1/projects/Toniator`
Base checkpoint: `f9c138c493a9d687b5300abddf14e78281f2ad63`

## Scope completed

- Preserved `MarkSet` and `CurveGeometry` as the exact existing Marks/Paths
  canonical forms.
- Added typed canonical filled regions/cells and shared boundary networks,
  with a typed regions-plus-network composite for future cell patterns.
- Defined Y-down artboard coordinates, bounds clipping at each consumer,
  affine transforms, ring winding, fill rules, layer/channel identity,
  deterministic ordering, opacity/blend behavior, polarity, bounded limits,
  and cancellation checkpoints.
- Routed document preview, PNG, and SVG through one canonical generation seam;
  added direct canonical PNG encoding for fixture/future generator parity.
- Added editable SVG groups, compound paths/fill rules, and explicit masks for
  subtractive geometry. Subtraction removes alpha and is never a background
  colored stroke.
- Added synthetic fixtures for filled cells, holes, shared topology, negative
  space, clipping, transforms, transparency, ordering, cancellation, and
  raster/PNG/SVG parity.

## Verification

- `cargo fmt --all`
- `cargo test --locked` — 136 library tests and 46 binary/UI tests passed.
- `cargo clippy --locked --all-targets -- -D warnings`
- `git diff --check`

Weighted Voronoi, schema-driven UI, and the custom-pattern ecosystem were not
implemented in this stage.
