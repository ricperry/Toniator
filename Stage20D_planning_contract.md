# Stage 20D Planning Contract: Curved and Procedural Guide Mechanisms

## Status

Approved on 2026-08-13 as a decision-complete planning-only contract. Stage
20D remains `Planned`; implementation has not started. This approval and its
local planning checkpoint authorize no source edit, push, Stage 20E, or later
work. A separate explicit implementation request is required before the
implementation start gate may run.

## Start gate evidence

- Repository: `/home/ricperry1/projects/Toniator`.
- Branch: `rewrite/greenfield-foundation`.
- Accepted Stage 20C checkpoint and current HEAD:
  `2b8fceccd8e20e8343b993732efd87f8a2a33967`.
- Direct parent: `08d970a6134e1d55d93180910971247e7c7342ec`.
- Upstream matched HEAD when planning began.
- The tracked worktree was clean when planning began. Ignored handoff and
  checkout-local evidence files remain non-authoritative evidence.
- `ProgressTracker.md` and `docs/GREENFIELD_REWRITE_PLAN.md` record Stage 20C
  complete and Stage 20D+ planned.
- The accepted Stage 20C implementation, independent review `PASS`, acceptance
  evidence, and documentation closeout were read and matched this checkout.
- Protected specifications were read with `Project Specification/Addendum.md`
  taking precedence. No protected file was changed.
- Installed `semantic-map 0.1.1` reported a complete but stale index anchored
  before accepted Stages 20B and 20C. Exact pre-20B symbols were useful only
  for navigation; current source and tests are authoritative. No cache update
  was authorized, and no further usage-evaluation maintenance is required.

Before any source edit, the parent must verify that current HEAD is the named
Stage 20D planning checkpoint whose direct parent is the exact Stage 20C
checkpoint above, then revalidate the branch, upstream, complete worktree,
protected inputs, and Stage 20C evidence. The tracked worktree must be clean;
only the approved ignored handoff/evidence may be present unless the user
explicitly permits another item.

## Goal

Add a generic headless guide-production boundary that lets the existing
`GuideIntersections` family and its existing selected-intersection and
along-guide site products consume either:

1. a document-owned Stage 20C `OpenPath`; or
2. a deterministic procedural circular arc.

Each definition owns one through four stable guide dimensions. Each dimension
selects one prototype and either a single instance or a density-resolved
translation stack. Geometry produces complete ordered guide-path instances
over the padded local generation domain; patterns produce truthful guide and
site results; the engine resolves document resources and keys the family cache
by resolved content. Existing circle realization may consume the resulting
sites through the accepted pipeline without a new family, renderer branch, or
canonical path output.

Stage 20D establishes reusable guide construction, repetition, coverage,
provenance, cancellation, identity, and persistence. It stops before guide
paths become visible strokes or topology.

## Explicit non-goals

Stage 20D does not add or change:

- a `Curves` or named artistic pattern family;
- document-owned closed-shape realization or user-shape marks (Stage 20E);
- guide-path/stroke canonical output, stroke width, joins, caps, preview,
  render, PNG, or SVG behavior (Stage 20G);
- full affine tiling, scale/shear authoring, or motif repetition;
- normal-offset curves, crossing dissolution, split/collapse cleanup, or
  region offsetting (Stage 20M);
- graphs, networks, walks, mazes, arrangement faces, regions, Voronoi, or
  composite output;
- arbitrary formulas, a procedural DSL, plug-ins, scripts, noise, warps,
  spirals, contours, or source-derived guide displacement;
- GTK/libadwaita, direct manipulation, canvas editing, CLI, or accessibility
  exposure;
- preset recipes, bundled presets, preset persistence, or preset-library UI;
- a canonical path/stroke/region variant or a renderer fallback;
- a document/container/preset schema-version increment, a new migration, or
  compatibility for superseded pre-release formats;
- persisted resolved paths, guide instances, coverage plans, sites,
  provenance results, fingerprints, caches, history, descriptors, drafts, or
  UI state;
- assets, fixtures, protected specifications, `ToniatorLegacy/`, Cargo
  manifests/lockfile, or Stage 20E+ work;
- a commit or push before their separate approval gates.

`Tile` and `NormalOffset` are deliberately absent from the Stage 20D enum,
rather than present as partial or unsupported variants. Full `Tile` requires
general affine scale/shear authority; `NormalOffset` requires the later
crossing/offset/collapse contract. A later approved stage may add variants
without reinterpreting Stage 20D data.

## Domain vocabulary and ownership

The domain model adds these persisted intent types:

