# Stage 20+ decomposition

## Status and authority

**Approved roadmap scope.** The approved roadmap in
`docs/GREENFIELD_REWRITE_PLAN.md` / `ProgressTracker.md` records Stage 20A as
complete, Stage 20B as complete in the single Stage 20B acceptance checkpoint,
Stage 20C as complete in its single named acceptance checkpoint, and the
**remaining Stage 20D+ work as Planned**: curved or procedural guide
generators and coverage; connected or network topology; regions, ordinary
Voronoi, reusable offset/collapse; composite outputs; multiframe sources; and
simple transitions. Stage 19B is complete at implementation checkpoint
`b0b84e4`. Stage 20A is complete at implementation checkpoint `b7fbd81`.

This document records the approved decomposition after the Stage 20C
acceptance checkpoint. Stage 20C's named checkpoint has direct parent
`08d970a`; its own hash is intentionally not self-referenced here. The
document does not authorize Stage 20D implementation or reorder the remaining
Stage 20D+ roadmap.

| Authority | Present responsibility | Stage 20+ rule |
|---|---|---|
| `toniator-domain` | Typed `PatternDefinition`, mechanisms, output layers, commands, descriptors, revisions/history | Persisted user intent is the only editor authority. |
| `toniator-patterns` | Generic family planning, source-dependent structural density where declared, modulation and typed realization | New mechanisms enter the generic pipeline; no artistic-name dispatch. |
| `toniator-geometry` | Points, transforms, bounds, straight guides/sites/provenance and canonical primitives | Own reusable paths, graphs, regions, clipping and topology mathematics. |
| `toniator-engine` | Snapshot evaluation, cache boundaries, scheduling/cancellation and complete-document orchestration | Keep cache identity and transactional publication authoritative and headless. |
| `toniator-io` | Current container/DTO persistence and preset serialization | Persist schema and deterministic inputs, never caches or regenerated topology. |
| Render/export | Canonical-scene consumers for preview, PNG and SVG | Consume canonical geometry only; final clipping is consumer-side. |
| `toniator-app` | GTK private drafts, descriptor/command bindings and preview coordination | Expose commands/descriptors; never evaluate a frontend model. |

Current types now separate the shared site contract: `PatternFamily`,
`PatternMechanism`, `PatternOutputLayer`, opaque `TypedFamilyOutput`,
`FamilySiteSet`, `FamilySiteProvenance`, `GridFamilyOutput`,
`GeneralizedStraightGuideOutput`, `GeneralizedSite`, `RandomSiteProvenance`,
`SiteId`, `IntersectionSite`, `SiteScope`, and `CanonicalCircleMark` exist.
Generalized along-guide and random results retain truthful provenance in
`FamilySiteSet`; the existing circle realization receives only a private
compatibility view. Stage 20C now adds document-owned authored open paths and
closed shapes, their authoritative commands/descriptors/history, current-v2
persistence, and an exact conversion boundary to Stage 20B `CurvePath`
construction geometry. No guide consumer, graph, region, canonical path/region,
curve prototype, Voronoi, or multiframe/transition authority exists yet.

## Ten-concern separation audit

