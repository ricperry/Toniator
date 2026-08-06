# Toniator Rewrite Schema Addendum

**Status:** Normative addendum  
**Applies to:** `ArchitectureSchema.md`, `PatternSchema.md`, `ChannelSchema.md`, and `ModuleStructure.md`  
**Purpose:** Record decisions made after the initial four schema documents were generated. This addendum supersedes conflicting language in those documents but does not modify them.

---

## 1. Numeric types

The original documents stated too broadly that all serialized numeric values should use `f64`.

The corrected rule is:

> Use `f64` for continuous authored values. Use conventional discrete types for discrete values.

### Continuous `f64` values

Examples include:

- Density and density aspect.
- Rotation.
- X/Y translation or phase.
- Shape size.
- Curve, line, or network thickness.
- Region or cell inset/outset.
- Opacity and color components.
- Sampling gain and bias.
- Minimum spacing and visible-mark margin.
- Cluster scale, spread, and strength.
- Correlated-field scale and strength.
- Canvas dimensions.
- Animation interpolation progress and duration.

### Discrete values

Examples include:

- Random seeds: `u32`.
- Number of guide dimensions: integer.
- Tile width and height: integer.
- Sequence periods and repeat counts: integer.
- Signed connection-walk steps: signed integer.
- Frame indices and frame counts: integer.
- Site, guide, graph, edge, face, and topology identifiers: integer or stable ID types.
- Realized collection sizes: `usize` or another suitable integer type.
- Enum choices and Boolean flags: enum or `bool`.

---

## 2. Connected-output programs

Connected output must support user-authored connection behavior as well as generative topology.

```rust
pub enum ConnectionTopology {
    Explicit(ExplicitConnectionProgram),
    Generative(GenerativeConnectionProgram),
}
```

### 2.1 Repeating per-dimension connection masks

Compact text can define repeating enabled/disabled edges along guide dimensions.

Example:

```text
X1X0 | Y0Y1
```

Meaning:

- X: connect, skip, repeat.
- Y: skip, connect, repeat.

Suggested parsed form:

```rust
pub struct RepeatingConnectionMask {
    pub clauses: Vec<DimensionConnectionMask>,
}

pub struct DimensionConnectionMask {
    pub dimension: GuideDimensionId,
    pub sequence: Vec<bool>,
    pub phase: u32,
}
```

Parser requirements:

- Ignore insignificant whitespace.
- Treat axis aliases case-insensitively.
- Support `X` and `Y` for the first two guide dimensions.
- Support `D1` through `D4`.
- Repeat each mask indefinitely.
- Reject empty masks or values other than `0` and `1`.
- Preserve normalized source text for editing and serialization.

Example for more than two dimensions:

```text
D1:1010 | D2:0101 | D3:1000
```

### 2.2 Explicit motif/path walks

Compact signed steps can describe a walk through the family-generated site graph.

Example:

```text
x4y3x-2y-1x1y-1x1y-1x-2y3x4y-4
```

Suggested schema:

```rust
pub struct MotifWalkProgram {
    pub sequence_text: String,
    pub tile: ConnectionTile,
    pub repeat: TileRepeat,
    pub origin: TileOrigin,
}

pub struct ConnectionStep {
    pub dimension: GuideDimensionId,
    pub delta: i32,
}

pub struct ConnectionTile {
    pub width: u32,
    pub height: u32,
}

pub enum TileRepeat {
    None,
    AlongX,
    AlongY,
    BothAxes,
}
```

Tile dimensions and repeat direction must be explicit rather than inferred only from the walk.

The sequence follows graph adjacency, not literal screen-horizontal or screen-vertical directions. On a curved grid, `X4` means four adjacency steps along the first guide dimension.

### 2.3 Generative topology

```rust
pub enum GenerativeConnectionProgram {
    Maze {
        algorithm: MazeAlgorithm,
        seed: u32,
    },

    SpanningTree {
        algorithm: TreeAlgorithm,
        seed: u32,
    },

    RandomWalk {
        seed: u32,
        length: u32,
    },
}
```