```rust
pub enum GuidePrototype {
    AuthoredOpenPath {
        structure_id: AuthoredStructureId,
    },
    CircularArc {
        center: AuthoredPoint2,
        radius: f64,
        start_angle_degrees: f64,
        sweep_angle_degrees: f64,
    },
}

pub enum GuideRepetition {
    Single,
    TransformStack {
        direction_degrees: f64,
        spacing_multiplier: f64,
    },
}

pub struct GuideDimension {
    pub id: GuideDimensionId,
    pub baseline_angle_degrees: f64,
    pub phase: f64,
    pub prototype: GuidePrototype,
    pub repetition: GuideRepetition,
}
```

`GuideDimensionId` remains the existing document-wide stable dimension
identity. There is no new resource ID, per-segment ID, persisted guide-instance
ID, name, style, fill, stroke, smoothing, winding, or cache identity.

`GuidePrototype` is pattern-local intent:

- `AuthoredOpenPath` resolves only by raw `AuthoredStructureId` in the owning
  `Document` and shares that resource.
- `CircularArc` is the one bounded procedural prototype. Its center is in
  local pattern coordinates. It is not an authored structure, a shape, a
  region, or a render primitive.

`GuideDimension.baseline_angle_degrees` rotates the complete resolved
prototype counterclockwise about the pattern-local origin before repetition;
for an arc this rotates its center and all constructed points, and for an
authored path it rotates every point/control point without changing the shared
resource. The dimension's phase is a finite pattern-local distance, matching
the accepted straight-guide authority rather than a normalized fraction.

`GuideRepetition::TransformStack.direction_degrees` is relative to the
baseline angle. The repetition unit vector is therefore resolved at
`baseline_angle_degrees + direction_degrees`; `Single` uses the baseline unit
vector as its implicit repetition direction. Every instance first receives
the common translation `phase * unit_vector`. `Single` then stops at index
zero, while `TransformStack` adds `index * spacing` along the same vector.
`spacing_multiplier` multiplies the existing directional density spacing
resolved along that vector. Phase is reduced modulo resolved spacing only for
coverage-range calculation and reporting; its authored bits remain unchanged
in persistence and identity. These transform rules apply identically to both
prototype variants.

## Pattern-definition integration

Keep `PatternFamily::GuideIntersections` unchanged. Add exactly one guide-root
mechanism:

```rust
PatternMechanism::GuideDimensions {
    id: PatternMechanismId,
    dimensions: Vec<GuideDimension>,
}
```

The new root is consumed by the existing, unchanged variants:

```rust
PatternMechanism::SelectedGuideIntersections { ... }
PatternMechanism::AlongGuideSites { ... }
```

The existing `PatternOutputLayer::MarkPrototype` and circle output contract
remain unchanged. A new constructor may assemble the exact supported order:

```rust
PatternDefinition::generalized_guides(
    id,
    name,
    guide_id,
    site_id,
    output_id,
    dimensions,
    GeneralizedSiteProduct,
    MarkOrientation,
    CoveragePolicy,
)
```

The definition contains exactly the guide root followed by one site-product
mechanism, then exactly one existing circle mark-prototype output. This is a
new guide producer, not a new family or output kind. The accepted legacy and
generalized-straight mechanism forms and their exact evaluation/serialization
paths remain unchanged.

`AddTypedPatternDefinition` and
`ReplaceSelectedChannelDefinitionTopology` remain the construction/topology
commands. Stage 20D adds no ID-free `PatternDefinitionRecipe` because a raw
document-owned structure reference is not portable preset intent.

## Reference, copy, and removal semantics

- Every `AuthoredOpenPath.structure_id` must resolve in the owning document.
- The resolved structure must have declared kind `OpenPath`. A coincident seam
  does not make it a closed shape; the declared kind remains authoritative.
- A guide dimension owns exactly one prototype. A definition may contain one
  through four dimensions, and multiple dimensions or definitions may share
  the same authored structure ID.
- Duplicating a pattern definition copies the raw structure references and
  therefore shares the same authored resources. It remaps only the
  definition-owned mechanism, dimension, and output IDs under the existing
  duplication authority.
- Duplicating an authored structure still allocates a fresh resource and
  retargets nothing. Retargeting requires an explicit guide-prototype edit.
- Replacing an authored open path preserves its ID and store order; every
  referencing definition observes the shared edit.
- `Document::authored_structure_is_referenced` must scan every
  `GuideDimensions` prototype. Removing any live referenced structure rejects
  with the already accepted `authored_structures.remove.referenced` path and
  message.
- A replacement that would change a referenced `OpenPath` to `ClosedShape`
  fails complete candidate validation before document or history publication.
- Direct open-path replacement reports the ordered, deduplicated document
  channels linked to every referencing definition. Add, duplicate, an
  unreferenced closed-shape replacement, and successful unreferenced removal
  retain an empty affected-channel set.

## Validation and fixed bounds

