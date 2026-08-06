# Toniator Pattern Schema

**Status:** Normative architecture specification  
**Applies to:** Greenfield Toniator rewrite  
**Related documents:** [ArchitectureSchema.md](ArchitectureSchema.md), [ChannelSchema.md](ChannelSchema.md), [ModuleStructure.md](ModuleStructure.md)

---
Noted exceptions can be found in `Addendum.md`.
---

## 1. Purpose

This document defines the schema used to describe a Toniator pattern independently of any GTK widget, channel color, renderer, export format, or saved-document compatibility layer.

The schema must support:

- Random and density-weighted random site placement.
- Grids composed of one to four guide dimensions.
- Straight, curved, warped, procedural, or user-edited guide curves.
- Sites generated from guide intersections or intervals along guides.
- Marks placed at sites.
- Paths and networks built from sites or guides.
- Mazes, spirals, repeated curves, and related connected structures.
- Regions formed from guide arrangements.
- Ordinary Voronoi cells constructed from whatever sites the selected family produces.
- Reusable region shrinking or expansion based on the Bezziator shrink/grow model.
- Exact canvas coverage at arbitrary rotation, offset, density, and aspect.
- A shared canonical geometry pipeline for preview, PNG, and SVG.

The pattern schema describes **what structural pattern exists and how it is realized**. Per-channel density, rotation, offset, mark size, line thickness, color, opacity, and visibility are defined in `ChannelSchema.md`.

---

## 2. Architectural invariants

1. A pattern begins with exactly one `PatternFamily`: `Grid` or `Random`.
2. The family is responsible for generating the structural source: guides, sites, or both.
3. Site placement belongs entirely to the family.
4. Voronoi never chooses, weights, perturbs, or regenerates sites.
5. The output schema consumes family output and realizes it as marks, connected geometry, or regions.
6. Pattern evaluators must not depend on GTK, Cairo, SVG libraries, file dialogs, or UI state.
7. All user-authored and serialized numeric settings are `f64`.
8. Integer counts, indices, graph identifiers, and topology sizes are derived implementation details only.
9. Preview, PNG, and SVG consume the same evaluated canonical geometry.
10. Coverage is computed before final generation; a finite grid must never be generated and then rotated into the canvas.
11. Pattern geometry must be generated beyond the canvas and clipped back to the exact canvas.
12. Canvas clipping edges may participate in topology but are not rendered unless debug display is enabled.
13. Stochastic behavior is deterministic for the same schema, channel state, source artwork, and seed.
14. Shared mechanisms must live in reusable geometry or pattern infrastructure rather than inside individual named presets.

---

## 3. Top-level schema

```rust
pub struct PatternDefinition {
    pub id: PatternDefinitionId,
    pub name: String,
    pub schema_version: f64,
    pub family: PatternFamily,
    pub output: PatternOutputSchema,
    pub modulation: ModulationSchema,
    pub coverage: CoveragePolicy,
}
```

`PatternDefinition` is structural. It does not contain channel color, opacity, visibility, rotation, offset, density, mark-size response, line-thickness response, or cell-inset response.

Named patterns such as “Rectangular Dots,” “Triangular Dots,” “Random Stippling,” “Maze,” “Curved Lines,” or “Voronoi Cells” should normally be presets over this schema rather than independent renderer implementations.

---

## 4. Pattern family

```rust
pub enum PatternFamily {
    Grid(GridFamily),
    Random(RandomFamily),
}
```

A family produces a `FamilyOutput`.

```rust
pub struct FamilyOutput {
    pub generation_domain: GenerationDomain,
    pub guides: GuideSet,
    pub sites: SiteSet,
}
```

A family may produce:

- Sites only.
- Guides only.
- Both guides and sites.

The downstream output schema declares which parts it requires.

---

## 5. Grid family

### 5.1 Definition

A grid contains one to four guide dimensions.

```rust
pub struct GridFamily {
    pub origin: Point2,
    pub dimensions: Vec<GuideDimension>,
    pub site_generation: GridSiteGeneration,
}
```

Validation rule:

```text
1.0 <= number_of_dimensions <= 4.0
```

