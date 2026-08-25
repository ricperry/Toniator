# Stages 20N–20S — Headless Pattern Completion Plan

Status: **Accepted roadmap; Stages 20N and 20O are complete and Stages 20P–20S
remain Planned and separately gated** (Stage 20O checkpoint
`7ab97f01ec372ab1e6201b3913742476a1511c02`).

## Summary and stage boundary

Stages 20N–20S complete the remaining headless pattern architecture after the
accepted Stage 20M checkpoint `33f1bde3be9afdc3fb88f479c4ee7ec52b80114a`.
They add multi-output authority, canonical filled regions, ordinary Voronoi and
guide-face sources, region realization, composites, capability completion, and
ordinary serialized gallery recipes.

No stage in this plan adds a GTK workflow, visible frontend control, or
frontend-owned pattern behavior. Narrow `toniator-app` adapters are permitted
only when necessary to keep the workspace compiling; new variants remain
unexposed or read-only until Stage 21. Stage 21 owns all remaining
pattern-authoring GTK work. Stage 22 owns headless frame, media, sequence, and
simple-transition authority. Stage 23 owns temporal GTK controls.

Stage 20N is complete at implementation checkpoint
`b8701686042a69fcd1ac68a4038adbad4c0ccdc9`. Its accepted foundation retains the
one-output authoring/validation gate while providing ordered keyed output
authority, schema-v5/preset-v3 persistence, independently keyed realization and
cache units, and canonical-region/render foundations. It does not add a
concrete region source or treatment. This roadmap does not authorize Stage
20P+ implementation, a tracker status advance for those stages, a commit,
publication, Stage 21+, or a protected-specification revision. Every remaining
row retains its own decision-complete plan, one-writer implementation,
independent read-only review, user acceptance, and checkpoint gate.

## Stage 20N — Multi-output and canonical-region foundation (complete)

- Replace the singular document geometry response with ordered, typed
  per-output settings keyed by `PatternOutputLayerId`. Every structural output
  has exactly one compatible response entry. Reject missing, duplicate,
  reordered, foreign, or output-incompatible entries.
- Give channel deltas and effective instances the same keyed structure.
  Copy-on-edit, explicit shared editing, recipe replacement, history, and
  undo/redo move structural output intent and its settings atomically.
- Introduce current-only document schema v5 and preset format v3. Reject v4/v2
  and older data without a migration or compatibility adapter. A preset recipe
  contains both structural output intent and its aligned output settings so an
  output-kind replacement cannot create a transient invalid document.
- Generalize realization into independently keyed output units while retaining
  one family evaluation. Required family support is the maximum across all
  outputs, and a broader accepted family envelope may satisfy a narrower
  request.
- Add geometry-owned `CanonicalRegionId`, `CanonicalRegion`,
  `CanonicalRegionSet`, diagnostics, limits, cancellation, and fingerprints.
  Each canonical region is one finite, positive-area, canonically oriented
  closed outer ring without a hole. A split produces separate positive
  components with stable ordinals.
- Extend canonical render scenes and PNG/SVG consumers to fill regions and
  perform only final canvas clipping. Renderers never construct, close, offset,
  or repair topology. Existing mark and path fingerprint bytes remain unchanged
  for geometrically equivalent documents.
- Extend the headless capability projection with per-output response facts. Do
  not expose them in GTK during Stage 20.

## Stage 20O — Ordinary Voronoi regions

Status: **Complete at implementation checkpoint
`7ab97f01ec372ab1e6201b3913742476a1511c02`** (user acceptance recorded
2026-08-25; independent re-review and final artifact inspection passed).
Stage 20P+ remains Planned and separately gated.

- Add `PatternOutputLayer::Regions` with
  `RegionSourceIntent::VoronoiSites { site_mechanism_id }` for eligible
  `FamilySiteSet` products: grid/guide intersections, along-guide sites,
  `AlongParametricCurveSites`, and dispersion/random sites. Reject direct raw
  `ParametricPaths`, non-site products, and incompatible capabilities before
  topology allocation. Eligibility follows the authoritative site set, not the
  mechanism's source provenance.
- Use exactly pinned Spade 2.15.1 behind a geometry-owned adapter. Spade types
  never cross a Toniator crate API. Toniator owns stable ordering, identity,
  duplicate policy, canonicalization, coverage, cancellation, resource limits,
  and fingerprints.