Domain validation runs before any authoritative mutation, save, cache lookup,
or family allocation.

- `GuideDimensions` contains one through four dimensions.
- Dimension IDs are nonzero, unique document-wide, and stored in authored
  order under the existing dimension namespace.
- Site selections retain their accepted cardinality and stored-order rules:
  at least two dimensions for intersections and at least one for along-guide
  sites; no duplicate, missing, or positional aliases.
- Every continuous authored value is finite.
- `GuideDimension.baseline_angle_degrees` and `GuideDimension.phase` are
  finite. They remain raw authored values; validation does not normalize or
  rewrite either value.
- `CircularArc.radius` is strictly positive.
- `CircularArc.sweep_angle_degrees` is nonzero and has absolute value at most
  360 degrees. A full revolution remains an open guide prototype and implies
  no fill, region, or shape semantics.
- `TransformStack.spacing_multiplier` is strictly positive.
- `Single` has no dormant spacing or direction payload; it still consumes the
  dimension-owned baseline angle and phase exactly as defined above.
- Resolved density spacing and every computed transform, projection, index,
  point, tangent, length, and bound must remain finite.
- Existing Stage 20C limits remain unchanged: 4,096 structures, 4,096
  segments per structure, and 65,536 authored segments per document.
- Existing Stage 20B path limits remain unchanged: 4,096 segments per path,
  subdivision depth 48, 262,144 work items/segment pairs, 65,536 arc-length
  leaves, 4,096 intersections, 4,096 clipping fragments, and 65,536 clipped
  segments as applicable.
- Existing `EvaluationLimits::max_family_candidates` remains the configurable
  family-wide aggregate bound. Before allocation, Stage 20D checks guide
  instances, selected guide-instance pairs, segment-pair products, merge work,
  and predictable along-guide counts with checked arithmetic. During
  evaluation it also bounds raw intersections, merged sites, and emitted
  along-guide sites. No new unbounded work list is permitted.

The new stable domain paths and literal messages are:

```text
pattern_definitions.mechanisms.guide_dimensions
  "guide dimensions must contain one through four entries"
pattern_definitions.mechanisms.guide_dimensions.id
  "guide dimension IDs must be nonzero and unique in stored order"
pattern_definitions.mechanisms.guide_dimensions.baseline_angle
  "guide dimension baseline angles must be finite"
pattern_definitions.mechanisms.guide_dimensions.phase
  "guide dimension phases must be finite"
pattern_definitions.mechanisms.guide_prototype.reference
  "authored guide prototype references a missing structure"
pattern_definitions.mechanisms.guide_prototype.kind
  "authored guide prototypes require an open path"
pattern_definitions.mechanisms.guide_prototype.arc.center
  "circular-arc centers must be finite"
pattern_definitions.mechanisms.guide_prototype.arc.radius
  "circular-arc radius must be positive and finite"
pattern_definitions.mechanisms.guide_prototype.arc.angles
  "circular-arc angles must be finite with a nonzero sweep of at most 360 degrees"
pattern_definitions.mechanisms.guide_repetition.direction
  "guide stack direction must be finite"
pattern_definitions.mechanisms.guide_repetition.spacing_multiplier
  "guide stack spacing multiplier must be positive and finite"
```

Existing selection, output-orientation, coverage, command-stale/no-op,
resource-removal, path, and geometry diagnostics remain unchanged.

## Procedural circular-arc construction

Geometry adds one deterministic Stage 20B construction boundary for
`CircularArc`:

1. Split the signed sweep into `ceil(abs(sweep) / 90)` equal angular spans,
   yielding one through four segments.
2. Convert each span to one cubic Bézier with control factor
   `4 / 3 * tan(span_radians / 4)` and the analytical endpoint tangents.
3. Reuse each computed prior endpoint as the next segment start so exact C0
   continuity is structural rather than tolerance-joined.
4. Build an explicitly `Open` `CurvePath`, even for a 360-degree sweep.
5. Reject any non-finite intermediate through the stable arc diagnostic before
   exposing a partial path.

This approximation policy and segment order are fixed Stage 20D identity.
There is no caller tolerance, adaptive procedural tessellation, smoothing, or
second curve representation.

## Geometry guide authority

Geometry owns these derived, non-persisted concepts (exact field privacy may
follow existing crate conventions):

```rust
pub struct GuidePathInstance {
    id: GuideInstanceId,
    source_structure_id: Option<AuthoredStructureId>,
    path: CurvePath,
}

pub struct GuidePathLocationProvenance {
    pub guide_id: GuideInstanceId,
    pub segment_index: usize,
    pub parameter_bits: u64,
}

pub struct GuidePathSet {
    family_fingerprint: String,
    guide_mechanism_id: PatternMechanismId,
    guides: Vec<GuidePathInstance>,
}

pub struct GuideCoveragePlan {
    generation_domain: Bounds,
    per_dimension: Vec<GuideDimensionCoverage>,
}
```