The stored schema version and any user-facing dimension-count control are `f64`; evaluation validates that the value represents one of the supported discrete configurations. Internally, the validated dimension collection is represented normally as a vector.

### 5.2 Guide dimension

```rust
pub struct GuideDimension {
    pub id: GuideDimensionId,
    pub guide: GuidePrototype,
    pub baseline_angle_degrees: f64,
    pub phase: f64,
    pub repetition: GuideRepetition,
}
```

Each dimension consists of:

- A guide prototype.
- A baseline angle.
- A phase.
- A repetition strategy.

Examples:

| Grid | Guide dimensions |
|---|---|
| Parallel lines | One straight guide family |
| Rectangular | Straight guides at 0° and 90° |
| Rhombic | Straight guides at two non-orthogonal angles |
| Triangular | Straight guides at 0°, 60°, and 120° |
| Four-direction | Straight guides at 0°, 45°, 90°, and 135° |
| Curvilinear | Two or more curved guide families |
| Warped | Curves whose local tangent and normal vary over the canvas |

The canvas remains two-dimensional. “Dimension” means one independently repeated guide family, not a spatial dimension.

### 5.3 Guide prototypes

```rust
pub enum GuidePrototype {
    Straight(StraightGuide),
    Bezier(BezierGuide),
    Polyline(PolylineGuide),
    Arc(ArcGuide),
    Spiral(SpiralGuide),
    Procedural(ProceduralGuide),
    UserPath(UserPathReference),
}
```

Guide prototypes are defined in local pattern coordinates.

A guide prototype must support:

- Bounds estimation.
- Tangent evaluation.
- Normal evaluation.
- Arc-length evaluation or approximation.
- Transformation.
- Clipping.
- Repetition support declaration.
- Coverage-envelope calculation.

### 5.4 Guide repetition

```rust
pub enum GuideRepetition {
    Single,

    TransformStack {
        spacing: f64,
        direction: StackDirection,
    },

    Tile {
        transform: AffineTransform2D,
    },

    NormalOffset {
        spacing: f64,
        sides: OffsetSides,
        cleanup: OffsetCleanup,
    },
}
```

#### `Single`

Uses one guide instance.

Typical uses:

- A spiral.
- A single user-edited path.
- A contour that is sampled or stroked directly.

#### `TransformStack`

Creates repeated copies with a transform, usually translation.

Properties:

- Every copy preserves the prototype’s exact shape.
- Gaps may become nonuniform around curved guides.
- Coverage is achieved by extending the stack until the padded generation domain is fully crossed.

#### `Tile`

Repeats a bounded guide or motif using an affine tile transform.

Properties:

- Supports translation, rotation, scale, or combined transforms.
- Tile indices are generated for every transformed motif envelope intersecting the padded generation domain.

#### `NormalOffset`

Builds successive offset curves at a requested perpendicular distance.

Properties:

- Intended to preserve approximately uniform gaps.
- Based conceptually on Bezziator’s shrink/grow behavior.
- May introduce cusps, self-intersections, splits, or collapsed loops.
- Must use reusable crossing dissolution and topology cleanup.
- Must never silently stop while leaving uncovered canvas areas.

---

## 6. Grid site generation

The grid family determines how sites are derived from its guides.

```rust
pub enum GridSiteGeneration {
    None,

    Intersections {
        dimensions: DimensionSelection,
        merge_epsilon: f64,
        jitter: JitterSchema,
    },

    AlongGuides {
        dimensions: DimensionSelection,
        interval_mode: GuideIntervalMode,
        phase: f64,
        jitter: JitterSchema,
        include_endpoints: bool,
    },

    Combined {
        sources: Vec<GridSiteSource>,
        merge_epsilon: f64,
    },
}
```

### 6.1 Intersections

Intersections may use any selected guide dimensions.

Requirements:

- Two or more dimensions are required.
- Intersections are computed on generated guide instances, not only prototypes.
- Coincident intersections are merged using `merge_epsilon`.
- Intersection sites retain provenance identifying the contributing guides and local parameters.
- Sites outside the visible canvas may be retained as guard sites for downstream topology.

### 6.2 Along-guide sites