Family and connection seeds are distinct:

- Family seed controls site placement.
- Connection seed controls selected graph connections.

### 2.4 Crossing treatment

Connection selection and crossing rendering are separate.

```rust
pub enum CrossingTreatment {
    Junction,
    PassThrough,
    AlternateOverUnder,
    Dissolve,
}
```

This is required for weave, Aztec, maze, and multi-guide connected patterns.

---

## 3. Canvas boundaries must not form topology

The canvas boundary must never be used to:

- Close cells or regions.
- Create graph edges.
- Complete Voronoi cells.
- Create guide segments.
- Form maze boundaries.
- Insert synthetic topology.
- Create visible pattern edges.

The rule is:

> Generate the pattern as though the canvas boundary does not exist, extend complete pattern structure beyond the visible area, then clip the final canonical geometry to the exact canvas.

### 3.1 Coverage process

```text
Canvas bounds
→ estimate off-canvas generation extent
→ generate complete guides, sites, connections, cells, and regions
→ apply mark sizing, stroke thickness, and region treatment
→ construct final canonical geometry
→ clip preview/export to the canvas
```

### 3.2 Structural guard depth

```rust
pub struct CoveragePolicy {
    pub guard_steps: u32,
    pub additional_margin: f64,
}
```

Recommended default:

```text
guard_steps = 2
```

One guard step means:

| Structure | Guard step |
|---|---|
| Repeated guides | One guide interval |
| Along-guide sites | One site interval |
| Random sites | One equivalent neighborhood radius |
| Network | One adjacency layer |
| Voronoi | One outer ring of family-generated sites |
| Tiled motif | One complete tile |
| Offset curves | One repeated/offset curve |

The continuous support margin must also include maximum mark radius, half stroke width, jitter, region expansion, antialiasing support, and topology-specific neighborhood requirements.

### 3.3 Voronoi

```text
Family-generated visible and guard sites
→ ordinary Voronoi construction
→ optional region treatment
→ final canvas clipping
```

The canvas polygon is not part of cell formation.

### 3.4 Guide-derived regions

Complete faces must be formed outside the canvas. If a face is incomplete before clipping, coverage planning failed. The canvas must not close it.

### 3.5 Hard dependency rule

No topology-building function may receive the canvas as an edge source. The canvas is only:

- A hint for calculating generation extent.
- The final clipping boundary.

---

## 4. First-class headless CLI

Toniator must provide a complete command-line frontend that uses the same core pipeline as GTK.

```text
Shared core
├── domain
├── geometry
├── sampling
├── patterns
├── rendering
├── persistence
└── validation

Frontends
├── toniator-app
└── toniator-cli
```

The CLI exists for deterministic production, automated testing, and AI-agent development without GTK or Wayland interaction.

### 4.1 Standard conventions

At minimum:

```text
-i, --input
-o, --output
-h, --help
--version
```

Errors go to stderr and commands return meaningful exit codes.

### 4.2 Inputs

The CLI should accept:

- Still images.
- Multi-frame images.
- Image sequences.
- Video.
- Toniator documents.
- General presets.
- Pattern presets.

Examples:

```bash
toniator render -i source.png -o output.svg --pattern rectangular-dots
```

```bash
toniator render -i document.toniator -o output.png
```

### 4.3 Output format

Infer format from output extension:

```text
.png → PNG
.svg → SVG
```

Unsupported extensions fail clearly.

### 4.4 Presets and overrides

Patterns need not be authored from scratch in CLI syntax. The CLI must select pattern/general presets and override channel values.

Recommended syntax:

```text
--channel <channel>.<property> <value>
```

Examples:

```bash
--channel C.visible true
--channel C.color "#00ffff"
--channel C.opacity 0.85
--channel C.density-x 90.0
--channel C.density-y 60.0
--channel C.aspect-locked true
--channel C.rotation 15.0
--channel C.offset-x 3.0
--channel C.offset-y -5.0
--channel C.shape-size-min 0.1
--channel C.shape-size-max 1.0
```

### 4.5 RGB/CMYK mode