`GuideInstanceId { dimension_id, index }` remains the sole derived guide
identity. `Single` uses index `0`. `TransformStack` uses signed integer indices
in ascending order. Instances are grouped by stored dimension order, then
index. `GuidePathSet` validates nonempty family/mechanism identity, unique
ordered guide IDs, finite paths, and exact emission order; it never sorts or
renumbers caller data.

Resolved authored prototypes call `CurvePath::from_authored_structure`.
Procedural arcs use the fixed construction above. Repetition transforms the
complete local path and never edits the source resource or manufactures a
closing segment.

`FamilySiteProvenance` gains curve-specific variants rather than mislabeling
curve sites as straight-guide results:

```rust
CurveGuideIntersection {
    contributors: Vec<GuidePathLocationProvenance>,
}
CurveAlongGuide {
    location: GuidePathLocationProvenance,
    guide_order: usize,
    sequence: i64,
    absolute_arc_position_bits: u64,
    local_arc_position_bits: u64,
}
```

Existing straight and random provenance variants remain byte-for-byte and
behaviorally unchanged. Curve contributor order is selected-dimension order,
then guide instance ID. Locations retain exact segment index and parameter
bits so later consumers need not invent a screen-axis or global normalized
parameter.

`TypedFamilyOutput` exposes `guide_path_set() -> Option<&GuidePathSet>` as the
truthful reusable guide authority. Existing straight/random results may return
`None`; Stage 20D curve-guide results return `Some`. Renderers and current
circle realizers continue consuming only accepted canonical marks and never
read this set directly.

## Coverage and transform-stack semantics

Coverage completeness means that no complete guide instance or site whose
support envelope can affect the padded generation domain is omitted. It does
not mean that a one-dimensional guide geometrically fills every point of the
two-dimensional domain.

The required order is:

```text
visible canvas bounds
→ add support radius, antialias margin, and structural guard depth
→ inverse-transform the padded bounds into pattern-local coordinates
→ resolve each prototype and density-dependent repetition interval
→ plan every complete local guide instance that can affect the domain
→ generate intersections or along-guide sites in the complete local domain
→ transform guide/site geometry into document coordinates
→ classify sites as Canvas or Guard
→ existing realization and final-consumer clipping
```

- Reuse the accepted channel rotation-about-center plus translation transform;
  Stage 20D adds no scale or shear.
- `Single` emits exactly one complete prototype at index zero. It is complete
  even when it produces no visible sites; coverage never clips or extends it.
- Resolve the dimension transform first: rotate the complete prototype by its
  baseline angle, resolve the repetition vector from the baseline plus the
  stack-relative direction (or the baseline alone for `Single`), and apply the
  dimension's raw phase as a local-distance translation on that vector.
- `TransformStack` resolves spacing with the accepted directional density
  metric along that vector, multiplies it by the authored spacing multiplier,
  and adds `index * spacing` to the already phase-translated full prototype.
- Rotate a prototype into the stack projection frame and reuse conservative
  Stage 20B bounds. Solve the inclusive checked integer range whose translated
  projection interval can overlap the local padded domain, then extend both
  ends by `coverage.guard_steps`. This may conservatively include extra full
  instances but may omit none.
- For along-guide sampling, validate
  `spacing_x = canvas.width / density.across_x` and
  `spacing_y = canvas.height / density.across_y`, then define the exact global
  upper bound over every possible finite unit normal as
  `max_directional_spacing = max(spacing_x, spacing_y)`. The conservative
  along-guide interval is
  `max_directional_spacing * AlongGuideSites.interval_multiplier`; checked
  finite arithmetic is mandatory. This bound participates in preallocation
  and candidate-limit checks even when the realized tangent directions use
  smaller local intervals.
- The continuous padded margin is existing maximum support radius plus the
  existing antialias margin plus `guard_steps` times the maximum of every
  resolved stack spacing and the exact conservative along-guide interval.
  Bounds/projection failure is an error, never an empty partial success.
- Canvas bounds select generation extent and final `SiteScope` only. They do
  not close, extend, split, connect, or otherwise create guide topology.

Stable evaluator paths and messages are:

```text
coverage.curved_guides.numeric_overflow
  "curved-guide coverage arithmetic overflowed"
coverage.curved_guides.instance_limit
  "curved-guide instance count exceeds the configured family limit"
coverage.curved_guides.pairwise_limit
  "curved-guide pairwise work exceeds the configured family limit"
coverage.curved_guides.merge_limit
  "curved-guide merge work exceeds the configured family limit"
coverage.curved_guides.along_guide_limit
  "curved along-guide site count exceeds the configured family limit"
coverage.curved_guides.proof
  "curved-guide coverage could not prove a complete generation envelope"
pattern.family.curved_guides.tangent
  "curved along-guide sampling requires a nonstationary tangent"
```

