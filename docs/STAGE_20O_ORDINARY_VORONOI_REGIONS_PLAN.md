# Stage 20O — Ordinary Voronoi Regions

Status: **Complete at implementation checkpoint
`7ab97f01ec372ab1e6201b3913742476a1511c02`** (user acceptance recorded
2026-08-25; independent re-review and final artifact inspection passed).

Stage 20O adds the first concrete canonical-region output. A `Regions` output
references one site-producing mechanism and realizes ordinary, unweighted
Voronoi cells from its complete guard-inclusive `FamilySiteSet`. The canvas is
only the final relevance/clip boundary: it never supplies a topology edge or
closes an unbounded cell.

The output is deliberately fixed to `RegionGeometryResponse::Full`; treatment,
gap, scale, region sampling, composites, and GTK remain out of scope. The
Stage 20N one-output validation gate and schema-v5/preset-v3 current-only
boundary remain active.

## Public contracts and eligibility

- A `RegionSourceIntent::VoronoiSites { site_mechanism_id }` is valid only
  when the resolved product is a `FamilySiteSet` produced by that exact
  mechanism ID.
- Guide intersections, sites along guides, `AlongParametricCurveSites`, and
  random/dispersion products are eligible. A direct `ParametricPaths` product
  is not: curve spacing/phase is resolved once by the site family, not by
  Voronoi.
- Spade 2.15.1 is geometry-private. Toniator owns duplicate co-ownership,
  canonicalization, coverage, limits, cancellation, diagnostics, identities,
  and fingerprints.
- `PatternGeometryResponse::Regions(RegionGeometryResponse::Full)` is the
  sole keyed base/effective response. There is intentionally no Region channel
  delta or editable descriptor in 20O; future scale/gap treatment is 20Q.
- `PatternStructureRecipe::VoronoiRegions { definition }` wraps an ID-free
  site-family recipe. Materialization first allocates that family, then replaces
  its single output atomically with `PatternOutputLayer::Regions`; the source
  uses the allocated site-product mechanism ID. The current one-output gate
  remains unchanged.
- Eligibility follows `TypedFamilyOutput::site_set()`, not provenance:
  intersection, along-guide, `AlongParametricCurveSites { interval, phase }`,
  and random/dispersion products are eligible. Direct `ParametricPaths` has no
  site set and fails before topology allocation.
- The current ID-free recipe grammar intentionally has no parametric-family
  constructor. Typed current-schema v5 definitions may nevertheless persist
  and evaluate `AlongParametricCurveSites` Regions; adding recipe grammar is a
  later authorization, not a frontend adapter or a fallback topology source.

## Geometry contract

- Normalize signed zero, sort normalized positions and site IDs, and group
  only exact duplicate positions. Every sorted group owner co-owns one cell;
  near duplicates stay separate.
- Insert the complete family into the private triangulation in deterministic
  group order. Extract finite cells only. A relevant unbounded cell fails
  `region.voronoi.coverage.unbounded`; no canvas clipping creates a cell.
- Canonical regions use `CanonicalRegionSourceId::SiteOwners`, preserving the
  Stage 20N ordered identity, ring validation, positive winding, bounds,
  analytic area, and fingerprint authority.
- Defaults are 1,048,576 site groups, 4,194,304 topology edges, 1,048,576
  regions, 8,388,608 boundary points, and 67,108,864 inspections. Cancellation
  is `evaluation.cancelled`; no partial candidate may publish.
- Source groups, unique undirected edges, retained components, retained line
  points, and all grouping/insertion/traversal/relevance/canonical inspections
  share those limits. The canonical builder receives only the remaining
  inspection budget and its diagnostics are added to the producer count.
- Finite cells are retained only after exact polygon/canvas intersection or
  containment tests. An unbounded cell is relevant if its owner is in the
  canvas, a canvas corner has it as nearest owner, or one of its finite boundary
  segments or outward Spade dual rays intersects the canvas; relevant unbounded cells fail
  `region.voronoi.coverage.unbounded`. Canvas edges never close a cell.
- Stable producer diagnostics are `region.voronoi.identity.*`, `.insertion.*`,
  `.geometry.*`, `.coverage.*`, `.limits.*`, and `.allocation.*`; cancellation
  remains exactly `evaluation.cancelled`.

## Persistence, realization, and verification

- Persist only Regions source intent and fixed keyed response in v5/v3; no
  cells, diagnostics, limits, caches, or scheduler state are serialized.
- Evaluate one shared family, then cache the Region output independently using
  its output ID, site mechanism, family identity, contract, response, and
  limits. A one-output aggregate retains the output fingerprint unchanged.
- Region support is output-specific. Ordinary site families request the authored
  `additional_margin`; an `AlongParametricCurveSites` family additionally
  requests `guard_steps * maximum_nominal_cell_diameter`. This compensates for
  single parametric curves, whose generic repetition spacing is zero, so guard
  sites survive the family support envelope. The resulting request support is
  already part of family evaluation/cache identity; it does not expand emitted
  topology at the canvas boundary.
- Region identity includes `toniator.voronoi.spade-2.15.1.v1`, output and site
  mechanism IDs, family fingerprint, normalized exact duplicate/co-owner
  grouping, and ordered canonical rings/bounds/areas. Limits and diagnostics
  are excluded. Cache keys include the same contract and configured limits.
- Spade is pinned in the workspace as exactly `=2.15.1` with only `std`, used
  privately by `toniator-geometry`. Toniator selects its Apache-2.0 license;
  the verbatim upstream text is retained at
  `THIRD_PARTY_LICENSES/spade-2.15.1/LICENSE-APACHE`. The published crate has
  no NOTICE, so none is fabricated.
- Renderers consume complete canonical regions as solid nonzero fills and apply
  only the final canvas clip.
- Focused tests cover response/bundle validation, recipe and persistence,
  exact duplicate ownership, parametric-site eligibility, coverage, limits,
  cancellation, cache/stale atomicity, and PNG/SVG parity. Artifacts under
  `target/validation/stage20o/` exercise the immutable 1024×1024 raster and
  900×620 SVG inputs. The supplemental typed parametric document is 360×240:
  its fixed 12-turn spiral is guard-complete there, while the 900×620 trial
  correctly reports relevant unbounded coverage. The manifest records that
  bounded-performance rationale alongside direct duplicate and off-canvas
  canonical witnesses.
- Required gates are focused 20O domain/geometry/patterns/engine/IO tests plus
  affected 20A/20G/20H/20N foundations, affected-package format/check/strict
  Clippy, architecture/protected-path review, immutable-asset hashes, and
  semantic-map reconciliation. Validation artifacts include native PNG, raw
  SVG, SVG-rasterized PNG, hashes, RGB/alpha statistics, and a manifest for
  grid raster, dispersion SVG, parametric sites, duplicate ownership, and
  off-canvas coverage. Parent visual inspection completed before this Ready
  status.

## Explicit non-goals

No weighted/power Voronoi, guide faces, region scale/gap/full treatments beyond
fixed Full, region sampling, composites, heterogeneous authoring, filters,
wall complements, topology repair, schema migration, GTK, temporal behavior,
or Stage 20P+ work enters this stage.

The writer stopped first at **Implemented awaiting review**. Independent
read-only re-review passed after bounded repairs, and the parent visually
inspected the final native PNG/SVG-raster representatives with user cell
confirmation. User acceptance on 2026-08-25 completes the stage at
implementation checkpoint `7ab97f01ec372ab1e6201b3913742476a1511c02`. This
does not authorize publication or Stage 20P, which remains separately gated.