- Canonicalize signed zero and coalesce exact duplicate positions. Sorted
  source site IDs co-own one region; near-duplicates remain distinct.
  Diagnostics report duplicate groups and avoided insertions.
- Construct topology from the complete guard-inclusive family result. Emit
  every complete cell whose maximum treatment envelope can affect the canvas,
  including cells owned by off-canvas support sites when needed for boundary
  coverage. Canvas edges never manufacture Voronoi edges or close an unbounded
  cell; an unbounded relevant cell is a stable coverage failure.
- Derive region identity from output-layer identity and all sorted co-owner
  IDs. Sort by smallest owner ID and component ordinal. Fingerprints include
  algorithm contracts, family/site-set identity, co-owner groups, and canonical
  rings, but exclude limits and diagnostics.
- Use configurable nonzero defaults of 1,048,576 source-site groups, 4,194,304
  topology edges, 1,048,576 retained regions, 8,388,608 retained boundary
  points, and 67,108,864 topology inspections. Fail atomically on identity,
  allocation, coverage, geometry, work-limit, or cancellation errors.
- Consume Spade under its Apache-2.0 option, preserve applicable upstream
  license and notice material, and keep Toniator GPL-3.0-only.

## Stage 20P — Guide-arrangement faces

- Add `RegionSourceIntent::GuideFaces { guide_mechanism_id, dimensions }` for
  typed grid structural guides with at least two selected dimensions, including
  currently supported straight and authored curved guides. Reject site-only,
  dispersion, and raw-parametric products.
- Extract a reusable geometry-owned planar-arrangement builder from the
  accepted Stage 20M maze face machinery without changing Stage 20M behavior or
  fingerprints.
- Split guides at intersections, construct a deterministic half-edge embedding,
  and retain every complete bounded positive face whose support envelope can
  affect the canvas. Do not use canvas edges for closure or apply maze-specific
  component selection.
- Derive face IDs from ordered boundary provenance and use the stable area
  centroid as the face reference point. Apply the shared region contracts,
  limits, cancellation, canonicalization, rendering, and fingerprint rules.
- Do not add curved Triagrid/Tetragrid editing; existing valid authored curved
  guides may participate headlessly.

## Stage 20Q — Filled-region realization

- Add fill-only region treatments `Full`, `Scale`, and `ConstantGap`. Do not add
  outlines, outline-only rendering, wall complements, or subtractive region
  geometry.
- Scale a Voronoi cell about its source site and a guide face about its area
  centroid. Scale ranges are finite, ordered, and nonnegative. Zero removes the
  component; values above one may overlap neighboring regions.
- Define Constant Gap in finite absolute canvas units. Positive `g` offsets
  each adjacent region inward by `g / 2`; zero is neutral; negative values grow
  regions and may overlap. Maximum outward expansion participates in required
  support.
- Keep reusable closed-region scaling and offset cleanup in geometry, including
  crossing dissolution, winding normalization, deterministic splitting, and
  collapse. The initial collapse policy is `Remove`; do not invent a fallback
  mark.
- Add a matching typed region geometry response and per-output/channel deltas.
  Region source/treatment edits invalidate `Family`; numeric response changes
  invalidate `Realization`; paint and opacity retain their existing downstream
  invalidation.
- Support `ReferencePoint` sampling at the Voronoi site or guide-face centroid
  and deterministic `AreaAverage` sampling over the untreated base region in
  the decoded source domain. Area averaging uses bounded deterministic
  flattening and sampling-owned piecewise-bilinear integration, follows the
  existing alpha-association contract, and never samples already-modulated
  geometry.
- Add a configurable default limit of 33,554,432 source-pixel cell
  intersections. Allocation, work-limit, cancellation, identity, coverage, and
  geometry failures publish no partial region set.

## Stage 20R — Composite outputs and site filters

- Lift the single-output restriction and allow ordered heterogeneous marks,
  structural paths, connection paths, and filled regions over one family.
- Add authored filters `All`, `SitesUsedBy { output_layer_id }`, and
  `SitesUnusedBy { output_layer_id }`.
- Every realized output publishes a derived `SiteUsageSet`: positive marks use
  their sites, connections use selected-edge endpoints, and regions use every
  co-owner of retained positive regions. Final canvas clipping does not change
  usage.
