# Stage 20K — Parametric Curve Family

Status: **Accepted awaiting checkpoint** (2026-08-22).

The user accepted the verified implementation and artboard-detail correction on
2026-08-22. The implementation checkpoint is pending; no checkpoint hash is
claimed here.

Stage 20K adds the headless `ParametricCurve` family. It starts with finite
round and square spirals, carrying structural shape, turns, radial spacing,
phase, and winding. A source publishes either homogeneous raw paths or
equal-arc sites followed by ordinary marks; composites and additional forms
remain deferred.

`CurveRepetition` is the common finite repetition vocabulary. Existing guide
mechanisms retain their `GuideRepetition` spelling and v4 DTO shape, while
parametric sources use the common type directly. `NormalOffset` delegates to
the accepted Stage 20J geometry service. Generated structures remain finite
and independent of canvas edges; the evaluator only places the local origin
at the canvas center before applying existing effective-channel transforms.

The geometry crate owns deterministic analytic conversion to canonical open
`CurvePath`: round spirals use at-most-quarter-turn cubic Hermite intervals;
square spirals use exact line corners. The public geometry seam includes
path-neutral structural source, instance, and location identity types. Paths,
sites, outlines, capabilities, and caches are derived and never persisted.

Document schema remains v4 with additive parametric DTO branches. Preset v2 is
unchanged. Stage 20G effective settings, Stage 20H capability projection,
Stage 20I canonical connected geometry, Stage 20J offsets, engine cache
authority, and final-consumer clipping remain authoritative. GTK, Pattern
Wizard work, gallery recipes, density terminology, adjacency, regions, and
later forms are excluded.

Focused witnesses cover cubic and square construction, equal-arc sites,
normal-offset composition, v4 intent-only persistence, and intrinsic PNG/SVG
render evidence for both immutable inputs under `target/validation/stage-20k/`.
The intrinsic evidence fixture uses five full turns for both artworks, derives
round pitch from each artboard diagonal and square pitch from its shorter edge,
and places equal-arc sites at one quarter of that pitch. The reusable
arc-length service uses bounded adaptive five-point Gauss-Legendre measurement,
and canonical-stroke rasterization rejects row-inactive outline edges before
subpixel winding tests. Those optimizations preserve geometry and consumer
clipping while keeping artboard-detail evaluation within the existing limits.

## Settled implementation contract

- Geometry owns analytic-to-`CurvePath` conversion. Round spiral conversion
  begins with spans no larger than 90 degrees and recursively bisects a span
  when its cubic Hermite residual at `1/4`, `1/2`, or `3/4` exceeds `1/64`
  document units. It stops at depth 24 and a total 4,096 segments. Square
  turns derive from one phase basis and signed perpendicular swaps, preserving
  exact corners without repeated phase-plus-quarter-turn trigonometry.
- A `StructuralPathSourceId` tags a `GuideDimension` or `ParametricCurve`
  source. `StructuralPathInstanceId` adds repetition and component order, and
  `StructuralPathLocationProvenance` adds exact segment/parameter identity.
  These path-neutral values flow through structural output, family-site
  provenance, canonical strokes, fingerprints, and render-scene identity.
  `GuideInstanceId` remains the authority for actual straight guide and
  intersection products only.
- `CurveRepetition` is the reusable finite repetition authority: `Single`,
  `TransformStack`, and Stage 20J `NormalOffset`. Its nominal basis is radial
  spacing for `Single`, transform spacing for a stack, and absolute offset
  spacing for offsets. Normal-offset cleanup components for one repetition
  retain one continuous equal-arc interval/phase sequence and only suppress a
  shared join.
- The document command/history path has stale-base `PatternDefinitionEdit`
  leaves for spiral shape, turns, radial spacing, phase, winding, repetition,
  stack/offset payloads, and equal-arc interval/phase. They validate the
  active typed variant before candidate publication, preserve inverse/undo,
  and report `Family` invalidation. Topology changes remain recipe replacement.
- Property descriptors expose the active parametric source/site fields from
  the same authoritative definition and never compute a second effective
  model. Stage 20F draft squash retains its existing command/history boundary.
- Persistence remains schema v4 and serializes analytic source intent,
  repetition, and site controls only; it never persists derived paths, sites,
  offsets, or effective instances. Preset format v2 and bytes stay unchanged.
  Cache identity includes the typed structural source/instance and curve
  intent; final renderers still clip canonical output only at their consumer
  boundary.
- Evidence exercises both immutable artwork inputs at intrinsic dimensions,
  validates compact SVG/XML/Inkscape reopening, checks input hashes, and
  records artifacts under `target/validation/stage-20k/`. GTK is excluded
  because this stage has no application surface.
