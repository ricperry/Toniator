# TON-010 bundled recipes — Substage 3D2 implementation evidence

Date: 2026-08-01

## Scope and checkout

- Repository: `/home/ricperry1/projects/Toniator`.
- Producing agent: `desktop-implementer` (`/root/ton010_recipe_contract_2a`).
- Task: accepted TON-010 Substage 3D2 only — bounded native Shapes recipe bodies and typed runtime values.
- Git HEAD assumed: `262c7e857446ded100d4a90fd23d651e52460665`.
- The worktree was already dirty with accepted/user/parent Stage 5 work. Those changes were preserved; this substage added no commit, reset, deletion, UI, persistence/preset, Curves, site-distribution, Voronoi-geometry, or live-render-dispatch change.
- Parent review correction reopened 3D2 only. It removes built-in ID dispatch from the generic executor and makes Shapes intermediate ports family-specific before validation/execution.

## Files and abstractions

- `src/shapes_native.rs` (new)
  - Owns six fixed Shapes native operation bodies and bounded intermediate types: `ShapesLattice`, `ShapesSamples`, `ShapesMappedValues`, `ShapesSelectedPrimitive`, and `ShapesTransformedMarks`.
  - Implements a semantic custom-motif subset: one safe SVG root/path in the deterministic bundled/adapter form, with explicit uppercase `M`, `L`, `C`, and final `Z` commands only. It verifies digest bytes, closure, segment count, finite f32-representable coordinates, and usable area before any recipe node executes.
  - Defines explicit lattice and candidate bounds (one million each) and preserves cancellation checkpoints in placement, mapping, and transform loops.
- `src/pattern_definition.rs`
  - Adds the Shapes runtime variants, distinct `ShapesSamples`/`ShapesMappedValues` port kinds, the generic `RecipeSourceFieldProvider` request boundary, definition-owned asset context, and a registry-trusted semantic preflight hook.
  - The generic executor invokes the configured registry hook after strict definition/instance validation and before the first node. It has no Shapes ID dispatch; Weighted's registry deliberately has no hook.
  - Corrects stage ownership: `max-size` belongs to `shapes.mark-map`, not `shapes.transforms`. Mapping exactly preserves the old formula and suppresses threshold-mapped zero before a nonzero minimum can create a mark.
- `assets/patterns/compat-shapes.v1.tnpattern` and `src/bundled_pattern_definitions.rs`
  - Bind the corrected operation parameter ownership and distinct Shapes port flow in the immutable recipe, assert it, and prove parameter-valid Shapes-to-Weighted wiring is rejected during graph validation with both incompatible port kinds named.
- `src/lib.rs`
  - Exposes the bounded Shapes native registry/types and generic source-field-provider API.
- `src/weighted_voronoi.rs`
  - Supplies `None` for the new generic provider/assets context fields; its behavior is otherwise unchanged.

No helper was extracted from `src/render.rs`: the live compatibility renderer remains byte/struct authority and oracle. The native module reuses maintained public `calculate_web_grid`, `map_web_threshold`, canonical mark types, strict definition/asset validation, and cancellation services, while implementing bounded staged flow instead of wrapping `generate_web_shape_marks*`.

## Verified behavior

- `shapes.lattice-placement` creates the exact current grid density/resolution frame and retains rotation, pivot, and offset semantics for the downstream transform; it rejects artboard mismatch and excessive source grids.
- `shapes.source-sample` consumes its declared lattice and either a matching direct semantic field or `RecipeSourceFieldProvider`. The provider request receives the declared lattice dimensions, so 3D3 does not need a hidden Shapes-specific lattice prepass to resolve pipeline fields.
- `shapes.mark-map` consumes typed samples and applies threshold, minimum, maximum, and `max-size` with current ordering. A thresholded zero stays zero; `max-size` scales only the upper endpoint.
- Primitive selection supports circle, regular polygon, rectangle, triangle, pentagon, hexagon, and validated editable cubic custom motifs. Safe but unsupported SVG is an actionable error, never rasterized or silently substituted.
- Transform consumes the upstream lattice and selected primitive, checks lattice identity, applies local rotation/scale/width/height/grid-scale behavior, and performs exact phase/rotation/pivot candidate coverage without regenerating an upstream grid.
- Mark emission consumes transformed marks and emits one semantic channel layer with declared enabled/color/opacity values. Crosshatch remains artwork-pipeline/output-assignment authority and is absent from recipe inputs.
- `PatternDefinition::execute_recipe` injects definition-owned immutable assets into operation context and calls only a trusted registry preflight before any node can run. Shapes registers selected user-defined motif validation there; instrumentation proves an unsupported selected motif fails with zero native-node invocations.
- `RecipePortType::ShapesSamples` and `RecipePortType::ShapesMappedValues` prevent a Shapes-to-Weighted connection from reaching runtime. The bundled Shapes graph still validates through the new exact port flow.

## Tests and checks

- `cargo test --locked shapes_native --lib` — 6 focused tests passed: all-operation error paths, cancellation, lattice/candidate bounds including extreme float-to-i32 range pressure, provider dimensions, typed end-to-end flow, every primitive kind, cubic paths, semantic SVG digest/subset registry preflight with zero body invocations, rotated/offset coverage, and exact threshold/nonzero-min/max-size mapping.
- `cargo test --locked bundled_shapes --lib` — bundled graph/registry/ownership test passed, including explicit `ShapesSamples -> Samples` cross-family validation rejection.
- `cargo test --locked shapes_recipe --lib` — 3D1 adapter strictness remains green.
- `cargo test --locked live_shapes_document_render_remains_on_compatibility_branch --lib` — existing live Shapes compatibility-renderer proof passed.
- `cargo test --locked --quiet` — 200 library tests and 48 binary/UI tests passed.
- `cargo check --locked`, `cargo check --locked --release`, and `cargo clippy --locked --all-targets -- -D warnings` — passed.
- `cargo fmt --check` and `git diff --check` — passed.

No GTK launch, screenshot, preview/export artifact, or manual acceptance claim was made: 3D2 adds no UI or live rendering change. `git diff --name-only -- src/site_distribution.rs src/voronoi_geometry.rs` was empty.

## 3D3 boundary, uncertainty, and invalidation

- 3D3 owns exhaustive oracle equivalence and any separately accepted dispatch handoff. The old Shapes renderer remains live through this boundary.
- 3D3 orchestration must request semantic pipeline fields through `RecipeSourceFieldProvider` only for enabled channels, thereby avoiding field/native work for disabled channels without adding pipeline selection to recipe parameters. This substage intentionally does not add that orchestration or switch dispatch.
- Future review should compare native candidate range/mark output against the retained renderer across large custom motifs and all artwork-pipeline assignments; 3D2 proves typed behavior, not exhaustive parity.
- Durable documentation likely affected at milestone review: the Stage 5 architecture map should describe the generic recipe-driven field-request boundary and the stricter custom-SVG execution subset.
- Invalidate this evidence if Shapes grid/range/threshold/size semantics, custom-path-to-SVG projection, supported SVG subset, trusted registry-preflight contract, runtime context/field-request contract, family-specific port kinds, declared graph ownership, resource limits, or canonical mark semantics change.