Reuse `evaluation.cancelled` exactly. Stage 20B overlap, subdivision,
arc-length, tangent, and numeric errors remain intact when they are the direct
failing authority.

## Existing site products over guide paths

### Intersections

- Pair only instances from distinct selected dimensions, following selected
  dimension order and ascending guide IDs.
- Call the accepted `CurvePath::intersections`; retain crossing and tangent
  results and reject overlap atomically through the existing overlap path.
- Preserve both segment-local locations in curve-specific provenance.
- Raw order is selected dimension-pair order, left guide ID, right guide ID,
  then Stage 20B path-intersection order.
- Merge coincident records only through the existing declared
  `merge_epsilon`. The first raw record supplies the exact emitted position;
  contributor locations are deduplicated and ordered by selected dimension,
  guide ID, segment index, then parameter bits.
- Emit sites in first-record order with normal `FamilySiteId` ordinals. Distinct
  self-visits with distinct path locations remain distinct unless the declared
  site merge contract joins their raw positions.

### Along-guide sites

- Build the accepted `PathArcLength` for every selected guide path and sample
  by arc length, never by Bézier parameter.
- The local interval is the existing `interval_multiplier` times directional
  density spacing evaluated from the nonstationary local unit tangent at the
  current sample. This intentionally permits nonuniform arc spacing under an
  anisotropic density metric.
- `AlongGuideSites.phase.rem_euclid(1.0)` chooses the first fractional local
  interval from the authored path start. Sequences then increase from zero in
  guide order until the complete finite path is exhausted.
- A stationary tangent at a required sampling location fails the complete
  family request; no chord, screen-axis, or previous-tangent fallback exists.
- Emission order is selected dimension order, guide index, then sequence.
  Provenance retains guide order, sequence, source-start absolute arc bits,
  instance-local arc bits, and the exact path location.

Sites inside the exact visible canvas are `Canvas`; sites outside it but inside
the padded generation domain are `Guard`; all others are omitted only after
the complete guide/site calculation establishes the domain relationship.

## Cancellation and atomic publication

Check cancellation:

- before resource resolution and coverage planning;
- before each prototype conversion and dimension plan;
- before aggregate allocations;
- for each generated guide instance;
- for each selected guide-pair batch and intersection result batch;
- for each arc-length guide and bounded site batch;
- before `GuidePathSet`/`FamilySiteSet` construction; and
- before returning the complete typed family result and before scheduler/cache
  acceptance.

Every intermediate lives in local candidate vectors. Missing/wrong-kind
resources, invalid arcs, unprovable coverage, geometry errors, limit
exhaustion, cancellation, supersession, and downstream failure publish no
partial guide set, site set, cache transaction, scene, raster, revision, or
history entry. Existing latest-only scheduler acceptance remains authoritative.

## Commands, descriptors, and history

Stage 20D adds these structural `PatternDefinitionEdit` leaves:

```rust
SetGuidePrototype { mechanism_id, dimension_id, prototype }
SetGuideAuthoredStructure { mechanism_id, dimension_id, structure_id }
SetGuideArcCenterX { mechanism_id, dimension_id, value }
SetGuideArcCenterY { mechanism_id, dimension_id, value }
SetGuideArcRadius { mechanism_id, dimension_id, value }
SetGuideArcStartAngle { mechanism_id, dimension_id, value }
SetGuideArcSweepAngle { mechanism_id, dimension_id, value }
SetGuideBaselineAngle { mechanism_id, dimension_id, value }
SetGuidePhase { mechanism_id, dimension_id, value }
SetGuideRepetition { mechanism_id, dimension_id, repetition }
SetGuideStackDirection { mechanism_id, dimension_id, value }
SetGuideStackSpacingMultiplier { mechanism_id, dimension_id, value }
```

The compound prototype/repetition setters carry a complete new variant and
therefore require no fabricated dormant defaults. Payload-specific setters are
applicable only to the active variant. Every edit is `Family` invalidation,
uses the existing stale-base/no-op/candidate validation, and is owned by
`DocumentHistory` through the existing selected copy-on-edit or explicit
shared-edit commands.

The exhaustive property surface adds corresponding fields, command kinds,
current values, contracts, and projections:

```text
GuidePrototype
GuideAuthoredStructure
GuideArcCenterX
GuideArcCenterY
GuideArcRadius
GuideArcStartAngle
GuideArcSweepAngle
GuideBaselineAngle
GuidePhase
GuideRepetition
GuideStackDirection
GuideStackSpacingMultiplier
```

