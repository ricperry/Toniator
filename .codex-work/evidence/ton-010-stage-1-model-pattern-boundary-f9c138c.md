# SUPERSEDED — TON-010 Stage 1 model/pattern boundary

The defaulted-compatibility recommendation below predates the project-wide
no-backwards-compatibility policy. Current document definitions use v7 and
reject obsolete definitions.

- Git HEAD: `f9c138c493a9d687b5300abddf14e78281f2ad63`
- Worktree: TON-013 has broad dirty UI/resource/docs changes; the requested
  model/render/persistence/preset files were clean at inspection time.
- Verified: `Document.artwork_pipeline` is authoritative; `RenderVariant` is
  still document-wide; Shapes resolve to canonical mark geometry and Curves
  emit canonical path commands; persistence validates and canonicalizes at
  save/load boundaries.
- Safe Stage 1 scope: add a registry/types module with stable dotted pattern
  IDs, family/output/version metadata, validated versioned parameter payloads,
  and separate mark/path output types. Add non-invasive adapters only; do not
  change `RenderVariant`, visible UI, existing render dispatch, or output
  semantics.
- Relevant files/symbols: `src/model.rs` (`Document`, `RenderVariant`,
  `ClosedShapePath`, `CurvePath`), `src/render.rs` (`MarkSet`, `MarkGeometry`),
  `src/curve_render.rs` (`CurveGeometry`, `CurveOutline`), `src/persistence.rs`,
  `src/preset.rs`, `src/lib.rs`.
- Unresolved before implementation: whether immediate document-schema
  persistence is required; defaulted compatibility fields are safer than a
  format bump during this dirty TON-013 checkpoint.
