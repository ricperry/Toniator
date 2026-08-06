Toniator should not organize the halftone patterns primarily as a flat list of patterns. There are really several **pattern-generation families**, each with different inputs and invariants.

## 1. Structured fields

These begin with one or more mathematically defined families of curves. Their intersections or spacing determine where marks are placed.

Examples:

* rectangular grid;
* triangular grid;
* angled grid;
* curved grid;
* warped grid;
* dot waves;
* mesh patterns.

A better abstraction than “grid” may be **field lattice** or **intersection field**.

Conceptually:

```text
Field A: repeated curves
Field B: repeated curves
Placement rule: intersections, crossings, cells, or nearest approach
Mark rule: dot, shape, stroke, or local segment
```

A conventional square grid is merely:

```text
Field A: parallel horizontal lines
Field B: parallel vertical lines
Relative angle: 90°
```

A triangular grid could use three line families, often separated by 60°, while more experimental patterns could use curved or warped line families.

Useful parameters would include:

* field count;
* angle per field;
* spacing;
* phase or offset;
* curvature;
* distortion;
* intersection rule;
* mark primitive;
* value-to-size mapping.

This family should not assume straight lines or 90° relationships.

## 2. Stochastic distributions

These do not have a regular lattice. They produce sites, marks, or cells from a deterministic pseudorandom process.

Examples:

* film grain;
* Poisson or blue-noise points;
* random Voronoi cells;
* weighted stippling;
* clustered points;
* aperiodic distributions.

“Grid” is not really the right term here. I would call these **distributions** or **site distributions**.

Their core contract is different:

```text
Distribution algorithm
+ domain
+ density field
+ random seed
= deterministic set of sites
```

Common parameters:

* seed;
* density;
* minimum separation;
* clustering;
* jitter;
* weighting by source value;
* relaxation iterations;
* boundary behavior.

The seed must be part of the saved document and export state. Ideally the output should be reproducible from:

```text
algorithm version + parameters + seed + source data
```

Voronoi is slightly layered because it starts with a site distribution and then derives cells from those sites:

```text
site generator → Voronoi construction → cell mark/fill rule
```

That separation matters. It would let Toniator combine:

* random sites with Voronoi cells;
* blue-noise sites with Voronoi cells;
* weighted sites with power diagrams;
* grid-based sites with Voronoi cells.

## 3. Parametric paths

These are generated from an explicit mathematical path or path family rather than from a lattice or random distribution.

Examples:

* spiral;
* concentric contours;
* sinusoidal paths;
* radial waves;
* procedural contour lines.

A spiral is defined by parameters such as:

* center;
* radius range;
* angular span;
* radial growth law;
* spacing;
* rotation;
* phase.

The marks may then be sampled along the path or the path itself may become a variable-width halftone stroke.

I would call this family **parametric paths**.

Conceptually:

```text
Path equation
+ path spacing
+ sampling rule
+ value modulation
= pattern geometry
```

## 4. Constructive or hybrid patterns

Maze-like patterns belong here. They combine structured topology, local rules, and deterministic randomness.

Examples:

* right-angle maze;
* randomized line network;
* paver pattern;
* clustered lattice;
* cellular mesh;
* branching paths.

A maze might use:

```text
base lattice
+ connectivity rules
+ seeded random decisions
+ path extraction
```

This is neither purely grid-based nor purely random. The grid supplies the allowable topology, while randomness selects connections or removes walls.

I would call these **constructive patterns** or **rule-based patterns**.

## A useful architecture

Rather than one provider interface with a large number of optional fields, Toniator could use a small hierarchy:

```text
PatternProvider
├── StructuredFieldProvider
├── StochasticDistributionProvider
├── ParametricPathProvider
└── ConstructivePatternProvider
```

Each concrete pattern then belongs to the closest family:

```text
StructuredFieldProvider
├── RectangularGrid
├── TriangularGrid
├── AngledGrid
├── CurvedGrid
└── MeshWave

StochasticDistributionProvider
├── FilmGrain
├── PoissonPoints
├── BlueNoise
├── WeightedStippling
└── VoronoiSites

ParametricPathProvider
├── Spiral
├── CurvedLineField
├── DotWave
└── ProceduralContours

ConstructivePatternProvider
├── OrthogonalMaze
├── PaverPattern
├── ClusteredGrid
└── RandomizedMesh
```

## Separate placement from rendering

There is another useful decomposition underneath those families:

1. **Generator** — creates sites, paths, cells, or topology.
2. **Sampler** — obtains source values from the image.
3. **Mark mapper** — converts value into size, width, density, or coverage.
4. **Geometry builder** — emits dots, paths, polygons, or cells.
5. **Renderer/exporter** — draws or serializes the result.

For example:

```text
Triangular line fields
→ intersections
→ sample luminance at each intersection
→ map luminance to dot area
→ emit circles
```

Or:

```text
Seeded blue-noise sites
→ Voronoi cells
→ sample average value over each cell
→ map value to inset area
→ emit inset cell polygons
```

Or:

```text
Archimedean spiral
→ sample source along path
→ map value to stroke width
→ emit variable-width path
```

This decomposition would make unusual combinations possible without implementing every combination as a separate monolithic pattern.

## Likely terminology

I would use:

* **Structured Fields**
* **Stochastic Distributions**
* **Parametric Paths**
* **Constructive Patterns**

Then reserve “grid” for the specific structured cases where there actually is a lattice.

That terminology is broad enough for the patterns you have described without forcing Voronoi, grain, spirals, and mazes into the same conceptual bucket.