| Concern | Current authority | Missing or coupled issue | Intended boundary | Invalidation / cache class |
|---|---|---|---|---|
| Source/sample | `SourceReference`, sampling fields, decoder identity and conditional artwork-weighted random input | Frame selection does not exist; source values must not become a renderer setting. | 20P source/frame contract; existing sampling stays reusable. | **Source/modulation:** decode always keys logical+content identity; only declared structural weighting reaches family. |
| Guide/procedural structure | `StraightGuideDimensions`, selected intersections, along-guide sites and coverage planners | Straight and future curved prototypes lack a shared path/repetition substrate. | 20B–20D. | **Structural:** guide prototype, dimensions, coverage and phase miss family/downstream. |
| Sites/distribution | `TypedFamilyOutput` and `FamilySiteSet`; `GridFamilyOutput` remains a dedicated diagnostic/compatibility type | Site interchange is now truthful and mechanism-neutral; future consumers must not depend on the private circle adapter. | 20B and later consumers. | **Structural:** site process/selection/seed/weighting changes miss family. |
| Adjacency/topology | None; only guide/site provenance exists | No reusable graph; no topology can consume random and grid sites alike. | 20H graph, 20I programs. | **Structural:** graph rule/program/seed miss family-derived topology and downstream. |
| Regions/cells/faces | None | No canonical region, Voronoi or guide-face output; canvas must not create cells. | 20K–20M. | **Structural:** region source/treatment changes miss realization/downstream; final clip stays consumer-only. |
| Modulation/sampling | `PatternModulation` boundary, source mapping and mark response; random weighting is explicitly structural | Current modulation is minimal and must not be conflated with site generation or presentation. | Extend only after output kinds exist, beginning with 20E/20G/20N/20O. | **Modulation/source** when sampled values alter geometry/paint; artwork-weighted placement is explicitly structural. |
| Realization | `PatternOutputLayer`, typed circular realization and `CanonicalCircleMark` | Current output is circles only and receives an intersection-shaped compatibility input. | 20E, 20G, 20K, 20O. | **Realization:** prototype, geometry response, path/region treatment miss realization/downstream while retaining family. |
| Per-channel transforms/controls | `ChannelPatternLayout`, appearance, mapping, descriptor/command/history authority | Structural transform is distinct from mark response and presentation; future controls need the same descriptors. | Existing authority is reused; expose new fields only with their headless checkpoint then 20F/20J/20N/20S. | Layout is **structural**; mark size/thickness/inset is **realization**; sampled response is **modulation/source**; opacity/visibility/color are **presentation**. |
| Canonical geometry/render/export | geometry canonical circles; renderer scene; engine preview/raster/SVG consumers | Canonical paths/regions/strokes are missing; renderer must not generate them. | 20B, 20E, 20G, 20K–20O. | Canonical content follows structural/realization changes; raster target/AA/background are **presentation** consumers. |
| Persistence/presets/UI | domain/IO current-v2 authored structures and pure preset records; app private drafts/commands/descriptors | Temporal values lack schema; presets/UI must not own evaluator state. | Each mechanism checkpoint after 20C; 20F/20J/20N/20S UI. | Persisted structural/realization/modulation/presentation values use their existing invalidation class; never persist derived sites/topology/caches. |

The classification is deliberate: structural values select source structure,
guides, sites, topology and regions; realization values select marks, paths,
strokes, fills and geometric response; modulation/source values sample source
data or transform it into those outputs; presentation values control channel
appearance and final raster presentation. A value that crosses a boundary must
use the earliest class it changes, never a convenient lower-cost cache class.

## Dependency narrative

Stage 20A established the truthful, common derived site interchange. Curves,
random distributions, straight intersections and future authored structures can
now produce sites without claiming the same guide contributors. The reusable
curve/path geometry dependency and document-owned authored-structure boundary
are now accepted; the next dependency is curved/procedural guide coverage.
Curved
guide coverage consumes them; graph/topology and Voronoi consume the common
site set independently; generic region operations consume canonical regions;
composite output combines already-canonical kinds. Frame sources and
transitions are deliberately separate temporal/source work.

```text
20A site interchange ─ 20B curve/path geometry ─ 20C authored structures ─ 20D curved guides
                                                              ├─ 20E shape marks ─ 20F GTK exposure
                                                              ├─ 20G guide paths/strokes ─ 20J GTK exposure
                                                              └─ 20L arrangement faces
20A site interchange ─ 20H adjacency ─ 20I network programs ─ 20J GTK exposure
20A site interchange ─ 20K ordinary Voronoi ─┐
20B/20D curved guides ─ 20L arrangement faces ─┴─ 20M offset/collapse ─ 20N GTK region exposure
20E shape marks + 20G strokes + 20K Voronoi ─ 20O core composite outputs ─ later bounded exposure
20P frame-source abstraction ─ 20Q frame export ─ 20R transitions ─ 20S GTK exposure
```

Canvas bounds select coverage and apply final clipping, never sites, edges,
faces, cells, maze boundaries or topology. Persist schema, stable resource IDs,
seeds and topology-program parameters; regenerate sites, adjacency, paths,
faces, cells, offsets, caches and scheduler results.

## Remaining planned checkpoints after Stage 20C