- Require filter references to target a compatible output over the same site
  mechanism. Reject missing references, self-references, incompatible
  mechanisms, and dependency cycles.
- Evaluate the layer-reference DAG with output-layer ID as the stable
  topological tie-breaker. Authored vector order remains painter order and is
  independent of dependency order.
- Cache one family result and independently keyed output realizations. A
  dependent key includes the referenced usage fingerprint. Do not add separate
  adjacency, Voronoi, arrangement, or filter caches.
- Persist only authored layers, settings, painter order, and filter references.
  Sites, topology, usage sets, regions, diagnostics, limits, caches, cancelled
  candidates, and stale scheduler results remain derived and non-persisted.

## Stage 20S — Headless capability and recipe completion

- Extend the domain-owned projection with region sources, treatments, sampling
  strategies, composite ordering, filter dependencies, typed bounds, and
  conditional availability.
- Add ordinary data-only preset-v3 gallery recipes covering accepted guide,
  mark, spiral, structural-path, and connection behavior plus grid/dispersion
  Voronoi, two-/three-guide faces, full/scaled/constant-gap regions, and
  residual-site and regions-plus-marks composites.
- Reconstruct every recipe through the ordinary domain, engine, geometry,
  sampling, and renderer path. Names, gallery metadata, and eventual thumbnails
  never select evaluator behavior.
- Add no GTK cards, gallery view, thumbnail UI, Pattern Wizard pages, or
  frontend-specific capability interpretation.
- Finish Stage 20 with a full headless architecture review and a bounded Stage
  21 planning handoff.

## Verification and acceptance

- Each stage runs only focused new and directly affected foundational tests for
  validation, descriptors, commands/history, typed replacement, invalidation,
  persistence, fingerprints, replay, cancellation, allocation, limits, cache
  behavior, and stale-publication rejection.
- Schema tests prove current v5/v3 round trips, obsolete-format rejection,
  derived-data absence, and preserved existing mark/path geometry identities.
- Geometry and sampling tests cover duplicates, degeneracy, guard coverage,
  boundary-touching sites, off-canvas support owners, curved faces, splits,
  collapse, positive/negative gap, reference/area-average sampling, and exact
  composite usage.
- Render tests prove canonical PNG/SVG parity and that consumers never repair
  topology. Stage-scoped validation artifacts cover grid/dispersion Voronoi,
  two-/three-guide faces, scale/gap, area averaging, and both composite filters.
- Exercise and directly inspect immutable `assets/raster-sample.png` at
  1024×1024 and `assets/vector-sample.svg` at 900×620. Retain raw SVG and
  rasterized inspection PNGs; inspect native RGB and alpha separately.
- Run affected-package format checks, focused tests, strict Clippy,
  `scripts/validate_architecture.sh`, protected-path review, immutable-asset
  hashes, and semantic-map reconciliation. No GTK/Wayland run is required;
  compile-check only any unavoidable mechanical app adapter.
- For Stages 20P–20S, stop each stage at **Implemented awaiting review** and
  again at **Ready for user acceptance**. Do not commit, push, publish, or
  begin the next stage without explicit authorization.

## Later roadmap

- **Stage 21 — Pattern-authoring GTK:** Pattern Wizard, gallery browsing,
  capability-driven pages, region/composite editors, Review, nested guide/shape
  editors, progressive disclosure, preview workflow, accessibility, private
  Sway evidence, and eventual human GNOME/Mutter acceptance. GTK projects
  domain capabilities and commands; it owns no pattern semantics.
- **Stage 22 — Headless temporal pipeline:** still-frame abstraction, bounded
  multi-frame decoding, deterministic frame sequences and CLI export, then
  simple start/end transitions. Media backend selection receives its own
  planning decision.
- **Stage 23 — Temporal GTK:** start/end pins appear only for
  descriptor-declared continuous values: density X/Y, rotation, X/Y
  translation, mark size, path thickness, region scale/gap, opacity, color
  components, and sampling gain/bias. Seeds, counts, IDs, algorithms, topology,
  output kinds, and other discrete or pattern-definition settings remain
  static. Arbitrary keyframes, multiple segments, editable curves, timeline
  lanes, and a dope sheet remain excluded.

## Deferred work

Explicit connection masks/walks, TSP, wall complements, wrap-around endpoints,
arbitrary motifs, additional parametric forms, Legacy work, compatibility
adapters, and protected-specification changes remain outside this plan.