```bash
--mode rgb
--mode cmyk
```

Mode controls channel defaults, source mapping defaults, and PNG background defaults.

### 4.6 PNG background

- RGB PNG default: solid black.
- CMYK PNG default: solid white.
- `--transparent`: omit the solid background.

### 4.7 Suggested subcommands

```text
toniator render
toniator validate
toniator inspect
toniator patterns
toniator presets
```

### 4.8 Precedence

Recommended deterministic precedence:

```text
Built-in defaults
→ RGB/CMYK defaults
→ general preset
→ pattern preset
→ loaded document
→ CLI channel overrides
```

### 4.9 CLI acceptance rule

Every primary output producible through the GUI must also be producible headlessly using presets and channel overrides, without GTK.

---

## 5. Multi-frame and video operation

The source subsystem should unify still and moving media.

```rust
pub enum SourceMedia {
    StillImage(StillImageSource),
    AnimatedImage(AnimatedImageSource),
    ImageSequence(ImageSequenceSource),
    Video(VideoSource),
}
```

```rust
pub trait FrameSource {
    fn metadata(&self) -> SourceMediaMetadata;

    fn frame_at(
        &mut self,
        position: TimelinePosition,
    ) -> Result<SourceFrame, SourceError>;
}
```

The pattern evaluator receives a decoded `SourceFrame` and must not depend on the source container type.

### 5.1 Frame-sequence output

For a multi-frame source, write numbered frames:

```text
frame-000000.svg
frame-000001.svg
frame-000002.svg
```

or:

```text
frame-000000.png
frame-000001.png
frame-000002.png
```

Example:

```bash
toniator render \
  -i input.mp4 \
  -o frames/frame-%06d.svg \
  --start-frame 0 \
  --end-frame 299 \
  --fps 30.0
```

Useful arguments:

```text
--frame
--start-frame
--end-frame
--fps
--start-time
--end-time
```

---

## 6. Simple start/end animation

Do not implement a fully featured timeline or arbitrary keyframe tracks.

Animation consists of:

- Start value.
- End value.
- One interpolation/easing mode.
- The render job's duration or frame range.

```rust
pub enum AnimatedValue<T> {
    Constant(T),

    Transition {
        start: T,
        end: T,
        interpolation: Interpolation,
    },
}
```

```rust
pub enum Interpolation {
    Hold,
    Linear,
    QuadraticIn,
    QuadraticOut,
    SmoothStep,
    SmoothInOut,
}
```

### 6.1 Animatable channel settings

Continuous channel settings may transition:

- Density X/Y.
- Rotation.
- X/Y offset.
- Shape size.
- Curve/network thickness.
- Region/cell inset.
- Opacity.
- Color components.
- Sampling gain and bias.

Pattern-definition settings remain static for a render job.

Discrete settings should normally remain static. If animated, they use `Hold`.

No initial requirement exists for:

- Arbitrary keyframes.
- Multiple segments.
- Editable curves.
- Timeline lanes.
- Dope sheets.

---

## 7. Curated random site processes

The current implementation labels Uniform, Gaussian, Pink Noise, Blue Noise, and Poisson Disc while producing identical point layouts. The rewrite must not repeat this.

Separate:

```text
Base site process
+ spatial modulation
+ exclusion/collision policy
```

### 7.1 Base site process

Recommended curated set:

```rust
pub enum RandomCharacter {
    RawRandom,

    EvenRandom {
        quality: EvenRandomQuality,
    },

    Clustered {
        cluster_density: f64,
        cluster_spread: f64,
        cluster_strength: f64,
    },
}
```

#### Raw Random

Independent random placement with natural clusters, gaps, and potentially close neighbors. Advanced option.

#### Even Random

Poisson-disc or another genuinely even process with enforced separation and blue-noise-like visual quality. Recommended default.

#### Clustered

Intentional islands and open spaces with explicit cluster scale, spread, and strength.

### 7.2 Terminology

