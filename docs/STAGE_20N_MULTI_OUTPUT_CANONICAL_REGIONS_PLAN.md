# Stage 20N — Multi-output Authority and Canonical Regions

Status: **Complete at implementation checkpoint
`b8701686042a69fcd1ac68a4038adbad4c0ccdc9`** (user accepted 2026-08-25).

## Summary

Stage 20N establishes the data, realization, cache, and rendering foundations required for later composite and filled-region work. It does not introduce a concrete region-producing output, Voronoi cells, guide faces, region treatments, or GTK behavior.

The accepted implementation becomes structurally multi-output-capable while
retaining the current one-output authoring and validation gate. Stage 20R
remains responsible for permitting heterogeneous multi-output definitions.

The accepted implementation includes the schema-v5/preset-v3 transition,
ordered keyed output authority, independently keyed realization/cache units,
canonical-region geometry/render foundations, and atomic publication behavior.
It does not introduce a concrete region-producing output, Voronoi cells, guide
faces, region treatments, heterogeneous composites, or GTK behavior.

## Public contracts and authoritative state

- Replace separate structural definitions with atomic `PatternDefinitionBundle { definition, output_settings }` records. Each `PatternOutputSettings { output_layer_id, response }` must match definition output IDs, order, cardinality, and response kind exactly.
- Remove the singular geometry response from `DocumentPatternSettings`; retain document-wide density, pattern rotation, shape rotation, and selected definition ID.
- Replace channel response intent with ordered `PatternOutputResponseDelta { output_layer_id, delta }` entries. Effective instances expose ordered `EffectivePatternOutputSettings { output_layer_id, response }`.
- Reject missing, duplicate, reordered, foreign, or kind-incompatible settings and deltas. Effective response arithmetic remains finite, typed, unclamped, and domain-owned.
- Convert `PatternDefinitionRecipe` into a complete ID-free unit containing renamed structural `PatternStructureRecipe` plus ordered `PatternOutputSettingsRecipe` entries. Materialization allocates output IDs and binds each recipe setting atomically.
- Recipe or definition-selection replacement retains still-valid deltas, deterministically removes affected foreign or incompatible deltas, remaps deltas for exact old-to-new copy-on-edit correspondence, retains IDs for shared edits, and records complete prior bundles and deltas for exact undo restoration. Invalid recipes, allocation exhaustion, stale bases, and unrelated validation failures fail atomically.
- Key response commands and resets by `PatternOutputLayerId`. Document-base response changes invalidate `Realization`; structural and recipe changes remain `Family`.
- Add `PropertyTarget::ChannelOutput(channel_id, output_layer_id)`. Descriptors use output targets, preserve typed bounds/reset semantics, and appear in definition output order.
- Capability projections are ordered output records containing output-layer ID, structural capability, and applicable base/effective typed response. Frontends do not interpret these facts in Stage 20N.

## Pipeline, cache, and canonical geometry

- Generalize realization around independently keyed output capabilities and `TypedOutputRealization` units. Realizers receive one explicit output capability and matching effective setting; no code indexes a global response.
- Continue rejecting definitions other than the current single compatible output while orchestration iterates ordered outputs.
- Evaluate one family per channel, compute support independently per output, use the maximum family request, and retain broader-envelope reuse.
- Cache output realizations independently by output ID, output contract, typed response, inputs, shape rotation, algorithm contracts, and limits. Aggregate ordered output fingerprints only above units; a one-output aggregate returns the existing output fingerprint unchanged.
- Expose per-output cache diagnostics. Cancellation, failure, or stale completion publishes neither partial output units nor candidate family, scene, raster, or SVG state.
- Add geometry-owned canonical regions: `CanonicalRegionSourceId::SiteOwners(Vec<FamilySiteId>)`, `CanonicalRegionSourceId::GuideBoundary(Vec<StructuralPathLocationProvenance>)`, `CanonicalRegionId { output_layer_id, source_id, component_ordinal }`, `CanonicalRegion`, `CanonicalRegionSet`, proposal/source-group inputs, diagnostics, limits, and a cancellable builder.
- Source groups provide one or more closed `CurvePath` components. Builder canonicalizes signed zero; requires finite, exactly closed, connected line/cubic rings; rejects zero-length, zero-area, self-crossing, overlapping, or nonadjacent-touching rings; reverses negative Cartesian winding to CCW; rotates lexicographically smallest anchor with complete cyclic-segment tie break; sorts source IDs/components; assigns contiguous ordinals; computes exact analytical line/cubic area and finite bounds; and emits no holes, empty, or partial set.
- Default nonzero limits: 1,048,576 source groups; 1,048,576 regions; 8,388,608 ring segments; 67,108,864 inspections. Poll cancellation through every substantial builder and renderer phase; cancellation uses `evaluation.cancelled`; other diagnostics use `region.identity.*`, `region.geometry.*`, `region.limits.*`, and `region.allocation.*`. Fingerprints include contract, supplied source identity, ordered IDs, exact canonical segments, bounds, and area, excluding limits/diagnostics.

## Rendering and persistence

- Reshape each channel `RenderLayer` into ordered `RenderOutputLayer { output_layer_id, geometry, primitive_paints }` values. Add canonical regions to `GeometryOutput`; regions use fixed nonzero fill and closed rings; raster/SVG only fill and final-canvas clip.
- Validate output IDs, paint cardinality, geometry, and channel-model compatibility. Sampled paint remains accepted only for current mark output; region sampling is Stage 20Q. Preserve existing mark/stroke geometry fingerprints and equivalent one-output PNG/SVG painter behavior.
- Document schema v5 persists bundles, keyed output settings, response-less document pattern settings, and keyed deltas. Preset v3 persists full structural recipe plus ID-free ordered settings. Reject v4/v2 and older with no migration/adapter; port current fixtures once. Derived effective values, regions, diagnostics, limits, caches, and scheduler state never persist.

## Verification and gates

Focused Stage 20N tests cover bundle alignment; deltas/effective response/history; descriptors/capability/invalidation/single-output gate; v5/v3 persistence and obsolete rejection; output cache/support/cancellation/staleness; canonical-region validation/normalization/limits/fingerprints; and raster/PNG/SVG parity while retaining mark/path behavior. Run only Stage 20N plus directly affected 20G/H/I/M foundational targets, affected-package format/check/strict Clippy, architecture and protected-path validation, immutable-asset hashes, and semantic-map reconciliation.

Generated `target/validation/stage20n/intrinsic/` v5 mark and path artifacts
from both immutable inputs at 1024×1024 and 900×620, direct canonical-region
scenes at both dimensions, native PNG/raw SVG/SVG-raster PNG, applicable
documents, RGB/alpha statistics, hashes, and a manifest. Parent visual
inspection and independent correction re-review passed; no GTK/Wayland run was
required. The user accepted this stage at the local checkpoint above. Do not
push, publish, or begin Stage 20O without separate authorization.

## Explicit non-goals

No concrete region-output intent, Voronoi, guide faces, region treatment/sampling, composites, site filters, gallery, GTK workflow, temporal behavior, compatibility work, wall complements, renderer topology repair, or protected-specification edits.