```rust
pub enum GuideIntervalMode {
    UniformArcLength {
        nominal_interval: f64,
    },

    DensityMetric {
        longitudinal_multiplier: f64,
    },
}
```

Requirements:

- Sampling is based on arc length, not curve parameter spacing.
- Jitter may be tangential, normal, or two-dimensional.
- Each site retains guide identity, guide-order position, and arc-length position.
- These metadata allow downstream path and network realization without reconstructing ordering.

### 6.3 Jitter

```rust
pub enum JitterSchema {
    None,

    Tangential {
        maximum: f64,
        seed: f64,
    },

    Normal {
        maximum: f64,
        seed: f64,
    },

    TwoDimensional {
        maximum: f64,
        seed: f64,
    },
}
```

All serialized values, including seeds, are `f64`. Before use by a random-number generator, a seed is normalized and deterministically converted to the internal generator’s integer seed representation.

---

## 7. Random family

```rust
pub struct RandomFamily {
    pub distribution: RandomDistribution,
    pub seed: f64,
    pub weighting: SiteWeighting,
}
```

```rust
pub enum RandomDistribution {
    Uniform,
    PoissonDisk {
        minimum_separation_multiplier: f64,
    },
    BlueNoise {
        minimum_separation_multiplier: f64,
        relaxation_iterations: f64,
    },
}
```

### 7.1 Site weighting

For Toniator, “weighting” means **artwork-weighted site placement**.

```rust
pub enum SiteWeighting {
    Uniform,

    ArtworkDensity {
        strength: f64,
        polarity: Polarity,
        response_curve: ResponseCurve,
        field: SamplingField,
    },
}
```

Rules:

- Weighting affects where sites are generated.
- Weighting does not create weighted Voronoi or power cells.
- After generation, sites are ordinary points.
- `strength = 0.0` means uniform placement.
- `strength = 1.0` means maximum configured artwork response.
- The family still honors the channel’s continuous density metric and coverage domain.

---

## 8. Site set and provenance

```rust
pub struct SiteSet {
    pub sites: Vec<Site>,
}
```

```rust
pub struct Site {
    pub id: SiteId,
    pub position: Point2,
    pub scope: SiteScope,
    pub provenance: SiteProvenance,
}
```

```rust
pub enum SiteScope {
    Canvas,
    Guard,
}
```

```rust
pub enum SiteProvenance {
    Random {
        sequence_position: f64,
    },

    GuideIntersection {
        contributors: Vec<GuideIntersectionReference>,
    },

    AlongGuide {
        guide_id: GuideInstanceId,
        normalized_arc_position: f64,
        absolute_arc_position: f64,
        sequence_position: f64,
    },
}
```

Provenance is required because downstream realization may need:

- Stable ordering.
- Guide adjacency.
- Maze graph construction.
- Path reconstruction.
- Deterministic selection.
- Debug visualization.
- Repeatable edits.

---

## 9. Pattern output schema

```rust
pub enum PatternOutputSchema {
    Marks(MarkOutputSchema),
    Connected(ConnectedOutputSchema),
    Regions(RegionOutputSchema),
}
```

This layer describes how family output becomes visible geometry.

---

## 10. Mark output

```rust
pub struct MarkOutputSchema {
    pub site_source: MarkSiteSource,
    pub prototype: MarkPrototype,
    pub orientation: MarkOrientation,
}
```

```rust
pub enum MarkSiteSource {
    FamilySites,
    GuideIntersections,
    AlongGuides,
}
```

Normally, `FamilySites` consumes the family’s already evaluated `SiteSet`. The alternative variants may be retained only when the pattern definition intentionally exposes multiple site subsets.

```rust
pub enum MarkPrototype {
    Circle,
    Ellipse,
    Rectangle,
    Polygon {
        sides: f64,
    },
    UserShape(UserShapeReference),
}
```

Mark size, per-channel rotation response, and color are not stored here. They are channel-instance settings.

---

## 11. Connected output

```rust
pub struct ConnectedOutputSchema {
    pub source: ConnectionSource,
    pub topology: ConnectionTopology,
}
```

```rust
pub enum ConnectionSource {
    Guides,
    FamilySites,
}
```