- **Blue noise** is a spectral quality, not one unique point generator.
- **Poisson-disc** is one common construction that can produce blue-noise-like layouts.
- Do not expose both as distinct options unless implementations genuinely differ.
- **Pink noise** is a correlated field, not a complete point process.
- **Gaussian** must name a defined construction such as Gaussian clustering, displacement, or field modulation.

### 7.3 Density modulation

```rust
pub enum DensityModulation {
    Uniform,

    ArtworkWeighted {
        strength: f64,
        response: ResponseCurve,
    },

    CorrelatedField {
        field: CorrelatedField,
        strength: f64,
        scale: f64,
    },
}
```

```rust
pub enum CorrelatedField {
    Pink,
    Brown,
    Procedural,
}
```

A correlated field may explicitly modulate:

- Site density.
- Site displacement.
- Mark orientation or irregularity.
- Connection probability or direction.
- Region inset or roughness.

### 7.4 Exclusion policy

```rust
pub enum SiteExclusion {
    None,

    CenterDistance {
        minimum: f64,
    },

    MarkMargin {
        margin: f64,
        sizing: ExclusionSizingPolicy,
    },
}
```

Size-aware exclusion requires:

```text
distance(site_i, site_j)
>= support_radius_i + support_radius_j + margin
```

Suggested policies:

```rust
pub enum ExclusionSizingPolicy {
    NominalSize,
    MaximumPossibleSize,
    LocallySampledSize,
}
```

Recommended default:

- Even Random.
- Small positive visible-mark margin.
- Locally sampled size where practical.
- Deterministic `u32` seed.

### 7.5 Unsatisfiable density

If density conflicts with minimum spacing, Toniator must preserve the exclusion constraint and report the achieved density instead of silently permitting overlap.

---

## 8. Presets must use exposed controls only

Every bundled preset must be reproducible from a blank pattern definition using controls exposed in the pattern editor.

No preset may invoke:

- Hidden pattern-specific code.
- Private variables.
- A special renderer.
- Evaluator branching based on preset name.

Prohibited:

```rust
match preset_name {
    "plasma" => render_plasma_special_case(),
    _ => ...
}
```

Required:

```text
Preset file
→ ordinary pattern schema
→ ordinary family output
→ ordinary realization
→ canonical geometry
```

Deleting a preset removes only the shortcut, not the underlying capability.

### 8.1 Schema-adequacy targets

The pattern editor must be capable of constructing:

- Grain.
- Petroglyph.
- Plasma.
- Pebbles.
- Pointillism 1.
- Pointillism 2.

These are adequacy tests, not hidden named algorithms.

### 8.2 Required exposed controls

#### Random family

- Site-process type.
- Density.
- `u32` seed.
- Minimum center spacing.
- Minimum visible-mark margin.
- Cluster scale/strength.
- Correlated-field scale/strength.
- Displacement/warping.
- Relaxation/evenness quality.

#### Marks

- Primitive or user shape.
- Size response.
- Aspect variation.
- Orientation variation.
- Shape irregularity.
- Contour roughness.
- Site subset.
- Treatment of unconnected sites.

#### Connected output

- Graph source.
- Explicit connection sequence.
- Generative topology.
- Generative `u32` seed.
- Path length.
- Turn and branch behavior.
- Smoothing and pruning.
- Caps, joins, and corners.
- Crossing treatment.
- Whether unused sites remain as marks.

#### Regions

- Voronoi or guide-derived cells.
- Inset/outset.
- Rounding/smoothing.
- Boundary roughness.
- Fill/outline.
- Polarity.
- Collapse behavior.

---

## 9. Composite output layers

A single exclusive output enum may be insufficient for patterns that mix isolated marks, connected fragments, paths, and regions.

Suggested revision:

```rust
pub struct PatternDefinition {
    pub family: PatternFamily,
    pub outputs: Vec<PatternOutputLayer>,
}
```

```rust
pub struct PatternOutputLayer {
    pub source_filter: SourceFilter,
    pub realization: PatternRealization,
    pub ordering: i32,
}
```

```rust
pub enum PatternRealization {
    Marks(MarkOutputSchema),
    Connected(ConnectedOutputSchema),
    Regions(RegionOutputSchema),
}
```

