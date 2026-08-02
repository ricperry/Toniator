# TON-010 bundled recipes — Substage 3D1 implementation evidence

Date: 2026-08-01

## Scope and baseline

- Git HEAD observed before work: `262c7e857446ded100d4a90fd23d651e52460665`.
- Accepted/user/parent worktree changes were preserved. No commit, push, deletion, UI change, document/preset format update, renderer-dispatch conversion, Curves work, or site/Voronoi change occurred.
- This substage owns only the bundled Shapes declarative contract, typed operation descriptors, and one-way adaptation boundary. It intentionally implements no Shapes native operation body.
- Parent review correction reopened 3D1 only. The corrected scope covers guided authoring metadata, truthful lattice parameter ownership, removal of an unused Crosshatch recipe parameter, and strict source-path validation before SVG derivation.

## Changed files

- `assets/patterns/compat-shapes.v1.tnpattern`
  - Adds immutable `compat.shapes.v1`, retaining `PatternId::COMPATIBILITY_SHAPES_V1`.
  - Declares six composable stages: lattice placement, source sampling, mark mapping, primitive selection, transforms, and canonical mark emission.
  - Declares renderer-relevant global and per-output-channel settings, strict defaults/constraints, quick controls, mark output capability, a safe digest-identified default custom motif SVG, and complete guided authoring sections.
  - The lattice node owns resolution, grid rotation/pivot, and phase offsets; mark transformation owns local rotation/scale/deformation plus `max-size` and `grid-scale` effects.
  - Removes `crosshatch-color`: no Shapes recipe node consumes it.
- `src/pattern_definition.rs` (accepted framework file)
  - Adds truthful Shapes-only recipe port types and six operation descriptors. They are descriptors only; the native runtime has no Shapes body or registry implementation in 3D1.
- `src/bundled_pattern_definitions.rs` and `src/lib.rs`
  - Load/export Shapes through the same compile-time byte, strict parser, and bundled registry path as Weighted. The bundled registry now holds both definitions.
- `src/shapes_recipe.rs`
  - Adds the strict one-way Shapes adapter. It returns `ShapesRecipeAdaptation { definition, instance }` because a pure instance cannot carry dynamic custom motif bytes safely.
  - Validates every resolved global and per-channel `ClosedShapePath` with the public native validator before converting it deterministically into a derived safe SVG asset, calculating its SHA-256 digest, appending it only to the transient derived definition, and storing only `SvgAsset(digest)` values in the strict instance.
- `src/render.rs`
  - Adds test-only thread-local instrumentation proving a live Shapes document still reaches the existing compatibility mark renderer exactly once. Production dispatch is unchanged.

## Contract decisions

- `value_mode` and `single_channel` are deliberately absent from the Shapes recipe instance: `ArtworkPipelineSettings` is their authority, and duplicating them would create a second parameter authority.
- `base_channel` is not a recipe-stage input: rendering consumes the already authoritative effective per-channel records. It remains in `Document.pattern_state` for its current inspector/base-edit semantics and is not fabricated as geometry state.
- Every declared recipe parameter appears exactly once in a nonempty guided authoring section: Placement, Motif, Modulation, Deformation, or Output. There are no hidden/internal Shapes recipe parameters in 3D1. Placement reflects the lattice-driving source-sampling coordinates; Deformation is present because the graph has a distinct mark transformation node.
- Editable `ClosedShapePath` values and embedded SVG assets remain distinct concepts. A `ClosedShapePath` is native editable cubic geometry in current Shapes state; it is never serialized into an opaque Text protocol. At this adapter boundary it is deterministically projected to a derived, validated SVG asset for the future primitive stage. The persisted/bundled recipe only identifies that asset by SHA-256 digest, and the immutable bundle is not mutated.
- The bundled default asset is safe SVG with a verified digest. Dynamic projected assets traverse the same `PatternDefinition::validate_assets` safety, parser, byte-limit, and digest checks before the instance validates.
- The definition covers global dimensions/lattice/mark mapping/shared primitive state and each semantic channel's enablement, color, transform, response, primitive, and custom motif reference. The currently renderer-irrelevant base inspector record and pipeline settings are intentionally not geometry parameters.
- `crosshatch_color` remains current compatibility state for the Crosshatch output-assignment workflow, not a Shapes recipe parameter. Before legacy Shapes state is removed, the later schema boundary must retain or relocate it to the artwork-pipeline/output-assignment compatibility state; it must not be reintroduced as a Shapes recipe parameter.

## Verification

- `cargo test --locked bundled_shapes --lib` — strict bundled Shapes graph/load/registry test passed, including complete authoring sections, parameter ownership, and absent Crosshatch recipe state.
- `cargo test --locked shapes_recipe --lib` — deterministic adapter, enum/transform mapping, derived digest asset, and malformed global/per-channel path rejection tests passed.
- `cargo test --locked live_shapes_document_render_remains_on_compatibility_branch --lib` — live Shapes compatibility branch proof passed.
- `cargo test --locked --quiet` — 194 library tests and 48 binary/UI tests passed.
- `cargo check --locked` and `cargo check --locked --release` — passed.
- `cargo clippy --locked --all-targets -- -D warnings` — passed.
- `cargo fmt --check` and `git diff --check` — passed.

No interactive GTK launch, screenshot, or new export artifact was required: 3D1 does not alter UI, rendering, preview, PNG, or SVG dispatch.

## Remaining 3D2/3D3 work and invalidation

- 3D2 must implement bounded native bodies for these six declared Shapes stages and prove intermediate/runtime typing; it must not use the old whole renderer as a body.
- 3D3 must establish authoritative output equivalence before a separate accepted renderer-dispatch handoff. The existing Shapes compatibility renderer remains live through this boundary.
- Documentation likely affected at the later schema/dispatch milestone: the Stage 5 architecture map and the current Crosshatch compatibility-state boundary. No durable documentation changed in this bounded correction.
- Invalidate this evidence if `WebShapeSettings` rendering semantics, custom path validation, pipeline authority, Crosshatch compatibility authority, SVG safety/digest contract, declared Shapes graph/ports/layout, or parameter scope changes.