It also adds stable enum choices `GuidePrototypeKind::{AuthoredOpenPath,
CircularArc}`, `GuideRepetitionKind::{Single, TransformStack}`, typed
`PropertyReferenceValue::AuthoredStructure`, and applicability dependencies
for generic guide dimensions, authored-path prototypes, circular arcs, and
transform stacks. The authored reference is a singular stable-ID reference.
Descriptors expose no labels, layout, geometry result, default resource,
resolver, cache state, or UI policy.

Selected copy-on-edit duplicates the pattern definition, remaps its internal
IDs, preserves/shared the raw authored structure reference, retargets only the
selected channel, and reports that channel. Explicit shared edits preserve the
definition ID and report every linked channel in document order.

## Invalidation, identity, and cache contract

- Every guide prototype, resource reference, procedural parameter,
  repetition, coverage, selection, intersection, and along-guide edit is
  `InvalidationLevel::Family`.
- Replacing any referenced authored open path is `Family` and reports every
  linked consumer channel. Closed shapes cannot be guide references and retain
  the accepted Stage 20C realization-level behavior when otherwise edited.
- The raw resource ID is never sufficient family identity.

The Stage 20D family fingerprint and both single-channel and complete-document
family cache keys include:

- definition/family/mechanism IDs and stored order;
- ordered dimension IDs plus every baseline-angle and dimension-phase raw bit;
- prototype discriminant and all procedural parameter bits;
- for an authored prototype: structure ID, declared kind, ordered segment
  discriminants, and every point/control coordinate bit;
- repetition discriminant and every raw parameter bit;
- selected site product, selected dimensions, merge epsilon or along interval
  and phase;
- canvas, density metric/aspect policy, channel rotation and translation;
- coverage guard steps, maximum support radius, the exact conservative
  along-guide interval bound, fixed antialias/arc-policy contract IDs; and
- configured family candidate limit because it changes acceptance.

The fingerprint uses a new fixed contract prefix such as
`toniator-stage-20d-guide-family-v1` and the existing deterministic hash
mechanism. It excludes source pixels unless an existing source-dependent
structural mechanism requires them; presentation and realization-only values
remain excluded. Resolved resource content is added to `FamilyDefinitionKey`
or an adjacent explicit resolved-guide key before cache lookup. A same-ID
resource content edit must miss family; identical resolved content under an
unchanged definition may reuse family according to the exact key.

No cache schema or serialized derived identity is added. Failures and
superseded requests leave the last accepted cache unchanged.

## Resolver and public headless pipeline boundary

Definition-only resolution remains valid for all existing resource-free
families. A resource-bearing guide definition requires an explicit document
resolver boundary; it must never consult global state.

Patterns adds a document-aware resolver/evaluator surface conceptually named:

```rust
resolve_document_pattern_pipeline(document, definition)
evaluate_document_typed_family_cancellable(document, definition, request, cancel)
```

It resolves and converts every authored guide before constructing the
`FamilyCapability`. Calling the legacy definition-only resolver with a
resource-bearing guide definition returns:

```text
pattern.pipeline.guide_resources
  "document-owned guide resources require document-aware pipeline resolution"
```

The engine's existing public complete-document evaluation uses the
document-aware path and passes the resolved content into its family key. This
is the sole integration consumer added by Stage 20D. The CLI, app, renderer,
and export crates are unchanged.

## Persistence

Keep container version 1, document schema 2, preset format 1, the immutable v1
parser/migration, and rejection of unknown versions.

Add current-v2 DTO variants mirroring only persisted intent:

```rust
PatternMechanismDtoV2::GuideDimensions { id, dimensions }
GuideDimensionDtoV2 { id, baseline_angle_degrees, phase, prototype, repetition }
GuidePrototypeDtoV2::{AuthoredOpenPath { structure_id }, CircularArc { ... }}
GuideRepetitionDtoV2::{Single, TransformStack { ... }}
```

Use the existing internally tagged snake-case enum convention, ordered vectors,
integer stable IDs, and `f64` authored values. Rebuild the complete document
and resolve all references through domain validation before a loaded document
can commit.

- Existing v2 definitions contain no new variant and serialize byte-for-byte
  as before.
- Existing v2 documents missing Stage 20C's optional structure store continue
  loading with an empty store; they cannot contain a valid resource-bearing
  guide definition.
- v1 migration creates no generic guide mechanism or authored reference and
  adds no migration report entry.
- Empty authored stores remain omitted and retain the accepted Stage 20C old-v2
  bytes/hash.
- Populated Stage 20D data round-trips definition/mechanism/dimension IDs,
  stored order, prototype/repetition variants, raw resource IDs, and every
  numeric bit deterministically.