This allows configurations such as:

```text
Plasma
├── connected organic fragments
└── residual isolated marks
```

```text
Petroglyph
├── thick connected paths
└── disconnected glyph fragments
```

```text
Pebbles
├── inset Voronoi regions
└── optional outlines or residual marks
```

All layers still use exposed schema controls and shared canonical geometry.

---

## 10. Channel-editor invalidation

Channel-editor edits must rebuild the correct pipeline layers.

### 10.1 Family regeneration

These must regenerate guides/sites and every dependent stage:

- Density X/Y.
- Density aspect or lock.
- Rotation.
- X/Y translation or phase.
- Canvas dimensions.
- Channel-level placement seed.
- Any setting affecting site placement.

```text
Family guides/sites
→ graph, connections, or cells
→ modulation
→ canonical geometry
→ preview/export
```

Grid translation must update the pattern coordinate frame and regenerate sufficient off-canvas structure. It must not move already clipped geometry.

### 10.2 Realization rebuild

These should reuse family sites/guides where possible:

- Shape size.
- Shape orientation response.
- Curve/network thickness.
- Region/cell inset.
- Geometry response curve or polarity.

### 10.3 Presentation only

These must not regenerate family geometry:

- Color.
- Opacity.
- Visibility.

### 10.4 Source invalidation

These require source resampling and dependent evaluation:

- Source frame/image.
- Source mapping.
- Sampling field.
- Gain/bias when used for placement or geometry.

### 10.5 Invalidation contract

```rust
pub enum InvalidationLevel {
    Presentation,
    Realization,
    Family,
    Source,
}
```

Every authoritative command returns its invalidation level.

```rust
pub struct CommandResult {
    pub affected_channels: Vec<ChannelId>,
    pub invalidation: InvalidationLevel,
}
```

### 10.6 Stale results

Every evaluation carries a document revision.

```text
Revision N evaluation starts
→ edit creates revision N+1
→ result for N is discarded
```

Stale geometry must never be displayed or exported.

---

## 11. Module-structure additions

Add a dedicated CLI crate:

```text
crates/
└── toniator-cli/
    ├── Cargo.toml
    └── src/
        ├── main.rs
        ├── args.rs
        ├── commands/
        │   ├── render.rs
        │   ├── validate.rs
        │   ├── inspect.rs
        │   ├── patterns.rs
        │   └── presets.rs
        ├── overrides/
        │   ├── channel.rs
        │   ├── mode.rs
        │   └── animation.rs
        ├── frame_range.rs
        ├── output_pattern.rs
        └── exit_codes.rs
```

Add source-media support:

```text
toniator-sampling/src/media/
├── still.rs
├── animated_image.rs
├── image_sequence.rs
├── video.rs
└── frame_source.rs
```

Add modules for:

- Connection mask/walk parsing.
- Curated random processes.
- Exclusion/collision enforcement.
- Simple interpolation.
- Explicit invalidation and revision tracking.

The CLI must not depend on GTK or libadwaita.

---

## 12. Required acceptance tests

### Numeric types

- Continuous values serialize as `f64`.
- Seeds serialize as `u32`.
- Guide counts and frame indices use integers.

### Connection programs

- `X1X0 | Y0Y1` parses and repeats correctly.
- Signed motif walks parse correctly.
- Tile dimensions and repeat direction round-trip.
- Curved-grid walks follow guide adjacency.
- Connection seed changes topology without moving sites.

### Canvas independence

- No topology operation consumes canvas edges.
- Guard generation naturally closes cells/faces.
- Final clipping is clean.
- Rotation and translation leave no edge gaps.

### CLI

- `--help`, `-i`, and `-o` work conventionally.
- Output extension chooses PNG/SVG.
- RGB PNG defaults black.
- CMYK PNG defaults white.
- `--transparent` removes background.
- All primary channel settings are headlessly overridable.
- GTK and CLI produce equivalent canonical geometry.

### Multi-frame