```rust
pub enum ConnectionTopology {
    GuidePaths {
        direction: TraversalDirection,
        close_paths: bool,
    },

    SequentialSites {
        ordering: SiteOrdering,
        close_path: bool,
    },

    NeighborGraph {
        strategy: NeighborStrategy,
    },

    Maze {
        graph_source: GraphSource,
        algorithm: MazeAlgorithm,
        seed: f64,
    },

    SpanningTree {
        graph_source: GraphSource,
        algorithm: TreeAlgorithm,
    },
}
```

Examples:

- Curves: `Guides` + `GuidePaths`.
- Spiral: one spiral guide + `GuidePaths`.
- Maze: grid-derived sites + adjacency graph + maze topology.
- Random network: random sites + neighbor graph.
- Connected stipple: family sites + sequential or nearest-neighbor topology.

Line thickness and opacity remain channel-instance settings.

---

## 12. Region output

```rust
pub struct RegionOutputSchema {
    pub source: RegionSource,
    pub treatment: RegionTreatment,
}
```

```rust
pub enum RegionSource {
    GuideArrangementFaces {
        dimensions: DimensionSelection,
    },

    Voronoi,
}
```

### 12.1 Guide arrangement faces

Guide curves are split at intersections and assembled into closed faces.

Requirements:

- Canvas boundaries may be inserted as synthetic topology edges.
- Synthetic edges close edge regions but are not rendered by default.
- Face winding is normalized.
- Degenerate and sub-threshold regions are discarded deterministically.

### 12.2 Voronoi

Voronoi has no site-generation settings.

Evaluation is:

```text
Family sites
→ ordinary Euclidean Voronoi construction
→ canvas intersection
→ optional region treatment
```

Rules:

1. Voronoi consumes the complete family-generated `SiteSet`.
2. Canvas and guard sites both participate in tessellation.
3. The Voronoi implementation does not know whether sites came from:
   - Uniform random placement.
   - Artwork-density-weighted random placement.
   - Grid intersections.
   - Curved or warped grid intersections.
   - Along-guide intervals.
   - Jittered sites.
4. No power-diagram or weighted-Voronoi behavior is required.
5. Only cells intersecting the visible canvas continue to output.
6. Canvas clipping edges are synthetic unless debug visualization is enabled.

---

## 13. Region treatment and Bezziator-style shrink/grow

```rust
pub enum RegionTreatment {
    Full,

    Offset {
        direction: OffsetDirection,
        base_amount: f64,
        cleanup: RegionOffsetCleanup,
        collapse_policy: RegionCollapsePolicy,
        canvas_edge_policy: CanvasEdgeOffsetPolicy,
    },
}
```

```rust
pub enum OffsetDirection {
    Inset,
    Outset,
}
```

The channel provides the final modulation response controlling how much each cell is inset or expanded. The pattern definition determines only whether and how region offsetting is structurally supported.

### 13.1 Required reusable operation

```rust
pub trait RegionOffsetter {
    fn offset_region(
        &self,
        region: &Region,
        signed_distance: f64,
        options: &RegionOffsetOptions,
    ) -> Result<RegionSet, RegionOffsetError>;
}
```

### 13.2 Topology process

The implementation should follow this conceptual sequence:

```text
Generate candidate offset curves
→ compute crossings
→ split curves at crossings
→ classify segments by winding and interior status
→ dissolve invalid crossing branches
→ assemble surviving closed loops
→ discard insignificant loops
→ normalize winding
```

This operation belongs in reusable geometry infrastructure and may be reused by:

- Voronoi cells.
- Guide-derived cells.
- Arbitrary closed shapes.
- Contour regions.
- Expanded paths converted to regions.
- Future pattern families.

### 13.3 Collapse policy

```rust
pub enum RegionCollapsePolicy {
    Remove,
    ClampToMinimumArea {
        minimum_area: f64,
    },
    PreserveSiteMark {
        minimum_radius: f64,
    },
}
```

Default: `Remove`.

---

## 14. Modulation schema

```rust
pub struct ModulationSchema {
    pub field: SamplingField,
    pub response_curve: ResponseCurve,
    pub polarity: Polarity,
    pub sampling: SamplingStrategy,
}
```