- Missing references, closed-shape references, invalid arcs/repetitions, and
  incoherent product selections reject before document commit.
- Preset records and preset recipes remain unchanged. A future portable
  resource/preset contract must decide how authored resources travel; Stage
  20D never embeds a document-local ID in a standalone preset.

## Implementation allowlist and ownership

After explicit contract approval and a separate implementation request, the
parent may update only:

- `Stage20D_planning_contract.md` for an explicitly approved correction;
- `ProgressTracker.md`, Stage 20D status only;
- `docs/GREENFIELD_REWRITE_PLAN.md`, Stage 20D status only;
- checkout-local Stage 20D reviewer/acceptance evidence.

Exactly one `desktop_implementer` is the sole source/test/evidence writer and
owns only:

- `crates/toniator-domain/src/lib.rs`;
- `crates/toniator-domain/tests/curved_guides.rs`;
- `crates/toniator-geometry/src/lib.rs` for guide exports only;
- `crates/toniator-geometry/src/guides/mod.rs`;
- `crates/toniator-geometry/src/guides/prototype.rs`;
- `crates/toniator-geometry/src/guides/repetition.rs`;
- `crates/toniator-geometry/src/guides/coverage.rs`;
- `crates/toniator-geometry/tests/curved_guides.rs`;
- `crates/toniator-patterns/src/lib.rs`;
- `crates/toniator-patterns/tests/curved_guides.rs`;
- `crates/toniator-engine/src/lib.rs`;
- `crates/toniator-engine/tests/document_evaluation.rs`, Stage 20D cases only;
- `crates/toniator-engine/tests/scheduler.rs`, only if a Stage 20D latest-only
  case is required without changing scheduler implementation;
- `crates/toniator-io/src/lib.rs`;
- `crates/toniator-io/tests/persistence.rs`, Stage 20D cases only;
- `.codex-work/agents/desktop-implementer/2026-08-13-stage20d-curved-guides.md`.

The writer may modify an accepted Stage 20B curve implementation file only if
the fixed circular-arc constructor cannot coherently live under `guides/` while
delegating to public Stage 20B segment/path constructors. That is not presumed
by this contract; if a Stage 20B implementation file or any other excluded path
is actually required, stop for parent approval rather than silently widening
the allowlist.

No Cargo manifest/lockfile change is permitted. Everything else is excluded,
including presets, renderer/export, CLI, app, assets, fixtures, protected
specifications, `ToniatorLegacy/`, canonical output, and later validation
directories.

## Focused tests

The Stage 20D implementation uses these exact new test responsibilities and
names:

```text
generic_guide_definitions_validate_prototypes_repetition_references_and_products
generic_guide_edits_descriptors_history_and_affected_channels_are_atomic
duplicated_definitions_share_authored_guides_and_live_references_block_removal

authored_and_circular_arc_prototypes_resolve_to_exact_ordered_guide_paths
single_and_transform_stack_coverage_emit_complete_deterministic_instances
curved_path_intersections_and_arc_length_sites_preserve_locations_and_limits
curved_along_guide_coverage_uses_exact_anisotropic_interval_upper_bound

curved_guides_reuse_existing_site_products_with_truthful_guide_and_site_sets
curved_guide_limits_cancellation_and_geometry_failures_publish_no_partial_output

stage20d_authored_content_repetition_and_layout_key_family_cache_exactly
stage20d_failed_or_superseded_evaluation_preserves_last_accepted_cache

stage20d_guide_definitions_round_trip_references_variants_order_and_numeric_bits
stage20d_absent_variants_preserve_existing_v2_bytes_and_v1_migration
stage20d_invalid_or_closed_guide_references_reject_before_document_commit
```

Together they prove:

- all stable paths/messages, one-to-four bounds, ID uniqueness/order, arc and
  repetition validation, missing/wrong-kind references, and product selection;
- selected/shared edits, copy-on-edit sharing, stale/no-op behavior, undo/redo,
  direct resource affected channels, reference-blocked removal, and complete
  document/history atomicity;
- exact authored conversion, fixed arc cubic construction, signed stack IDs,
  conservative coverage, no canvas-created topology, baseline/phase transform
  order, and finite/overflow/limit failures;
- curve intersection/tangency/overlap behavior, multiway merging, segment-local
  provenance, variable tangent-derived arc-length sampling, the exact
  anisotropic worst-case interval witness, scope, ordering, cancellation, and
  no partial `GuidePathSet`/`FamilySiteSet`;
- resource content, variant parameters, repetition, layout, product, coverage,
  and limits in both family cache paths, plus last-accepted cache preservation;
- deterministic schema-2 round-trip, exact existing-v2 preservation, v1
  migration, unchanged presets, and invalid-load rejection before commit.