| Label | Outcome | Non-goals | Layers / dependencies | Verification | GTK | Stop / Goal candidacy |
|---|---|---|---|---|---|---|
| 20A Structural Site Interchange | **Complete at `b7fbd81`:** deterministic `FamilySiteSet`, truthful provenance, accepted output parity. | Schema, paths, graphs, regions, rendering, UI. | geometry/patterns; accepted evaluator/cache. | Site contracts; family/realization/PNG/SVG/cache parity on both baselines. | None. | Historical authority; no further implementation. |
| 20B Canonical Curve/Path Geometry | **Complete in the Stage 20B acceptance checkpoint:** reusable polyline/Bézier segments, arc length, tangents, bounds, intersections, and clipping. | Authoring schema, curved families, strokes. | geometry; 20A. | Property, degeneracy, clipping tests. | None. | Historical accepted authority; no further implementation. |
| 20C Document-owned Authored Structures | **Complete in the single named Stage 20C acceptance checkpoint:** document-owned open paths/closed shapes, authoritative commands/descriptors/history, deterministic current-v2 persistence, and exact conversion to Stage 20B construction geometry. The checkpoint's direct parent is `08d970a`; its own hash is intentionally not self-referenced. | Consumers, evaluators, caches, canonical output, rendering/export, CLI, GTK, presets, schema-version changes, and later stages. | domain/io/geometry; 20B. | Validation, history, descriptor, save/reload, and exact conversion tests passed. | None. | Historical accepted authority; no further implementation. |
| 20D Curved/Procedural Guide Mechanisms | Guide prototypes plus repetition/coverage, reusing dimensions without a curves family. | Shape marks, strokes, topology. | domain/patterns/geometry; 20A–20C. | Coverage, IDs, cancellation, cache/invalidation, persistence. | None. | Parent review; needs replanning. |
| 20E User-shape Mark Realization | Authored closed structures as ordinary site marks through canonical geometry. | Renderer modes, preset magic, GTK. | patterns/geometry/render; 20A/20C. | Public evaluator, canonical PNG/SVG, round-trip. | None. | Parent review; needs replanning. |
| 20F Guide/shape editor exposure | Descriptor-driven private-draft exposure for 20C–20E. | Frontend evaluator, preset library/Save & Apply. | app; accepted commands/descriptors. | GTK Wayland plus command/persistence/export checks. | Yes. | Parent/user GNOME review; normal prompt/review. |
| 20G Connected Guide Paths and Strokes | Guide path output, canonical strokes, channel thickness response. | Networks/mazes. | geometry/patterns/render; 20B/20D. | Path, clip, PNG/SVG parity. | None. | Planned after dependencies; needs fresh contract. |
| 20H Site Adjacency Graphs | Deterministic mechanism-neutral adjacency over `FamilySiteSet`. | Connection program/rendering. | geometry/patterns; 20A. | Graph invariants/order/degeneracy/cache identity. | None. | Planned after dependency contract; needs fresh contract. |
| 20I Connection Programs and Networks | Generic masks/walks/tree/maze programs yielding paths. | Named maze renderer/preset-only topology. | domain/patterns/geometry; 20H. | Seed/replay, topology legality, persistence. | None. | Parent review; needs replanning. |
| 20J Connected-output exposure | Command/descriptor surfaces for 20G–20I. | UI-owned graphs. | app; 20G–20I. | Private Sway plus authoritative output checks. | Yes. | Parent/user GNOME review; normal prompt/review. |
| 20K Canonical Regions and ordinary Voronoi | Geometry-owned ordinary site-to-cell canonical regions. | Placement settings, power diagrams, guide faces. | geometry/patterns; 20A. | Grid/random guard/cell/canonical output tests. | None. | Planned after dependency contract; needs fresh contract. |
| 20L Guide-arrangement Faces | Complete off-canvas guide faces. | Canvas-created boundaries/special renderer. | geometry/patterns; 20B/20D/20K concepts. | Arrangement, coverage, degeneracy tests. | None. | Parent review; needs replanning. |
| 20M Reusable Region Offset/Collapse | Generic offset, split/dissolve, winding/collapse behavior. | Voronoi-only offset/artistic cleanup. | geometry; 20K and/or 20L. | Offset/crossing/collapse/clipping tests. | None. | Parent review; needs replanning. |
| 20N Region editor exposure | Descriptor/command UI, channel inset response. | UI region mathematics. | app; 20K–20M. | Private Sway plus schema/evaluator/export checks. | Yes. | Parent/user GNOME review; normal prompt/review. |
| 20O Composite Output Layers | Ordered canonical marks, paths and regions from typed layers. | Renderer artistic branches/hidden z-order. | domain/patterns/render/io; 20E/20G/20K. | Ordering, invalidation, persistence, PNG/SVG. | None in core; later bounded exposure only. | Parent review; needs replanning. |
| 20P Frame-source Abstraction | Still/multiframe source and deterministic frame selection. | Timeline/interpolation. | domain/sampling/io/engine; parent media decision. | Decode identity/cache/persistence/natural inputs. | None. | Parent review; needs replanning. |
| 20Q Frame-sequence Output | Bounded CLI/export sequence output. | Animation UI/arbitrary encoder. | engine/cli/render/io; 20P. | Numbered PNG/SVG deterministic artifacts. | None. | Planned after dependency contract; needs fresh contract. |
| 20R Simple Start/End Transitions | Start/end definitions and bounded continuous interpolation. | Timeline/keyframes/hidden presets. | domain/engine/io; 20Q. | Interpolation, serialization, frame parity. | None. | Parent review; needs replanning. |
| 20S Transition exposure | Minimal descriptor-driven start/end UI. | Frontend interpolation authority. | app; 20R. | GTK and canonical frame checks. | Yes. | Parent/user GNOME review; normal prompt/review. |