- Video frame selection is deterministic.
- Frame count and output numbering are correct.
- Start/end interpolation is correct.
- Rendering has no GTK/Wayland dependency.

### Random processes

- Raw, Even, and Clustered layouts are measurably distinct.
- Minimum center distance is enforced.
- Size-aware margin is enforced.
- Correlated modulation visibly affects its selected target.
- Same seed produces same sites.
- Impossible density reports achieved output.

### Preset reconstruction

For Grain, Petroglyph, Plasma, Pebbles, and Pointillism 1/2:

1. Build from a blank definition using exposed controls.
2. Save as a normal preset.
3. Reload it.
4. Render through CLI.
5. Confirm no preset-name-specific code path exists.

### Invalidation

- Density, rotation, and translation regenerate family output.
- Shape size reuses sites but rebuilds realization.
- Color and opacity do not regenerate sites.
- Stale evaluations are discarded.

---

## 13. Revision checklist for the original documents

### `PatternSchema.md`

Add or revise:

- Correct numeric types.
- Explicit connection masks and motif walks.
- Seeded generative topology.
- Canvas-independent topology and guard steps.
- Curated random processes and exclusion.
- Correlated modulation.
- Composite output layers.
- Presets as exposed-control configurations.
- Grain/Petroglyph/Plasma/Pebbles/Pointillism adequacy tests.

### `ChannelSchema.md`

Add or revise:

- Correct numeric types.
- Simple start/end transitions.
- Multi-frame evaluation.
- Invalidation levels.
- Family regeneration for density, rotation, and translation.
- Stale-result rejection.
- CLI mapping for primary channel settings.

### `ArchitectureSchema.md`

Add or revise:

- CLI as a first-class frontend.
- Video/multi-frame source abstraction.
- Simple animation model.
- Canvas exclusion from topology.
- Composite outputs.
- Curated random-process architecture.
- Strong cache/invalidation contract.
- Presets as pure schema.

### `ModuleStructure.md`

Add:

- `toniator-cli`.
- Frame-source/media modules.
- Animation/interpolation support.
- Connection parsers.
- Random-process/exclusion modules.
- Invalidation/revision infrastructure.
- CLI, multi-frame, random-layout, and preset-reconstruction tests.

---

## 14. Consolidated rules

1. Continuous authored values use `f64`; discrete values use discrete types.
2. Pattern definitions are static structural configurations.
3. Channel settings control density, transform, geometry response, appearance, and simple start/end animation.
4. Families own guide and site generation.
5. Voronoi only constructs cells from family sites.
6. Canvas boundaries never form topology.
7. Complete pattern structure extends beyond the canvas and is clipped only at final output.
8. Connected output supports masks, motif walks, and seeded generative programs.
9. Random layouts are curated, visibly distinct, and collision-aware.
10. Every preset is reproducible using exposed editor controls.
11. Composite output layers may combine marks, connections, and regions.
12. Channel edits invalidate exactly the required pipeline layers.
13. CLI and GTK use the same core pipeline.
14. Still images, multi-frame images, image sequences, and video use one frame-source abstraction.
15. Headless rendering is sufficient for automated development and testing.

---

## 15. Source-alpha interpretation

Decoded source pixels retain their raw straight (unassociated) sRGBA values,
including hidden RGB where alpha is zero. Realization derives its mark response
at the sampling boundary, before bilinear interpolation:

- For the color-derived Luminance component, `L` is Rec.709 luminance of linear
  RGB, opaque ink is `1 - L`, and effective ink is `alpha * (1 - L)` per source
  sample. These effective-ink values are bilinearly interpolated, so hidden RGB
  at alpha zero cannot create a fringe.
- For the independent Alpha component, alpha itself is bilinearly interpolated
  and the existing Alpha response is applied once: mark ink is `1 - alpha`.
  It is not multiplied by alpha a second time.

SVG decoder output must be unpremultiplied to retain straight RGBA; the normal
precision and zero-alpha caveats of that conversion remain applicable. Raw
hidden RGB remains available for source inspection, but does not affect the
alpha-associated color-derived mark response.
