# TON-010 Stage 5 framework restart

Recorded 2026-07-29 on `TON-010-Stage5-Framework-Restart`.

## Why Stage 5 was restarted

The previous `TON-010-Stage5-Voronoi` tip remains preserved as
`archive/TON-010-Stage5-Voronoi-pre-framework` and tag
`TON-010-stage5-voronoi-pre-framework` at `e37eeb2`. Its useful clipped-cell
and boundary mathematics is a reference, but its generation layer placed site
distribution, source response, Voronoi construction, canonical output, and
pattern settings in one Weighted Voronoi module. That ownership would make
Shapes, Curves, Pointillism, maze, spiral, grid, stacked-curve, and
traced-curve additions repeat the same services.

The restart is based directly on the accepted Stage 4 checkpoint `87b4ce3`.
Stage 4 authority, strict validation, cancellation, canonical output, and
shared preview/PNG/SVG routes remain in force.

## Generation pipeline and ownership

```text
Prepared artwork / resolved channel field
    -> site_distribution (neutral guide/site basis)
    -> voronoi_geometry (clipped cells and boundaries)
    -> weighted_voronoi (source response and canonical regions)
    -> CanonicalPatternOutput
    -> preview / PNG / SVG
```

`src/site_distribution.rs` owns bounded deterministic candidate generation,
uniform and source-weighted selection, polarity, strength, arrangement policy,
semantic identity input, stable ordering, fingerprints, and cancellation.
Uniform placement uses jittered stratified candidates; weighted placement uses
finite exponential-priority selection without rejection-loop fallback.

`src/voronoi_geometry.rs` owns pure half-plane clipping, bounded cell
construction, explicit artboard versus shared interior boundaries, and
boundary-derived insets. It does not know about channels, artwork, pattern
settings, UI, or rendering.

`src/weighted_voronoi.rs` is an adapter. It validates persisted settings,
requests each enabled channel's resolved field, maps settings to neutral
requests, applies response insets, allocates canonical positive/subtractive
regions, and records explicit region relationships and cache fingerprints.
Region IDs are deliberately disjoint; relationships, never numeric adjacency,
identify a positive region's subtractive region.

`Document.pattern_state` remains the only persisted pattern authority.
`RenderVariant::WeightedVoronoiCanonicalV1` is a derived dispatch marker. The
registry supplies stable identity, metadata, specialized control descriptors,
schema version 3, and generator version 2. The document and preset formats
were not bumped because their persisted envelopes are unchanged; generator
version 1 is rejected explicitly.

## Cache boundaries

Resolved channel fields use the existing bounded request-local cache keyed by
source generation, field bounds, pipeline identity, output model, assignment,
active channel, and enabled semantic channels. Weighted output metadata keeps
source generation, resolved-field generation, distribution fingerprint,
geometry fingerprint, channel identity, and view key separate. Distribution
settings (seed, count, arrangement, mode, polarity, strength) are therefore
distinct from geometry settings (boundary gap and response/inset controls),
and view-only preview presentation remains downstream of canonical output.
There is no process-global or unbounded pattern cache.

## Future consumers and intentional deferrals

The neutral services are deliberately small. Future Shapes can consume ordered
points or structured guide primitives; Curves can consume sampled paths,
connected paths, or intersections; constructive patterns can consume segments,
faces, and shared boundaries. Those modes are not silently migrated by this
stage.

Weighted Voronoi is the only pattern currently using the new site and geometry
services. Shapes and Curves still use their established compatibility
generators through the canonical output adapters. Pointillism, maze, spiral,
grid, stacked-curve, and traced-curve integration, generic guide editors,
custom-pattern libraries, mixed-generator documents, and a global cache are
deferred.

## Validation evidence

Focused distribution tests cover deterministic ordering, seed changes,
source-independent uniform placement, spatial spread, weighted polarity,
exact counts, distinct sites, shared/independent arrangements, cancellation,
and centralized limits. Geometry tests cover clipped bounds, shared interior
boundaries, artboard exclusion, insets, degenerate input, cancellation, and
clustered input. Weighted tests cover semantic fields, uniform independence,
arrangement policy, strict generator rejection, persistence, undo/redo,
preset behavior, canonical preview/PNG/SVG parity, and perimeter omission.
The realized GTK selector/control regression is also covered; human GNOME/
Wayland pointer and screen-reader acceptance remains unclaimed.
