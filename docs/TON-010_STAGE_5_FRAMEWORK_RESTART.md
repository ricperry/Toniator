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
requests, applies response insets, and emits each final boundary-derived inset
polygon as one positive canonical region. Raw clipped cells and raw-to-inset
boundary rings are construction data only; they are not final Weighted
Voronoi artwork. `WeightedVoronoiCellRegion` preserves semantic channel/site
identity without claiming a cell-sizing subtraction. General canonical
subtraction remains available for genuine holes, knockouts, and other
semantics that cannot be represented as final positive geometry.

`Document.pattern_state` remains the only persisted pattern authority.
`RenderVariant::WeightedVoronoiCanonicalV1` is a derived dispatch marker. The
registry supplies stable identity, metadata, specialized control descriptors,
schema version 3, and generator version 2. The document and preset formats
were not bumped because their persisted envelopes are unchanged; generator
version 1 is rejected explicitly.

## Declarative recipe contract boundary — 2026-08-01

The accepted recipe-contract milestone adds strict `.tnpattern` v1 data types
and validation in `src/pattern_definition.rs`, deterministic layered
resolution and provenance diagnostics in `src/pattern_definition_registry.rs`,
and a bounded cancellation-aware native-operation executor. It reuses
`SiteDistribution`, `DistributionField`, `VoronoiDiagram`, and
`CanonicalPatternOutput`; recipe data cannot load scripts, plugins, native
libraries, or arbitrary code.

The original contract-only status is now superseded by the 2026-08-02
preservation checkpoint. Bundled Shapes, Curves, and Weighted Voronoi resources
have production native operation bodies and execute through the same strict
loader and bounded DAG runtime. Embedded custom Shapes definitions are
persisted and dispatched through the canonical preview/PNG/SVG route. Current
documents are v9 and `.tntr` presets are v6; obsolete definitions are rejected
without migration or defaulting.

The integration is incomplete. The user Pattern Editor constructs a
Shapes-compatible definition by mutating fixed graph nodes and parameters;
named presets and their defaults are selected by hard-coded UI indices. Save As
writes a user `.tnpattern`, but no UI library/import/open or application-wide
layered registry resolution can select it again. The schema-driven guided and
graph editors, full portability/recovery workflow, and compatibility-dispatch
removal remain TON-010 work. See `ISSUES.md` and
`.codex-work/evidence/ton-010-preservation-checkpoint-audit-2026-08-02.md`.

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

The neutral services are deliberately small. Weighted Voronoi retains
`site_distribution.rs` and `voronoi_geometry.rs` as its algorithm authorities.
Shapes, Curves, and Weighted Voronoi now have bundled recipe adapters and
registered native operations, while typed compatibility adapters and
`RenderVariant` branches remain as production seams. The custom editor exposes
grid, triangular, curve, math, and random variants through a monolithic
Shapes-specific placement operation; these variants are not substitutes for a
general composable operation/editor surface.

Pointillism shared/independent arrangements, a declarative Wave Line Field,
the general guided/graph editor, recipe library/import/export, layered
resolution, and project embedding/recovery remain open. No global cache is
introduced.

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
The remaining Stage 5 manual gate also includes Krita-reference CMYK/RGB
inspection and Inkscape **Break Apart** inspection of editable SVG output.
Preserved reference images are evidence inputs only, not human acceptance.

## Correction pass

The 2026-08-01 correctness pass preserves the framework and changes only its
canonical consumers: semantic region rasterization now renders isolated
per-channel coverage before deterministic RGB additive or CMYK multiplicative
composition, so genuine subtraction cannot erase sibling channels. Direct
Weighted Voronoi inset regions therefore have no cell-sizing subtraction path,
and semantic SVG exports use one editable compound positive path per channel.
The artboard clip remains a page/domain constraint because canonical geometry
may be out of bounds; genuine subtraction masks remain only where genuine
subtractive regions exist. Preview Surface remains preview-only and
Export Background is applied at the export presentation stage.