The directly relevant accepted regressions remain Stage 20B curve paths,
Stage 20C authored structures/conversion, Stage 16A generalized straight guide
products, Stage 17 descriptor/cache authority, and current-v2 persistence. A
test filter must select only the new Stage 20D cases where the shared historical
test binary contains unrelated cases.

## Verification gate

Run only the focused Stage 20D and directly relevant foundational checks:

```bash
cargo fmt --all -- --check
cargo test -p toniator-domain --test curved_guides
cargo test -p toniator-domain --test authored_structures
cargo test -p toniator-geometry --test curved_guides
cargo test -p toniator-geometry --test curve_paths
cargo test -p toniator-patterns --test curved_guides
cargo test -p toniator-patterns --test grid_family
cargo test -p toniator-engine --test document_evaluation stage20d_
cargo test -p toniator-engine --test scheduler stage20d_
cargo test -p toniator-io --test persistence stage20d_
cargo check -p toniator-domain -p toniator-geometry -p toniator-patterns -p toniator-engine -p toniator-io --all-targets
cargo clippy -p toniator-domain -p toniator-geometry -p toniator-patterns -p toniator-engine -p toniator-io --all-targets -- -D warnings
bash scripts/validate_architecture.sh
git diff --check
git diff --exit-code -- Cargo.toml Cargo.lock crates/toniator-domain/Cargo.toml crates/toniator-geometry/Cargo.toml crates/toniator-patterns/Cargo.toml crates/toniator-engine/Cargo.toml crates/toniator-io/Cargo.toml
git diff --exit-code -- ToniatorLegacy 'Project Specification' assets fixtures
sha256sum assets/HolidayMugs_2024_2025.toniator assets/raster-sample.png assets/vector-sample.svg
git status --short --branch --untracked-files=all
```

If no Stage 20D scheduler test is needed after implementation inspection, the
writer must not add one and the filtered scheduler command is omitted rather
than reporting a zero-test pass as evidence.

Stage 20D is headless structural/evaluator work. It does not load, sample,
render, preview, or export the immutable artwork inputs, so no natural-size
artifact or historic validation directory is generated. The hash check proves
only that protected inputs stayed unchanged. No GTK/private Wayland session,
screenshot, human visual review, or manual desktop acceptance applies.

## Documentation and evidence obligations

Every touched non-trivial named Rust function, method, and test receives
literal `///` responsibility documentation covering present authority,
invariants/bounds, side effects, and applicable `# Errors`, `# Panics`, or
`# Safety` conditions. Do not perform a repository-wide documentation pass.

The implementation writer records checkout-aware evidence under
`.codex-work/agents/desktop-implementer/`; one independent read-only
`test_reviewer` reviews the final diff, numerical/coverage proof, resource and
cache identity, persistence, tests, allowlist, and evidence. Any correction
returns to the same writer. Evidence never advances stage status or substitutes
for parent review, user acceptance, or a checkpoint.

The parent alone owns contract interpretation, stage status, durable
documentation, acceptance, checkpointing, and the push gate. A documentation
maintainer is considered only after verified implementation and explicit user
acceptance materially change durable capability documentation.

## Approval and stop gates

1. This approved contract and its local planning checkpoint are the only
   deliverables of the planning task. Stage 20D remains `Planned` and no
   source/status edit begins.
2. Contract approval makes the plan authoritative but does not itself start
   implementation. A separate explicit implementation request is required.
3. At an explicit implementation request, the parent rechecks the start gate,
   records only Stage 20D `In progress`, and assigns exactly one writer.
4. The writer implements only the allowlist and runs the focused gate.
5. One independent read-only reviewer returns `PASS` or actionable findings;
   all corrections return to the same writer.
6. After final parent verification, the parent may record only Stage 20D
   `Implemented awaiting review` and must stop with the work uncommitted.
7. Explicit user acceptance is required for `Accepted awaiting checkpoint`.
8. A separate explicit authorization is required for a local checkpoint.
9. Push requires separate explicit authorization. Stage 20E or any later work
   requires its own fresh contract and start request.

Stop with the worktree preserved and report the exact decision if the smallest
coherent implementation would require:

- a prototype or repetition variant outside this contract;
- a Cargo manifest/lockfile or excluded-path edit;
- a document/container/preset version change or new migration;
- changing existing-v2 bytes when Stage 20D variants are absent;
- embedding document-local resource IDs in presets;
- a general affine scale/shear transform, Tile, NormalOffset, curve cleanup,
  topology, canonical path/stroke/region, renderer/export, CLI, or GTK work;
- relaxing a Stage 20B/20C invariant or accepted diagnostic;
- unbounded generation or a coverage result that cannot conservatively prove
  that no relevant full instance was omitted;
- a cache identity that lacks resolved authored content;
- a product decision not fixed by this contract.
