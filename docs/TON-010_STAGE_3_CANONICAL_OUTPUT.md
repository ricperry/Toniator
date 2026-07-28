# TON-010 Stage 3: canonical pattern output

Stage 3 defines the runtime output contract shared by preview, PNG, and SVG.
It is deliberately an in-memory rendering contract, not a second persisted
pattern authority. `Document.pattern_state` remains the sole persisted pattern
selection and parameter state; Shapes and Curves still project through the
existing `RenderVariant` compatibility adapter.

## Algebra

`CanonicalPatternOutput` has four intentionally distinct single-family forms
plus a typed composition form:

- `Marks(MarkPatternOutput)` preserves the existing `MarkSet` exactly.
- `Paths(PathPatternOutput)` preserves the existing `CurveGeometry` exactly.
- `Regions(RegionPatternOutput)` represents filled polygonal cells, including
  compound paths and holes.
- `Network(NetworkPatternOutput)` represents shared boundary topology through
  stable node and edge identities.
- `Composite(CompositePatternOutput)` composes a region family and a network
  family in that order, preserving both semantic types for cell-plus-boundary
  patterns. It is not a bag of untyped geometry.

Future generators choose the form that describes their semantics; they do not
flatten paths, cells, and networks into a universal geometry bag. Region and
network identities are stable only inside one generated output. They are not
document IDs and are never persisted selector authority.

## Coordinates, geometry, and clipping

Every output has an explicit positive `ArtboardSpace`. Coordinates use a
top-left origin with positive Y down; `(0, 0)` is the top-left artboard corner.
Geometry may extend outside the artboard. Raster consumers clip at the output
pixmap boundary and SVG consumers attach the common artboard clip path.

Regions contain one or more rings. Their declared winding is validated against
the Y-down coordinate system: clockwise has positive signed area and
counter-clockwise has negative signed area. A region explicitly chooses
`NonZero` or `EvenOdd`; callers use `EvenOdd` when hole semantics must not
depend on a particular ring orientation. Region and network transforms are
finite, invertible affine matrices applied before preview/export scaling.

## Composition and polarity

Layers have stable IDs, optional stable output-channel identity, labels,
deterministic `(order, id)` ordering, explicit color, opacity, and
Multiply/Screen blend behavior. Regions and edges are also deterministically
sorted by `(order, id)` inside their layer.

`GeometryPolarity::Subtractive` is alpha-mask semantics. Raster consumers use
destination-out composition; SVG uses a black shape/stroke in an explicit
layer mask. It is never represented by drawing a stroke in the export or
preview background colour. SVG keeps positive geometry in named editable
groups and preserves compound path fill rules.

## Consumers and limits

`generate_document_pattern_output_cancellable` is the canonical document
generation seam. Preview and document-output rendering consume it, so PNG
continues through its existing output path. `canonical_pattern_png_bytes` also
encodes an already-generated output for fixture and future generator use. SVG
consumes the same output before serializing its established mark/path forms.
This preserves existing Shapes and Curves geometry and output behavior while
giving regions/networks first-class raster and SVG consumers.

Consumers checkpoint cancellation before allocation and within bounded loops.
Validation rejects invalid coordinates, transforms, references, winding,
opacity, and duplicate IDs. Default limits cap layers, regions, vertices,
nodes, and edges; output pixels remain capped at 64 megapixels. Generators must
validate their output before expensive consumers and should stream or otherwise
bound their own intermediate work.

## Deliberate boundary

This stage adds no new visible pattern, schema-driven editor, persisted output
geometry, Weighted Voronoi generation, or obsolete-format migration. Those
features must build on this runtime contract in their separately authorized
stages.