## Recipe capability matrix

All recipes must use normal serialized schema and evaluator routes; no renderer
branch, preset-only bypass or test-only authority is permitted.

| Probe | Missing primitive introduction / convergence | Eventual serialized-schema path | Evaluator / geometry path | UI surfaces | Requires renderer/preset special case? | Normal-authority proof |
|---|---|---|---|---|---|---|
| Weighted random sites + user mark | Existing random/weighting/source; **20A** sites, **20C** shape resource, **20E** shape marks, **20F** UI. Schema-only after 20E. | `RandomSites → RandomSiteProduct → output layer(structure ID)`. | `site_set()` → modulation/sampling → canonical closed-shape mark. | Pattern Editor mechanisms/layers; channel mapping/response/transform. | **No.** | Public command, save/reopen, engine evaluation, canonical output; preset name has no effect. |
| Fully controlled ordered grid + reusable mark | Existing dimensions/intersections; **20A** sites, **20C/20E** mark. Schema-only after 20E. | `StraightGuideDimensions + SelectedGuideIntersections + structure-reference layer`. | Coverage → site set → realization → final consumer clip. | Existing guide descriptors plus future mark selector. | **No.** | Three-dimension command/history/reload/public evaluator proves no grid renderer mode. |
| Curved/linear crosshatching channels | **20B** paths, **20C** structures, **20D** curved guides, **20G** strokes, **20J** UI. Schema-only after 20G. | Per-channel straight/curve prototypes and ordered path layers. | Guide coverage → guide paths → canonical strokes → channel compositor. | Pattern Editor guides/layers; channel transforms/appearance. | **No.** | Two normal channel definitions persist/evaluate/export with no crosshatch mode. |
| Triangular grid feeding maze | **20A** sites, **20H** adjacency, **20I** programs, **20J** UI. Schema-only after 20I. | Three dimensions → selected intersections → graph-capable program. | `FamilySiteSet →` graph → maze/tree → canonical paths. | Dimension/topology descriptors and thickness. | **No.** | Saved three-dimension deterministic seed/replay via public graph/path evaluator. |
| Random Voronoi + offset cells + residual marks | **20A** sites, **20K** Voronoi, **20M** offset, **20O** composite, **20N** UI. Schema-only after 20O. | Random sites → Voronoi treatment + ordinary mark layers. | Guard-inclusive sites → geometry Voronoi → generic offset → canonical regions/marks. | Output/treatment descriptors and channel inset/modulation. | **No.** | Schema round-trip/engine scene prove no Voronoi placement settings or special route. |

## Decisions, warnings and documentation

| Parent decision | Latest decision checkpoint |
|---|---|
| Path command vocabulary/open-closed resource ownership | 20B/20C |
| Curve/procedural prototypes and repetition strategies | 20D |
| Direct-manipulation guide/shape UX | 20F |
| Stroke caps, joins, smoothing and crossing defaults | 20G/20I |
| Neighbor strategies and initial maze/tree programs | 20H/20I |
| Region fill/outline, cleanup and collapse defaults | 20K–20M |
| Composite ordering/source-filter semantics | 20O |
| Fedora-native multiframe backend/formats | 20P |
| Interpolation subset and transition UI | 20R/20S |

Do not add a `Curves` family owning structures: share guide/repetition
authority where evidence permits. Do not let Maze, Voronoi or a preset name
dispatch a renderer. Do not persist regenerated topology/cells/caches. Do not
let canvas clipping create structure or put source modulation in presentation
cache identity.

On every implementation touch, add literal `///` responsibility docs to each
non-trivial changed Rust function/method/test (authority, invariants/bounds,
side effects and relevant Errors/Panics/Safety), update durable plan/tracker
only with parent authority, and record checkout-aware evidence without treating
it as product authority.