```rust
pub enum SamplingStrategy {
    AtSite,
    AlongPath,
    RegionCentroid,
    RegionAverage,
    RegionMedian,
    RegionMinimum,
    RegionMaximum,
}
```

The pattern definition declares the supported modulation strategy. The channel instance supplies output-specific ranges such as:

- Minimum and maximum mark size.
- Minimum and maximum line thickness.
- Minimum and maximum region inset.

---

## 15. Coverage policy

```rust
pub struct CoveragePolicy {
    pub margin: CoverageMargin,
    pub clipping: ClipPolicy,
    pub boundary_visibility: BoundaryVisibility,
}
```

```rust
pub enum CoverageMargin {
    Automatic,
    Explicit {
        amount: f64,
    },
}
```

```rust
pub enum BoundaryVisibility {
    Hidden,
    Debug,
}
```

### 15.1 Coverage invariant

All structural generation occurs over a padded domain outside the canvas. Final geometry is clipped to the exact canvas.

The padded margin must include:

```text
maximum mark radius
+ half maximum stroke width
+ maximum jitter
+ antialiasing support
+ topology-specific guard distance
```

### 15.2 Channel transform and inverse-domain planning

Coverage planning receives the channel’s density metric, rotation, and offset.

Required order:

```text
Inflate visible canvas
→ inverse-transform padded canvas into pattern-local coordinates
→ resolve directional frequencies and guide repetition
→ generate guides and sites over the complete local domain
→ transform generated structure into document coordinates
→ realize marks, connections, or regions
→ clip to exact canvas
```

Prohibited order:

```text
Generate finite unrotated grid
→ rotate it
→ discover uncovered corners
```

### 15.3 Straight-guide analytical coverage

For a straight guide family with direction `d` and normal `n`:

1. Project all inverse-transformed padded-canvas corners onto `n`.
2. Determine minimum and maximum projection.
3. Resolve the guide spacing from the channel density metric.
4. Generate every required guide offset from the first floor index through the last ceiling index, inclusively.
5. Extend each guide along `d` far enough to cross the full padded domain.

### 15.4 Curved-guide coverage

Every curved repetition implementation must provide a coverage planner.

```rust
pub trait GuideCoveragePlanner {
    fn plan(
        &self,
        local_domain: &GenerationDomain,
        support_radius: f64,
    ) -> Result<GuideCoveragePlan, CoverageError>;
}
```

A planner must either:

- Prove the generated guide envelope covers the padded domain, or
- Return a validation error.

It must not return partial geometry with silent edge gaps.

---

## 16. Density interpretation

The pattern definition does not store literal pixel spacing.

Every family receives the channel’s continuous two-dimensional density metric:

```rust
pub struct DensityMetric2D {
    pub across_x: f64,
    pub across_y: f64,
    pub aspect_locked: bool,
}
```

Examples for a 900 × 600 source canvas:

```text
across_x = 90.0
across_y = 60.0
```

This represents equal nominal spacing when aspect is locked.

### 16.1 Grid interpretation

- Two-guide rectangular grids may correspond approximately to 90 columns and 60 rows.
- One-guide grids interpret the metric in local tangent and normal directions.
- Three- and four-guide grids resolve an effective spacing for each guide normal.
- Curved guides evaluate the density metric against their local tangent and normal.

### 16.2 Random interpretation

Random families derive an expected site density from the same metric. A common isotropic estimate is:

```text
expected_site_count ≈ across_x × across_y
```

The actual generated collection size is an internal integer derived from the authored floating-point target.

### 16.3 Directional frequency

Given nominal axis spacing:

```text
spacing_x = canvas_width / across_x
spacing_y = canvas_height / across_y
```

For a guide unit normal `n = (nx, ny)`, effective frequency is derived from the density metric:

```text
frequency(n) = sqrt((nx / spacing_x)^2 + (ny / spacing_y)^2)
```

Effective guide spacing is:

```text
guide_spacing = 1 / frequency(n)
```

Under isotropic spacing, every direction receives equal document-space spacing.

---

## 17. Validation

The validator must reject, at minimum:

- A grid with fewer than one or more than four dimensions.
- Intersections with fewer than two selected guide dimensions.
- A mark output requiring family sites when no sites are generated.
- A connected output requiring guide ordering when no guides exist.
- A maze without a graph-capable source.
- Guide arrangement faces without sufficient intersecting structure.
- Voronoi with an empty site set.
- A normal-offset repetition on a guide that cannot be offset.
- Non-finite numeric values.
- Zero or negative density.
- Invalid response ranges.
- Unresolvable coverage plans.
- Unsupported schema-version transitions.

Validation errors must be surfaced before rendering.

---

## 18. Serialization

Requirements:

- All authored numeric settings serialize as floating-point values.
- Pattern IDs and enum discriminants serialize as stable strings.
- Schema versions are explicit.
- Unknown fields are preserved when practical or rejected clearly.
- Migrations are deterministic and tested.
- Presets use the same schema as documents.
- UI defaults are not hidden sources of persisted behavior.
- No renderer-specific state is persisted.

---

## 19. Evaluation sequence

```text
PatternDefinition
+ ChannelPatternInstance
+ SourceArtwork
+ Canvas
        ↓
Validate schema
        ↓
Resolve density metric and channel transform
        ↓
Plan padded local generation domain
        ↓
Evaluate PatternFamily
        ↓
Produce GuideSet and SiteSet
        ↓
Evaluate PatternOutputSchema
        ↓
Apply channel-specific size/thickness/inset response
        ↓
Create canonical geometry
        ↓
Attach channel color and opacity
        ↓
Clip to exact canvas
        ↓
Preview / PNG / SVG
```

---

## 20. Canonical examples

| Named result | Family | Family site generation | Output |
|---|---|---|---|
| Rectangular dots | Grid, 0° + 90° | Intersections | Marks |
| Triangular dots | Grid, 0° + 60° + 120° | Intersections | Marks |
| Curved-grid dots | Grid, curved dimensions | Intersections | Marks |
| Jittered line dots | Grid | Along guides with jitter | Marks |
| Random stippling | Random uniform | Family sites | Marks |
| Weighted stippling | Random artwork-weighted | Family sites | Marks |
| Curved line screen | Grid | Guides and optional along-guide samples | Connected guide paths |
| Spiral | Grid with one single spiral guide | Along guide | Connected guide path |
| Grid maze | Grid | Intersections | Connected maze |
| Random network | Random | Family sites | Connected neighbor graph |
| Curvilinear cells | Grid | Guide arrangement | Regions |
| Grid Voronoi | Grid | Family sites | Voronoi regions |
| Random Voronoi | Random | Family sites | Voronoi regions |
| Warped Voronoi | Curved grid | Curved-guide sites | Voronoi regions |

---

## 21. Initial implementation order

1. Schema types and serialization.
2. Validation.
3. Density metric.
4. Channel transform and inverse-domain coverage planning.
5. Straight guide prototypes.
6. One- and two-dimension guide generation.
7. Intersections and site provenance.
8. Mark output.
9. Preview/PNG/SVG canonical parity.
10. Three- and four-dimension guides.
11. Curved guide prototypes.
12. Transform stacks.
13. Normal-offset guide repetition.
14. Along-guide sampling and jitter.
15. Random uniform distribution.
16. Artwork-density-weighted random distribution.
17. Connected guide paths.
18. Site graphs and maze topology.
19. Guide arrangement faces.
20. Ordinary Voronoi.
21. Reusable region offset and crossing dissolution.
22. Schema-driven pattern editor.

---

## 22. Acceptance criteria

The pattern architecture is acceptable when:

- A 900 × 600 document can request 90.0 across X and 60.0 across Y without storing a 10-pixel spacing.
- Rotating, offsetting, or changing aspect recomputes generation and leaves no edge gaps.
- A three-guide triangular grid uses the same family infrastructure as a two-guide grid.
- Curved guide intersections can drive marks or Voronoi without a new site-placement implementation.
- Random and weighted-random sites can drive marks, networks, or Voronoi without changing the Voronoi implementation.
- Voronoi contains no distribution settings.
- Preview, PNG, and SVG use identical canonical geometry.
- Cell shrinking uses the same region-offset infrastructure for Voronoi and guide-derived cells.
- Every stochastic result is repeatable.
- Every stage can be tested headlessly without GTK.
