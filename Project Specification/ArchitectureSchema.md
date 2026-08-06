# Toniator Architecture Schema

**Status:** Normative architecture specification  
**Applies to:** Greenfield Toniator rewrite  
**Related documents:** [PatternSchema.md](PatternSchema.md), [ChannelSchema.md](ChannelSchema.md), [ModuleStructure.md](ModuleStructure.md)

---
Noted exceptions can be found in `Addendum.md`.
---

## 1. Purpose

This document defines the architectural contract for a clean Toniator rewrite.

The project must not inherit Toniator v1’s (see /home/ricperry1/projects/Toniator/ToniatorLegacy/ for v1's source code) overlapping state authorities, mode-specific render paths, compatibility adapters, or UI-owned pattern logic. The rewrite should selectively port proven algorithms and user-visible behavior only after they are isolated, tested, and adapted to the new interfaces.

The architecture is organized around one pipeline:

```text
Source artwork
→ Authoritative document state
→ Pattern definition
→ Channel pattern instance
→ Family generation
→ Pattern realization
→ Canonical geometry
→ Render scene
→ Preview / PNG / SVG
```

---

## 2. Core principles

### 2.1 One authoritative document

There is one writable document state.

No secondary writable state may exist in:

- Render variants.
- GTK widgets.
- Preview adapters.
- Export adapters.
- Pattern evaluators.
- Specialized editors.
- Compatibility layers.

### 2.2 Structural pattern versus channel instance

`PatternDefinition` describes structural behavior.

`ChannelPatternInstance` describes how one channel uses that structure:

- Density.
- Rotation.
- Offset.
- Size, thickness, or inset response.
- Color.
- Opacity.
- Visibility.
- Source mapping.

### 2.3 Pure evaluation

Pattern family generation and realization should be pure whenever practical.

Given identical:

- Pattern definition.
- Channel instance.
- Source field.
- Canvas.
- Seed.

the evaluator must return identical canonical geometry.

### 2.4 One canonical output path

Preview, PNG, and SVG all consume the same canonical geometry.

They may differ in rasterization and format-specific encoding, but they must not reinterpret pattern settings.

### 2.5 Reuse by composition

Named patterns are compositions of:

```text
Pattern family
+ site or guide generation
+ output realization
+ modulation
+ channel geometry response
```

The system should avoid one renderer module per named pattern.

### 2.6 Coverage before clipping

The generator computes a padded domain, generates beyond the visible canvas, and clips afterward.

Rotation, offset, density, aspect, guide geometry, mark radius, line width, jitter, and topology requirements all participate in coverage planning.

### 2.7 Continuous authored values

All user-authored and serialized numeric settings are `f64`.

Integer topology and collection values exist only internally after validation and derivation.

---

## 3. Domain model

```rust
pub struct Document {
    pub id: DocumentId,
    pub schema_version: f64,

    pub canvas: CanvasSpec,
    pub source: SourceArtwork,
    pub output: OutputSettings,

    pub pattern_definitions: PatternDefinitionStore,
    pub channels: Vec<ChannelState>,
}
```

### 3.1 Canvas

```rust
pub struct CanvasSpec {
    pub width: f64,
    pub height: f64,
    pub background: BackgroundSpec,
}
```

Canvas dimensions are document-space values, not necessarily physical pixels.

### 3.2 Source artwork

```rust
pub struct SourceArtwork {
    pub reference: SourceReference,
    pub interpretation: SourceInterpretation,
}
```

The source subsystem exposes sampling fields to evaluators but does not own patterns.

### 3.3 Pattern definition store

```rust
pub struct PatternDefinitionStore {
    pub definitions: Vec<PatternDefinition>,
}
```

Definitions may be shared by multiple channels.

### 3.4 Channels

Each channel references a pattern definition and owns its channel instance and appearance state.

---

## 4. Dependency flow

Allowed dependency direction:

```text
domain
├── geometry
├── sampling
├── patterns
├── rendering
├── persistence
└── app
```

More precisely:

```text
toniator-domain
        ↓
toniator-geometry
        ↓
toniator-sampling
        ↓
toniator-patterns
        ↓
toniator-render
        ↓
toniator-io
        ↓
toniator-app
```

Some sibling crates may share lower-level dependencies, but no lower-level crate may depend on a higher-level one.

### 4.1 Forbidden dependencies

- Domain must not depend on GTK, libadwaita, Cairo, SVG writers, image widgets, or filesystem dialogs.
- Geometry must not depend on GTK or document persistence.
- Patterns must not depend on GTK, preview widgets, or exporter implementations.
- Rendering must not mutate document state.
- IO must not contain pattern mathematics.
- App must not implement geometric algorithms that belong in geometry or patterns.

---

## 5. Command architecture

All document mutations are commands.

```rust
pub trait DocumentCommand {
    fn validate(&self, document: &Document) -> Result<(), CommandError>;
    fn apply(&self, document: &mut Document) -> Result<CommandResult, CommandError>;
}
```

Command results identify invalidation:

```rust
pub struct CommandResult {
    pub invalidation: InvalidationSet,
    pub affected_channels: Vec<ChannelId>,
}
```

### 5.1 Undo and redo

Undo records authoritative state transitions or reversible command data.

Requirements:

- Undo does not depend on widget history.
- Undo restores shared-definition references correctly.
- Undo invalidates the same pipeline layers as the original edit.
- Coalescing is permitted for continuous controls but must preserve deterministic final state.

---

## 6. Evaluation architecture

### 6.1 Evaluation request

```rust
pub struct EvaluationRequest {
    pub document_revision: f64,
    pub channel_id: ChannelId,
    pub quality: EvaluationQuality,
}
```

### 6.2 Evaluation context

```rust
pub struct EvaluationContext<'a> {
    pub canvas: &'a CanvasSpec,
    pub source_fields: &'a SourceFieldSet,
    pub pattern: &'a PatternDefinition,
    pub channel: &'a ChannelState,
}
```

### 6.3 Stages

```text
Validate
→ Resolve source field
→ Resolve density metric
→ Resolve pattern-local transform
→ Plan padded generation domain
→ Generate family guides/sites
→ Realize marks/connections/regions
→ Apply channel geometry response
→ Build canonical geometry
→ Clip to canvas
→ Build render layer
```

### 6.4 Caching

Caches are derived and disposable.

Potential cache boundaries:

- Decoded source.
- Source sampling fields.
- Family output.
- Realized topology.
- Canonical geometry.
- Raster preview.

Cache keys must include every authoritative input relevant to the cached layer.

No cache may become writable authority.

---

## 7. Pattern architecture

The pattern system follows `PatternSchema.md`.

Formal structure:

```rust
pub struct PatternDefinition {
    pub family: PatternFamily,
    pub output: PatternOutputSchema,
    pub modulation: ModulationSchema,
    pub coverage: CoveragePolicy,
}
```

### 7.1 Family output

```rust
pub struct FamilyOutput {
    pub guides: GuideSet,
    pub sites: SiteSet,
    pub generation_domain: GenerationDomain,
}
```

### 7.2 Output realization

```rust
pub enum PatternOutputSchema {
    Marks(MarkOutputSchema),
    Connected(ConnectedOutputSchema),
    Regions(RegionOutputSchema),
}
```

### 7.3 Voronoi boundary

Voronoi is only a region constructor:

```text
Family-generated sites
→ ordinary Voronoi
→ canvas clipping
→ optional reusable region offset
```

Voronoi has no density, weighting, seed, grid, or site-generation settings.

---

## 8. Geometry architecture

The geometry layer owns reusable mathematical structures and algorithms.

### 8.1 Core primitives

```text
Point2
Vector2
Rect
Bounds
AffineTransform2D
Angle
Polyline
BezierPath
ClosedPath
Region
Mark
GuideCurve
Site
Graph
Cell
```

### 8.2 Required operations

```text
Transforms
Bounds
Arc-length sampling
Curve intersections
Segment intersections
Guide repetition
Normal offsets
Polygon and curve clipping
Region boolean operations
Winding normalization
Topology assembly
Crossing dissolution
Voronoi construction
Neighbor graphs
Spatial indexing
Geometry simplification
```

### 8.3 Region offset

Bezziator’s shrink/grow behavior is ported as a reusable geometry operation, not a Voronoi-specific feature.

Required behavior:

- Inset and outset.
- Curved boundaries.
- Crossing detection.
- Crossing dissolution.
- Region splitting.
- Region collapse.
- Winding normalization.
- Deterministic cleanup.

---

## 9. Sampling architecture

The sampling layer converts source artwork into deterministic fields.

```rust
pub struct SourceFieldSet {
    pub fields: Vec<SamplingFieldData>,
}
```

Supported fields may include:

- CMYK channels.
- RGB channels.
- Alpha.
- Luminance.
- Derived masks.
- Future user-defined fields.

Sampling must support:

- Point sampling.
- Bilinear or higher-quality interpolation.
- Region statistics.
- Along-path sampling.
- Density-weighted random placement.
- Response curves.
- Polarity.

Sampling is independent of GTK and export formats.

---

## 10. Canonical geometry

```rust
pub enum GeometryOutput {
    Marks(Vec<CanonicalMark>),
    Paths(Vec<CanonicalPath>),
    Regions(Vec<CanonicalRegion>),
}
```

```rust
pub struct RenderScene {
    pub canvas: CanvasSpec,
    pub layers: Vec<RenderLayer>,
}
```

```rust
pub struct RenderLayer {
    pub channel_id: ChannelId,
    pub visible: bool,
    pub color: ColorValue,
    pub opacity: f64,
    pub geometry: GeometryOutput,
}
```

Canonical geometry contains geometry and style semantics needed by all renderers.

It does not contain GTK widgets, Cairo contexts, SVG XML, or temporary preview state.

---

## 11. Rendering architecture

### 11.1 Preview renderer

Consumes `RenderScene`.

Responsibilities:

- Interactive display.
- Zoom and pan.
- Quality staging.
- Overlay support.
- Source/render comparison.
- Debug visualization.

### 11.2 Raster renderer

Consumes `RenderScene`.

Responsibilities:

- PNG output.
- Antialiasing settings.
- Background or transparency.
- Resolution and scaling.
- Color compositing.

### 11.3 SVG renderer

Consumes `RenderScene`.

Responsibilities:

- Vector path generation.
- Groups and layer order.
- Fill and stroke semantics.
- Opacity.
- Clipping paths.
- Document-space dimensions.

No renderer may regenerate sites or reinterpret density.

---

## 12. UI architecture

GTK/libadwaita is confined to the app layer.

Recommended shell:

```text
AdwApplication
└── MainWindow
    └── AdwOverlaySplitView
        ├── Canvas area
        └── Inspector
```

Inspector hierarchy:

```text
Document
Source
Output
Channels
    └── Selected channel editor
Pattern
    └── Edit Pattern…
Canvas / Presentation
```

### 12.1 Channel editor

Defined in `ChannelSchema.md`.

### 12.2 Pattern editor

The pattern editor edits structural schema only.

It may be:

- Schema-generated for ordinary controls.
- Specialized for guide or curve editing.
- Modal or modeless according to UX testing.

A specialized editor proposes schema changes. It does not mutate document state directly.

### 12.3 Blueprint resources

UI definitions should use Blueprint/GResource with:

- Explicit ownership.
- Stable widget IDs.
- Focus and accessibility tests.
- No business logic embedded in resource files.

---

## 13. Persistence architecture

Persistence owns:

- Document serialization.
- Preset serialization.
- Schema migration.
- Source references.
- Recovery files.
- Recent-document metadata.

Persistence does not own:

- Pattern evaluation.
- Geometry generation.
- UI state beyond explicitly persisted preferences.

### 13.1 Versioning

Every major schema has an explicit version:

- Document schema.
- Pattern schema.
- Channel schema.
- Preset schema.

Migrations must be:

- Deterministic.
- Tested.
- One-directional.
- Independent of UI defaults.

---

## 14. Error architecture

Errors must preserve stage and context.

```rust
pub enum ToniatorError {
    Validation(ValidationError),
    Coverage(CoverageError),
    Sampling(SamplingError),
    Pattern(PatternError),
    Geometry(GeometryError),
    Rendering(RenderError),
    Persistence(PersistenceError),
}
```

Requirements:

- No silent fallback that changes pattern semantics.
- No partial output presented as successful completion.
- Recoverable preview failures do not corrupt the document.
- Validation errors identify the specific schema path.

---

## 15. Concurrency

Evaluation may run off the GTK main thread.

Requirements:

- Authoritative document mutations occur through controlled commands.
- Evaluators receive immutable snapshots.
- Results carry the originating document revision.
- Stale results are discarded.
- Cancellation is supported for superseded evaluations.
- Determinism is preserved regardless of thread scheduling.

---

## 16. Testing strategy

### 16.1 Unit tests

- Density conversion.
- Transform inversion.
- Coverage planning.
- Guide repetition.
- Intersections.
- Random determinism.
- Site weighting.
- Voronoi construction.
- Region offset and crossing dissolution.
- Serialization and migration.

### 16.2 Property tests

- No non-finite geometry.
- No uncovered canvas after declared coverage success.
- Deterministic stochastic output.
- Transform round trips.
- Valid winding after region cleanup.
- Clipped output remains within canvas bounds.

### 16.3 Golden fixtures

- Canonical geometry.
- SVG.
- PNG.
- Representative saved documents.
- Representative presets.

### 16.4 UI tests

- Command dispatch.
- Focus and accessibility.
- Channel/pattern editor binding.
- Undo and redo.
- Visibility and color updates.
- Stale-preview rejection.

---

## 17. Development sequence

### Phase 0 — Contract

- Architecture documents.
- Type sketches.
- Dependency rules.
- Architecture decision records.

### Phase 1 — Application shell

- Workspace.
- GTK application.
- Main window.
- Split view.
- Empty canvas.
- Empty inspector.
- Resource pipeline.

### Phase 2 — Domain and commands

- Document.
- Canvas.
- Channels.
- Pattern definition references.
- Commands.
- Undo/redo.
- Save/load.

### Phase 3 — Canonical render path

- Hard-coded canonical geometry.
- Preview.
- PNG.
- SVG.
- Golden parity tests.

### Phase 4 — First vertical slice

```text
Two straight guide dimensions
→ arbitrary rotation and offset
→ density metric
→ analytical edge coverage
→ intersections
→ circular marks
→ per-channel size response
→ preview / PNG / SVG
```

### Phase 5 — Generalized families

- One to four guide dimensions.
- Random family.
- Weighted random placement.
- Along-guide sites.
- Curved guides.
- Normal-offset repetition.

### Phase 6 — Connected and region outputs

- Guide paths.
- Networks.
- Maze.
- Guide arrangement faces.
- Voronoi.
- Region shrink/grow.

### Phase 7 — Editors and advanced UX

- Schema-generated controls.
- Specialized guide editor.
- Presets.
- Performance tuning.
- Debug visualizations.

---

## 18. Non-goals for the initial rewrite

- Runtime-loaded third-party plugins.
- A stable external plugin ABI.
- Weighted Voronoi or power diagrams.
- Automatic compatibility with every Toniator v1 file.
- Reimplementation of every v1 feature before the first stable vertical slice.
- Pattern-specific renderers.
- UI-driven geometry.
- Hidden compatibility adapters.
- Premature unification with Bezziator or Threshiator repositories.

The architecture may reuse algorithms across projects later, but Toniator should first establish a stable reference implementation.

---

## 19. Acceptance criteria

The architecture is acceptable when:

- A complete pattern can be evaluated headlessly.
- The GTK app contains no pattern mathematics.
- The same canonical geometry drives preview, PNG, and SVG.
- Channel density and transforms produce no edge gaps.
- A random or grid family can drive marks, networks, or Voronoi without duplicating site-generation code.
- Voronoi is a pure site-to-cell operation.
- Region shrink/grow is reusable outside Voronoi.
- All authored numeric values round-trip as floating point.
- Undo/redo modifies only authoritative document state.
- Stale background evaluations cannot overwrite current output.
- A new pattern preset can be introduced primarily by schema composition rather than a new renderer.
