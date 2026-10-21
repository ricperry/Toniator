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
- X/Y translation.
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

## 1A. Parametric curve authority

The original Pattern Schema listed spiral as a `GuidePrototype`. That wording
is superseded: a finite spiral is an explicit `ParametricCurve` family source,
not a Grid guide dimension. The first accepted forms are round and square
spirals. A parametric source may publish exactly one homogeneous structural
product: an ordinary open `CurvePath` for connected realization, or equal-arc
sites for mark realization. It may use the shared finite `CurveRepetition`,
including `NormalOffset`; final canvas clipping remains a consumer concern.

Analytic intent is persisted. Derived curve paths, sites, stroke outlines,
capabilities, and caches are not serialized. Additional parametric forms,
composites, adjacency, and preset-name behavior require later authorization.

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

### 4.5 Halftone channel model, mapping, and paint

The authoritative halftone channel model is selected independently from final
output encoding or background:

```bash
--channel-model rgb
--channel-model cmyk
--channel-model source-color-alpha
```

```rust
pub enum HalftoneChannelModel {
    Rgb,
    Cmyk,
    SourceColorAlpha,
}
```

The model owns an ordered channel topology. Canonical topology uses stable
role IDs and order: Red `1`, Green `2`, Blue `3`; Cyan `4`, Magenta `5`,
Yellow `6`, Black `7`; and SourceColor `8`. A canonical topology factory owns
those roles, IDs, ordering, default source mappings, default layer paint,
visibility, and opacity. The caller supplies one validated
pattern/layout/response template, which the factory clones across roles. The
factory does not own screen angles or other pattern-geometry defaults.

A model change atomically replaces the model and a complete explicit ordered
channel topology. The command validates the supplied topology and never
silently resets, guesses, or migrates settings. Missing, duplicate,
out-of-order, unsupported, or extraneous roles are invalid; channel IDs must
be unique and stable. This transition reports `ChannelTopology` invalidation.

Source mapping and layer paint are separate authoritative concepts. Each
channel owns independent fill-response and weighting source configurations;
neither belongs to its pattern recipe. Each mapping selects a scalar field
from Red, Green, Blue, Cyan, Magenta, Yellow, Black, Alpha, or Luminance and
applies its own source-tone interpretation, inversion, gain and bias:

```text
oriented = inverted ? 1 - value : value
tone = levels, gamma, then centered contrast applied to oriented
clamp(gain * tone + bias)
```

Layer paint supplies either a solid presentation color or, for the
SourceColorAlpha topology, evaluated per-mark source color. Ordinary solid
color, visibility, and opacity changes are presentation invalidations.
Fill mapping and mark-response changes are realization invalidations. Weighting
source/response changes rebuild Family when consumed by the effective pattern;
unused weighting settings remain stored without geometry invalidation. Weighting
also owns its independent strength and response curve. The completed mapped
sample is subject to that consumer's cutoff before downstream response evaluation.

RGB scalar fields are decoded sRGB converted to linear-light `R`, `G`, and
`B`. CMYK scalar fields use deterministic profile-independent normalized
linear-light separation:

```text
K = 1 - max(R, G, B)
if K = 1: C = M = Y = 0
otherwise:
  C = (1 - R - K) / (1 - K)
  M = (1 - G - K) / (1 - K)
  Y = (1 - B - K) / (1 - K)
```

The implementation may use the algebraically equivalent stable forms
`C = 1 - R / max(R,G,B)`, `M = 1 - G / max(R,G,B)`, and
`Y = 1 - B / max(R,G,B)` outside the exact-black guard. This normalized CMY
with pure process paints is the inverse of the fixed linear CMYK transmittance
for source-color fidelity. ICC/profile handling, dot gain, configurable
UCR/GCR, black-generation curves, physical ink simulation, and soft proofing
remain deferred.

Color-derived response fields are associated with source alpha exactly once
before spatial interpolation. Alpha is an independent scalar and is not
alpha-multiplied again.

For SourceColorAlpha, source alpha drives mark response/size exactly once,
sampled source RGB supplies mark color, and channel opacity supplies
presentation opacity. Source-color sampling converts straight sRGB to linear
RGB, associates it with alpha for interpolation, interpolates associated RGB
and alpha, and unassociates RGB when sampled alpha is positive. Positive alpha
does not additionally attenuate mark color or opacity. At exactly zero sampled
alpha, hidden RGB remains inspectable but the mark makes no visible paint
contribution even when the configured minimum mark size is nonzero.

Halftone models have fixed, non-user-selectable composition semantics. Let
each ordered layer first produce its conventional layer-local premultiplied
linear result `(P_i, A_i)` in stable mark order. For fractional mark coverage
`q`, paint alpha `a`, layer opacity `o`, and straight-linear paint `C`, each
mark uses `s = clamp(q * a * o)`, then:

```text
P <- s*C + P*(1-s)
A <- s   + A*(1-s)
```

For antialiased output, a maximal contiguous run of layer outputs with no
per-primitive paints, fully opaque layer color, and full layer opacity joins
its typed positive geometry before coverage is measured. The joined area is
painted once with that layer color. Other outputs retain ordered per-primitive
source-over. Each primitive's own fill rule is applied before joining; the
join does not cross layers or channel-composition boundaries. Antialiasing-off
output retains its hard-edge behavior.

RGB uses additive linear Porter-Duff lighter composition:

```text
P = clamp(sum(P_i))
A = clamp(sum(A_i))
```

CMYK uses idealized subtractive transmittance. With
`C_i = P_i / A_i` for positive `A_i`:

```text
T = product(1 - A_i * (1 - C_i))
A = 1 - product(1 - A_i)
P = T - (1 - A)
```

SourceColorAlpha uses conventional ordered source-over. All three produce
straight sRGBA by returning `P / A` when `A` is positive and transparent black
when `A` is zero. The equations preserve the accepted single-layer result.
They are fixed model semantics, not user-selectable blend modes. ICC profiles,
physical ink simulation, dot gain, configurable UCR/GCR, black-generation
curves, and soft proofing are separate future concerns.

SVG retains vector channel geometry and applies the same model equations with
explicit same-document filter inputs, premultiplied-RGBA arithmetic, and
`color-interpolation-filters="linearRGB"`. It must not rely on unspecified
viewer blending, ordinary source-over for RGB/CMYK, or an embedded raster
substitute.

Final SVG serialization joins maximal contiguous runs of fully opaque geometry
with channel-owned solid paint within each channel as editable NonZero compound
paths. Each primitive retains its own filled meaning before joining; preserve
exact circles and certified simple curves, and normalize other fill topology
per primitive under explicit precision, work-limit, and cancellation checks.
Per-primitive sampled paint and translucent geometry retain their original
ordered vector primitives. This final-consumer operation does not change the
canonical scene, Pattern geometry, calibration, preview, or raster output.

### 4.6 Preview backdrop and export background

Preview backdrop is viewer-only presentation. Default viewer policy is black
for RGB, white for CMYK, and fixed neutral mid-gray for SourceColorAlpha. It
does not modify the document, `RenderScene`, native transparent raster, SVG,
or export background. RGB and CMYK preview targets apply their selected black
or white backdrop in the renderer's linear floating-point result before the
preview's final sRGBA8 encoding, so coverage below an alpha-byte threshold
still contributes visible color. SourceColorAlpha preview targets remain
transparent over the viewer's neutral gray. The preview target and raster
cache identity include this presentation choice; canonical scene identity and
PNG export policy do not.

PNG export background is a final-consumer choice:

```bash
--background transparent
--background black
--background white
```

When the user does not make an explicit choice, the default follows the
authoritative halftone channel model: pure black for RGB, pure white for CMYK,
and transparent for SourceColorAlpha. Black preserves a representative viewing
condition for additive RGB output when a PNG is displayed outside Toniator and
allows the opaque result to be added over another color by a downstream image
compositor. White is the corresponding representative backing for subtractive
CMYK output. An explicit `--background` choice overrides the model-sensitive
default without changing the document, model, canonical scene, scene identity,
or cache identity.

Black or white composites the final transparent scene onto that background.
PNG remains sRGB/sRGBA for every halftone channel model; selecting CMYK never
creates CMYK-encoded PNG data. SVG remains transparent unless a later
explicitly authorized SVG-background feature is added. Gallery thumbnails may
add an explicitly documented presentation background without changing ordinary
SVG export semantics or becoming canonical evaluation output.

PNG rasterization also exposes a user-selectable antialiasing option:

```bash
--antialiasing on|off
```

The default is `on`. `off` produces hard-edged, non-antialiased rasterization.
The application exposes the equivalent export control. This option applies
only to PNG rasterization; SVG output is unchanged. Where a raster output or
raster cache is keyed, the antialiasing choice participates in that identity,
but it does not alter the authoritative `Document`, family, realization,
`RenderScene`, or scene authority.

When a direct still source is opened or rendered without an explicit canvas
override, the app and CLI use the decoded/intrinsic source width and height as
the canvas dimensions and preserve the source aspect ratio. For SVG, the
resolved intrinsic dimensions or `viewBox` dimensions supply those defaults.
An explicit canvas override remains available (for example,
`--canvas <width>x<height>`); it replaces the source-native default only when
the user supplies it.

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
→ halftone-channel topology defaults
→ general preset
→ pattern preset
→ loaded document
→ CLI channel overrides
```

### 4.9 CLI acceptance rule

Every primary output producible through the GUI must also be producible
headlessly using presets and channel overrides, without GTK. `toniator render`
is the authoritative complete-document integration path. `inspect grid` and
`inspect marks` are lower-level structural and single-channel realization
diagnostics, not alternate document-render authorities.

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

- Density and density aspect.
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

    ArtworkWeighted,

    CorrelatedField {
        field: CorrelatedField,
        strength: f64,
        scale: f64,
    },
}
```

`ArtworkWeighted` declares source dependence only. The receiving channel
supplies its independent weighting mapping, tonal response, strength, and curve
from document/channel configuration. A reusable recipe carries none of those
values; changing a recipe preserves both fill and weighting source settings.

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

- Density and density aspect.
- Rotation.
- X/Y translation.
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

Pattern rotation and X/Y translation are available for every current pattern,
including `ArtworkWeighted` source-document density. Artwork-weighted placement
is a special alignment case: the interface warns that transforming the pattern
offsets its relationship to the source channel weighting. The warning explains
that neutral Rotation, X and Y values (all `0.0`) restore source-weight-map
alignment. It informs the artist without blocking the transformation.
Candidate positions are transformed first, then artwork weighting is recalculated
at those document positions; the application does not merely move an already
weighted site arrangement. Weighting and fill-response source components are
independent document/channel configuration. Both default to the receiving
channel's matching component; Advanced Settings owns their source selection,
including explicit Luminance for monochrome artwork. Changing either mapping
does not edit the recipe, and applying a Pattern does not overwrite those choices.

The Pattern Editor/Wizard defines construction and whether source weighting is
used. Its response preview uses neutral minimum `0` and maximum `1`; it does
not author adjustable fill or thickness bounds. Those typed bounds and region
artwork sampling belong to document/channel response state and are edited in
main-window Advanced Settings. Source mapping and source-response shaping also
belong to document/channel state. This includes weighting strength,
response curve, inversion, gain, bias and tonal interpretation. A recipe must
not store or edit these source settings. Weighting consumes its independently
configured source response; fill/geometry consumes its own. Do not implicitly
couple their source components or tonal adjustments. Additional source consumers
must use distinct channel configuration when they can legitimately differ.
Geometric controls such as stroke-normal alignment and region resize algorithms
remain construction/geometry intent, not source-response shaping.
Capability projection, command validation and effective evaluation must not
omit, reject or silently zero rotation solely because placement is artwork
weighted. Selecting artwork-weighted placement must not prune a rotation delta
for that reason; unrelated channel intent remains unchanged.

### 10.2 Realization rebuild

These should reuse family sites/guides where possible:

- Shape size.
- Shape rotation.
- Curve/network thickness.
- Region/cell inset.
- Geometry response curve or polarity.
- Fill/geometry source mapping, including component, inversion, gain, bias, and placement.
  Weighting source changes regenerate Family when an active recipe consumes them;
  an unused weighting binding is retained as document intent without rebuilding geometry.

### 10.3 Presentation only

These must not regenerate family geometry:

- Color.
- Opacity.
- Visibility.

### 10.4 Source invalidation

These require source resampling and dependent evaluation:

- Source frame/image.
- Decoded source content or decoder contract.

### 10.5 Invalidation contract

```rust
pub enum InvalidationLevel {
    Presentation,
    Realization,
    Family,
    Source,
    ChannelTopology,
}
```

Every authoritative command returns its invalidation level. `ChannelTopology`
is a structural document transition for an atomic model/topology replacement;
it invalidates affected channel evaluations and aggregate scene/raster state
without making otherwise matching per-channel structural artifacts unsafe to
reuse.

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
- Every current definition accepts nonzero pattern rotation through complete
  realization and rasterization. Artwork-weighted placement exposes the
  source-weight alignment warning and neutral-transform recovery described above.
- Rotated thin-stroke raster coverage retains exact nonzero-winding parity at
  disjoint, nested, coincident/opposite, antialiasing-off, and clipped-boundary
  cases while traversing only row-active edges and nonzero spans.

### CLI

- `--help`, `-i`, and `-o` work conventionally.
- Output extension chooses PNG/SVG.
- `--channel-model rgb|cmyk|source-color-alpha` selects authoritative topology.
- `--background transparent|black|white` is consumer-only; when omitted, RGB defaults to black, CMYK to white, and SourceColorAlpha to transparent.
- The halftone channel model selects only that omitted PNG consumer default; an explicit background overrides it and the model never selects PNG encoding.
- PNG output is always sRGB/sRGBA; SVG remains transparent.
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
- Document-owned base pattern recipes and domain-resolved effective channel
  recipes, as specified in section 15 of this addendum.

### `ChannelSchema.md`

Add or revise:

- Correct numeric types.
- Simple start/end transitions.
- Multi-frame evaluation.
- Invalidation levels.
- Family regeneration for density, rotation, and translation.
- Stale-result rejection.
- CLI mapping for primary channel settings.
- Optional channel recipe replacement, typed additive deltas, and explicit
  reset-to-inherit commands, as specified in section 15 of this addendum.

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
3. The document owns a base pattern recipe and base typed pattern settings.
   Channels may replace that recipe and add only typed scalar deltas; they
   retain independent translation, source mapping, appearance, and simple
   start/end animation.
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

## 15. Effective document pattern authority

This section supersedes conflicting language in `PatternSchema.md` and
`ChannelSchema.md` about every channel owning a fully independent pattern
instance.

The document owns one base structural Pattern recipe and base settings for
pattern rotation, shape rotation, and density/detail. It separately owns typed
ALL channel-response ranges and region artwork sampling. A channel may select
an optional replacement recipe, add applicable structural density/detail,
pattern-rotation, shape-rotation, or connected stroke-alignment deltas, and
author optional named response fields. A channel with no replacement or field
override inherits the applicable document value. Resetting a named response
field restores inheritance; it is not a hidden copy of the current ALL value.

The document/domain layer resolves and validates an effective pattern instance
before evaluation. GTK, CLI parsing, renderers, and exporters may project or
request that result, but never calculate an alternative effective value.
Invalid effective values are rejected at the command boundary rather than
clamped or reinterpreted by a consumer.

The following remain channel-specific and are not inherited through this
mechanism:

- translation;
- source mapping;
- color, opacity, and visibility; and
- any later channel-only presentation state.

Output-specific response values remain distinct typed fields. The artist may
use common labels such as minimum and maximum fill, but mark fill/size,
stroke-thickness response, and region inset/grow-shrink response are not one
stored value or one interchangeable unit. Structural connected stroke alignment
and region resize algorithm remain Pattern intent. Source tone and weighting
remain separate document/channel source consumers.

Stage 20G owns the current-only document-schema transition, validation,
history, invalidation, and persistence work for this authority. It does not
restore obsolete document or preset compatibility. A preset-format transition
is required only when its persisted recipe representation changes.

---

## 16. Source-alpha interpretation

Decoded source pixels retain their raw straight (unassociated) sRGBA values,
including hidden RGB where alpha is zero. Realization derives mapped scalar
responses at the sampling boundary before bilinear interpolation. Linear RGB,
CMYK, and Luminance are color-derived and are associated with alpha exactly
once after their component and mapping transform are evaluated. Hidden RGB at
alpha zero therefore cannot create a response fringe. Alpha remains an
independent scalar; mapping inversion/gain/bias determines its response, and
it is never multiplied by alpha a second time.

SourceColorAlpha separately interpolates associated linear RGB and alpha and
unassociates positive-alpha RGB for per-mark paint. In Variable mark mode,
sampled alpha drives mark size once. In Uniform artwork-weighted mark mode,
sampled alpha drives mark survival once; each surviving positive-alpha mark has
the calibrated fixed size and opaque sampled paint apart from channel opacity.
Exact zero sampled alpha suppresses visible mark paint in either mode, even
when an inactive Variable minimum mark size is nonzero.

SVG decoder output must be unpremultiplied to retain straight RGBA; the normal
precision and zero-alpha caveats of that conversion remain applicable. Raw
hidden RGB remains available for source inspection, but does not affect the
alpha-associated color-derived mark response.

---

## 17. Stage 21 density, still-source, and ALL authority

This section supersedes the earlier stored `across_x`, `across_y`, and
`aspect_locked` representation. A current document stores one finite positive
**Density** and one finite positive **Density aspect**. `density²` is the
approximate geometry count. Density aspect is `spacing_x / spacing_y`: `1.0`
is 1:1, `2.0` is 2:1 horizontal spacing, and `0.5` is 1:2.

For a canvas with positive finite `width` and `height`, the domain resolves
evaluator inputs as:

```text
across_x = density × sqrt(width / (height × aspect))
across_y = density × sqrt((height × aspect) / width)
```

For fresh documents and direct still-image imports, the default Density is
`100 × sqrt(min(width, height) / max(width, height))`, with aspect `1.0`.
This is pixel-resolution independent: it resolves to approximately 100 sites
on the longest edge and proportional coverage on the short edge (square
100×100, 2:1 landscape 100×50, and 1:2 portrait 50×100). Opening a current
schema-v6 document never rewrites its stored Density. Density and aspect affect
generated site positions and baseline curve layout. They do not scale mark
prototypes or authored closed shapes; regions change only as their source sites
and resulting boundaries change. Channels retain optional additive density and
aspect deltas as one atomic pair. Reset removes that pair and restores
inheritance.

The artist-facing main inspector presents effective density as **Pattern zoom
level**, where `zoom = default_density / effective_density`. `1.0` is the
aspect-normalized fresh/direct reference above; a smaller value zooms out and
produces more geometry, while a larger value zooms in and produces less
geometry. The adjacent **Pattern aspect** control presents the authoritative
density aspect unchanged. These are presentation projections only: documents,
commands, channel deltas, cache identity, and evaluator inputs retain Density
and Density aspect as the sole authority.

Normal app and CLI evaluation impose no application-authored upper count on
requested pattern geometry or derived realization work. Machine addressability,
checked arithmetic, fallible allocation, deterministic approximation depth,
cancellation, and stale-result rejection remain mandatory. Explicit finite
limits remain injectable for focused tests and constrained headless callers,
but are not the default creative policy.
Unbounded creative intent must not become an eager infallible allocation from
an untrusted derived count. Variable-size generation grows through fallible
incremental reservations and polls cancellation; counts that exceed machine
representation fail with a stable diagnostic before allocation. Random
dispersion builds accepted final sites directly, uses their positions for
spatial conflict checks, and releases clustered-parent and spatial working
storage when generation ends. Cached diagnostic and complete-document results
share immutable scene/raster values rather than deep-cloning them. Raster
composition retains at most one complete accumulator and one complete transient
layer, uses fallible full-pixel and final-byte allocation, and never refuses
valid creative work from an estimate alone.
The schema-v6 random-site `maximum_attempts` and
`maximum_neighbor_checks` members remain reserved nonzero recipe data during
this checkpoint; they are omitted from active controls, family identity, and
normal evaluation and therefore do not impose a creative-work ceiling.
Random placement publishes its best deterministic result after a complete
geometry-derived no-progress sweep. This convergence rule scales with the
requested padded population and, for even dispersion, the exclusion-sized
cells covering the generation domain; it is not an application-authored count
ceiling and physical packing saturation is not a render failure.

Guide-path thickness samples source response at the pattern's useful spatial
scale. Straight guides use an interval no smaller than the guide's nominal
pattern-spacing basis or one decoded-source-pixel footprint, whichever is
larger; they do not supersample every source pixel. Curved guides may add
samples only for independent centerline-flatness requirements. Linear response
between the retained samples is the current default, and the sampling interval
does not change pattern-site Density authority.

Shared canonical outline compaction preserves those adaptive source samples as
its input and retains the largest normalized error first. Centerline deviation
remains bounded by 0.125 document units. At each positive sampled half-width,
interpolation deviation is bounded by the smaller of 0.125 document units or
2% of that local half-width. Exact zero-width breaks, authored segment and
tangent boundaries, and closed seams remain explicit. The iterative compaction
polls cancellation and obeys the caller's derived-outline segment bound; it
does not alter source sampling or authored path geometry.

Canonical-stroke rasterization flattens each outline once, schedules edges by
the pixel-row intervals where they can contribute, and traverses only active
nonzero-winding X spans. It does not scan every flattened edge or every pixel
in a rotated stroke's axis-aligned bounds for every row. Traversal polls
cancellation at bounded intervals and preserves strict half-open crossings,
nonzero winding, antialiasing samples, final raster clipping, source-over
composition, canonical geometry, and raster/cache identity.

Main-preview workers publish ticketed, monotonic progress that is excluded from
document and cache identity. The first source-backed render queues and exposes
the same pending stages on the visible canvas before idle viewport submission;
source-less New remains empty. Fixed coordinator weights are 20% for preparation
and source decode, 25% for per-channel family geometry, 45% for ordered output
realization, 5% for scene composition, 4% for preview rasterization, and 1% for
final publication. Frontends discard stale-ticket progress and present one
overlay with an overall bar above a current-stage bar, visible percentage/stage
labels, and the last accepted preview retained until the new result is atomically
accepted.
Expensive dispersion-parent/candidate, straight multi-guide
intersection/merge, parametric construction, generic-guide expansion,
curve-contact/merge, and along-curve sampling loops report completed units
within their fixed family share. Ordered-output realization
reports completed site-bound work, including progress observed from parallel
workers, within its fixed output share. AreaAverage reports completed prepared
source-cell intersections rather than cancellation-poll frequency. Rasterization reports only completed
canonical primitives and completed final composition, background, and
quantization phases within its fixed raster share. The scheduler serializes
and coalesces these observations at per-mille resolution, including exact
duplicate suppression, so concurrently reported progress remains monotonic and
responsive without flooding the frontend. Progress is observational only: it does not change canonical
geometry, pixels, cancellation semantics, cache identity, or atomic
publication.

AreaAverage uses literal decoded-source pixel footprints. Canvas edges map to
the outer edges of the decoded pixel array. A pixel is included when the exact
intersection of its footprint with the complete untreated region covers at
least 50% of that footprint; an included pixel contributes its complete mapped
scalar and associated color/alpha values once, while a pixel below the
threshold contributes nothing. Fractional coverage never multiplies a pixel
value. The result is the equal-weight average of the included full pixels.
Finite off-source footprints remain part of the complete untreated region and
use the nearest edge pixel, once per unit footprint, rather than collapsing an
exterior band into one sample. Candidate footprints are classified in stable
source order inside indexed parallel region work. Request-wide checked counts,
fallible working storage, cancellation, coalesced progress, deterministic
publication order, and stale-result rejection remain mandatory.

The density and still-source behavior in this section was established at the
schema-v6 checkpoint; that version is no longer current. Current development
uses project document schema **13** in media container **2**, and source-free
Document Preset format **3** with configuration schema **12**. Earlier project
schema 12 and Preset configuration schema 11 are rejected; values are not
reconstructed from channel consensus or migrated through UI defaults. Pattern
format **6** stores structural output intent only; document response does not
become authority in reusable Pattern resources.

The still-source boundary accepts PNG, SVG, JPEG/JPG, WebP, BMP, TIFF,
OpenEXR, and AVIF. A `.toniator` container preserves the exact original source
bytes and explicit source format. Only single-image sources are supported
before the separately gated temporal-media stage: animated WebP/AVIF and
multipage TIFF fail rather than selecting one frame. Supported non-deep RGB or
RGBA OpenEXR decodes as linear floating-point data. Each channel is made
finite and clamped to normalized `0.0..=1.0` without hidden tone mapping or
color-management policy. Decoder-contract identity changes when this behavior
changes.

The main inspector has a runtime-only **ALL** target and real named-channel
targets. ALL is never represented by a fabricated channel ID. New and Open
select ALL by default; channel-model and topology changes preserve it.
Ordinary ALL scalar edits assign the entered effective value to every compatible
channel/output at the selected Start or End frame as one atomic, undoable edit.
Mixed values are displayed explicitly, not as a shared average. Only the edited
property is equalized: unrelated channel intent, the other initialized endpoint,
and interpolation choices remain intact. Inapplicable outputs are omitted and
invalid coupled bounds reject the entire edit. A preset applied to ALL atomically replaces the base recipe and clears
every channel's pattern replacement plus density, density-aspect,
pattern-rotation, shape-rotation, and structural stroke-alignment deltas. The operation
first discloses affected channels, is one undoable command, and Undo restores
the exact former base and channel intent. Translation, source mapping, paint,
color, opacity, and visibility remain untouched. Applying a preset to a named
channel retains selected-channel replacement and prunes only structural deltas
made incompatible by that replacement. Channel-response intent is reconciled
per channel as specified below.

### 17.1 Last-ALL Appearance baseline

The document and reusable `DocumentConfiguration` each store a typed, sparse
`DocumentAppearanceBaseline`. Its optional values are effective Start-frame
Density, Density aspect, pattern rotation, X translation, and Y translation.
Density and aspect are positive finite values; rotation and translations are
finite. An unset field carries no recorded shared intent and keeps the current
document or canonical-topology default.

An explicit ALL Start assignment atomically updates every compatible channel
and only the baseline entries named in that assignment. It records intent even
when the effective channel values already match. Such a baseline-only update
is one dirty, undoable persistence/history change with no render invalidation.
An ALL End assignment and every named-channel assignment leave the baseline
unchanged. The baseline is never inferred from current channels, a majority, or
history.

When the color/channel model changes, canonical channels are rebuilt from the
document's base Pattern and the stored baseline. Stored density/aspect values
become channel-relative additive deltas, pattern rotation becomes a relative
delta, and X/Y remain absolute channel translations. Existing channel-specific
deviations do not seed the replacement model. Unset baseline entries retain
the canonical topology defaults.

An ALL Pattern replacement clears stored density, density-aspect, and
pattern-rotation entries along with the corresponding Pattern-relative channel
deltas, so the replacement cannot later resurrect values tied to the previous
base recipe. It retains X/Y baseline entries because replacement preserves
translations. A named Pattern replacement leaves every shared baseline entry
unchanged.

Project schema 13 and Document Preset configuration schema 12 serialize this
baseline. The surrounding media container and Document Preset format stay at
versions 2 and 3; the Pattern resource version does not change.
Older development project and configuration schemas are rejected without
compatibility adapters or inferred defaults.

### 17.2 Channel-response ownership and Pattern replacement

The document and reusable configuration store typed ALL response intent for
marks, connected strokes, and regions. Each named channel may override individual
compatible fields without copying unrelated ALL fields. An explicit ALL Start
field assignment equalizes only that field on compatible active channels and
records the ALL value even when it matches the old baseline. Paired response
bounds can publish atomically. Named assignments leave the ALL intent unchanged;
Undo and Redo restore both authorities exactly. Color-model replacement seeds
new channels from the stored ALL response for the current output kind.

Applying a Pattern never imports adjustable response bounds or sampling from its
recipe. Compatible output-kind replacement preserves each affected channel's
effective Start response and initialized End value and easing while rebinding
the sole output identity. Same-structure edits preserve by output identity.
When a channel changes between marks, connected strokes, and regions, only that
channel's incompatible response defaults to `0..1`; a new region defaults to
area-average artwork sampling. A document-base kind change also records neutral
ALL intent so later model switches or a return to the old kind do not resurrect
stale bounds. Structural stroke alignment and region resize continue to follow
Pattern replacement and their own temporal reset rules. Source mapping and
independent artwork-weighting settings are preserved.

Project schema 13, Document Preset configuration schema 12, and standalone
Pattern Preset format 6 encode this boundary. Superseded pre-release shapes are
rejected rather than migrated.

### 17.3 Uniform artwork-weighted marks

Current document response also stores a mark density mode (`Contextual`,
`Uniform`, or `Variable`) and a finite artistic mark size scale in `0..2`, with
default `Contextual` and scale `1`. Named mark outputs may override each field
independently. `Contextual` resolves Uniform only for an actual artwork-weighted
random-site Marks output; other marks retain Variable sizing. A strict Visible
Mark Margin exclusion retains non-overlap and resolves Variable with an
applicability reason. An explicit Variable choice keeps the authored minimum
and maximum fill response; the inactive range and scale remain stored through
compatible replacements and model switches. Incompatible output-kind changes
reset only the affected response intent. The mode is static; size scale follows
the continuous Start/End response path.

Uniform mode constructs the complete site population independently of ordinary
artwork weighting, then uses stable per-site thinning without replenishment.
Site-use filters and guard-site mark output obey the same survival decision.
A bounded, source-independent canonical circle or authored closed-shape fit
measures at least `99.9%` full-tone opaque canvas coverage; overlap
multiplicity supplies the inverse expected coverage for lower tones. The
artistic scale applies after this fit and must remain within the population's
planned support. Density-weighted Lloyd remains source dependent. Independent
fill cutoff and alpha paint suppression remain effective; ordinary fill tone
and Variable bounds do not alter Uniform size. Variable RGB/CMYK geometric-union
calibration remains unchanged. This statistical flat-field response does not
promise local pixel equivalence at artwork boundaries.

Current project schema 14 and Document Preset configuration schema 13 persist
document-owned mode and scale. Standalone Pattern format 6 remains structural;
superseded development schemas reject without migration.

### 17.4 Authored spatial density for Scatter

This subsection supersedes the Scatter count interpretation and default Density
formula in section 17 above. Current Density is the scalar `100 / Feature size`,
where Feature size is finite, positive, dimensionless, and independent of canvas
extent. Fresh documents use Density `100`, hence Feature size `1`. The existing
resolved guide axis counts preserve the guide appearance at each canvas:

```text
guide_density = density × sqrt(min(width, height) / max(width, height))
across_x = guide_density × sqrt(width / (height × aspect))
across_y = guide_density × sqrt((height × aspect) / width)
```

The inverse projection uses those same factors. Scatter does not interpret
`across_x × across_y` as a target count. Raw, Jittered, and Clustered recipes
own one finite positive nominal spacing in authored document units; its built-in
default is `10.24`. Even dispersion uses its authored minimum-center distance as
its sole spatial scale and stores no second nominal spacing. Feature size
multiplies these lengths directly, without scaling them by canvas dimensions.
Scatter Density aspect is inapplicable; guide recipes retain their current
aspect behavior.

At a fixed Feature size, canvas area determines the offered Scatter population.
Raw offers candidates from area intensity, Jittered offers physical cells, and
Clustered derives parent and offspring intensities from area while retaining its
authored concentration and spread. Even dispersion runs its geometric exclusion
process to bounded saturation rather than stopping at a desired count. Source
weighting and exclusion filter this complete offered field without replenishing
rejected sites. Seed, full off-canvas guard topology, strict visible margins,
and Lloyd behavior remain authoritative. Exhausted resource limits fail with a
diagnostic rather than publish an underfilled partial family. Scatter mark and
line response use the same Feature-scaled spatial basis as placement. Changing
Feature size does not rewrite the authored recipe distance.

The current in-development schema transition is project `15`, Document Preset
configuration `14`, and standalone Pattern `7`. These formats persist the one
authored Scatter spacing where applicable; superseded formats reject without
migration. This transition does not yet persist a saved calibration artifact.

### 17.5 Saved structural calibration and Preset conversion

This subsection supersedes earlier transient or render-time fitting language
and the version/rejection statements above. A finished structural Pattern edit
measures one source-independent, fixed-reference coverage artifact. The
artifact stores a versioned geometry profile, typed ID-free structural binding,
and monotone raw `C(size)` observations over `0..=2`. Eligible marks also store
the dimensionless Uniform full-tone radius and survival inverse, regardless of
the response mode selected when measured. The binding includes effective
connected-stroke bias and guide aspect where those affect coverage. Measured
shape rotation remains a field in the saved binding as immutable provenance,
without a format change; all other static binding fields require exact
compatibility. Compatibility ignores the
effective individual Shape rotation. Static and animated Shape rotation retain
the assignment's saved artifact: changed overlap and tone are intentional;
explicit nonoverlap remains enforced. The binding excludes scalar
Density/Feature size, artwork, source tone and paint, whole-Pattern
placement/rotation, and Advanced response range, sampling, mode, and artistic
scale. Each channel independently owns its artifact, even when measured
bindings are equal. A compatible donor may fill a missing assignment during
guarded publication; it never replaces an existing compatible assignment.
Scatter site, randomized connection, and maze variation seed values are excluded
from that binding. The saved reference uses fixed seed `43` for each such field;
the authored seed remains in the Pattern and still changes its generated
geometry. Seed-bearing bindings use distinct seed-neutral v5 strict or mean
profiles, while seed-free v4 measurements remain current except for the typed
producer revisions described below. Cross-seed response is
statistical: representative alternate-seed checks are acceptance evidence, not
a guarantee for every `u32` seed or individual render.

The document also retains one optional saved companion for the last-ALL base
Pattern binding, even when no currently modeled channel uses it. Named channel
edits cannot erase that companion. A later model replacement may rebind it to
new channels without measuring. A structural change to the ALL binding must
prepare its missing companion alongside changed active bindings before
publication. Named Recalibrate updates only the selected channel and preserves
all other channel assignments and an existing ALL companion; it may fill a
missing ALL companion. ALL Recalibrate updates every active channel and the
ALL companion. Fresh measurement may deduplicate exact current bindings, but
publication enumerates the intended recipients explicitly. One atomic history
result retains every independently assigned numeric artifact in project and
Document Preset persistence.

Animation contains eligible continuous overrides only, never structural or
discrete keyframes. Every materialized frame retains Start's exact assigned
artifacts, including animated guide aspect and connected-stroke bias. This
exception belongs only to immutable domain frame authority consumed by
preview and export. A borrowed or cloned ordinary document gains no exception;
ordinary static aspect/bias edits remain incompatible and require measurement.
Materialization never fits, donates, or fills missing assignments.

When an effective Shape rotation trajectory is nonconstant, End frame presents
this dedicated warning near its controls and in its explanatory tooltip:
“Rotating marks can change overlap and tone. The saved calibration stays fixed.”
A full 0→360 turn qualifies. Base and named delta trajectories are evaluated
with their easing; exact cancellation suppresses the warning. This description
never replaces an error or progress message.

Ordinary preview, PNG, SVG, reopening, and nonstructural edits use the saved
artifact without entering a fitter. A missing, stale, or unsupported artifact
requires explicit recalibration; it never silently uses an identity response or
unbounded transient fit. A user may request Recalibrate. Structural Apply/Save
publishes the structure and all required artifact replacements as one guarded,
undoable result; cancellation or a stale revision publishes neither. Neutral
Wizard structural previews remain disposable and do not fit per edit. The
source-independent fixed reference does not infer a transfer artifact for an
unmeasured family or shape.

Advanced Variable mark, connected-stroke, and region response endpoints now
express **desired absolute geometric coverage** in `0..=1`, for every supported
single-output model and paint, including equal endpoints. For each sampled
source response, the document first interpolates desired coverage between
those endpoints. The saved raw `C(size)` curve then maps that request to one
final raw size in `0..=2`, clamping only to the measured `C(0)..=C(2)` reach.
The document does not normalize requested coverage to that reachable span or
apply the Advanced range a second time. A zero request remains at the earliest
attainable response; positive low tones retain measured inverse precision.
Uniform marks keep their separate 99.9% full-tone footprint and survival
inverse. Source mapping, cutoff, alpha suppression, paint, contextual mode, and
strict nonoverlap retain their existing authorities. Calibration probes use an
explicit transient raw-size request; no ordinary document field temporarily
stores a raw size in a coverage-valued response slot. Raw geometry profiles retain their fixed reference populations; the producer
revisions described below require matching numeric provenance.

The fixed 1024-square reference first fits 32 nominal spacings and checks an
independent 64-spacing population. Only a population-convergence failure may
retry one complete 64-spacing fit with a 128-spacing check. Both tiers retain
the same raw-knot, measurement-work, low-tone inverse, cancellation, and
accuracy limits. After both tiers fail convergence, an isotropic single finite
Round Spiral with one ordinary structural stroke or mark output may use one
distinct covered-interior profile: a fixed 768-square canvas, 64-spacing fit,
and independent 128-spacing check. Its complete outer revolution must enclose
the reference square: `(turns - 1) * radial_spacing >= hypot(384, 384)`.
The authored finite turns, spacing, shape, and exact calibration binding remain
unchanged; the profile records the alternate measurement window. All existing
fit, transfer, low-tone inverse, work, and cancellation gates remain in force.
An ineligible or failed interior attempt remains an explicit diagnostic.
Existing 1024-square companions retain their measured profile and numeric data.
After both strict population checks fail, only a typed random-site family,
seeded connection program, or maze may use a bounded average of the frozen 32-
and 64-spacing populations measured at each identical size. The mean retains
the same tight interpolation and inverse checks and an independent 128-spacing
transfer check: raw variation is at most 2% of reachable span, normalized tone
variation at most one percentage point and at most 10% relative for low tones,
including the default response interval `0..=1`. Its distinct seed-neutral v5 profile
stores typed observed maxima, not a confidence interval or guarantee for each
render. Eligible weighted Uniform marks use the same paired populations for
full-tone radius and survival inverse, require 99.9% full tone, and use white
128-spacing coverage to select one bounded common footprint. The 128-spacing
lower-tone survival response remains an independent transfer check. The
persisted profile identifies which strict tier or approved mean produced the
artifact. A former
numeric profile in a recognized source-free Document Preset is discarded during
conversion so the destination can recalibrate before publication; current
projects still reject that obsolete profile.
Corrected convex-linear UniformOffset insets intersect their shifted inward
edge half-planes: vanished edges disappear and a consumed polygon stays empty.
Outward RegionRound construction remains unchanged. Authored open guides derive
one shape-preserving scale from the stable canvas frame, independent of Feature
size and generation padding. Bounded endpoint rays provide required off-canvas
coverage, retaining tangent continuity where a finite bounded continuation is
possible; the original scaled authored interior and repetition pitch remain fixed.
Affected saved profiles append the applicable typed producer marker. When both
apply, the canonical order is `+guide-span-v2` followed by `+convex-inset-v3`. Only formerly valid unmarked companions for
those typed producers are obsolete. Ordinary projects reject them; supported
source-free Document Presets discard only that numeric companion and recalibrate
before publication, preserving authored Start/End intent and original bytes.
Current-format standalone Pattern Presets may likewise omit their recognized
obsolete optional companion and require recalibration before structural apply.
Unaffected geometry profiles and numeric artifacts retain their meaning.

Current seed-free strict geometry profiles are v4 at both 32/64 and 64/128
tiers except Guide Faces; seed-bearing strict and 32+64 mean profiles are
seed-neutral v5. Guide Faces producer contract v2 canonicalizes provenance-linked
near-coincident contacts while preserving the authored guides and the fixed
reference windows. It uses distinct seed-free Guide Faces v5 profiles for the
32/64 and 64/128 strict tiers. A former generic v4 companion whose typed output
is Guide Faces is obsolete even if its recipe still matches: supported source-free
Document Presets discard only that numeric companion and prepare the exact active
and hidden ALL bindings before publication. Configuration-16 desired-coverage
Start and End values stay unchanged; older raw-response Presets still convert
their endpoints through the newly prepared curve. A standalone Pattern Preset
may discard its optional former Guide Faces v4 companion in memory while
preserving authored recipe and file bytes. Ordinary projects reject the obsolete
companion. Unaffected producers keep their profiles and numeric artifacts; this
producer revision changes no container or schema shape. Recognized
former v2 and v3 measurements are removed from source-free Document Presets
without changing authored Pattern or response intent; ordinary projects reject
those obsolete profiles. A recognized source-free Document Preset also removes
only former v4 seed-bearing numeric companions. A current-format standalone
Pattern Preset may drop only its optional former v4 seed-bearing companion when
read, preserving the authored recipe, seeds, and original file bytes so a later
structural Apply can prepare v5. Unknown profiles still reject; ordinary projects
remain current-only. A generic CoverCanvas Pattern Preset may carry the
measurement of its exact finite spiral materialized on the fixed 1024-square
reference. Validation computes that concrete reference from the authored
CoverCanvas recipe while leaving the stored recipe generic. Applying the Preset
attaches the companion only when the receiving channel's complete concrete
binding matches it; a different canvas-derived turn count requires its own
measurement.

The current project document schema is **17** in media container **2**;
source-free Document Preset format remains **3** with configuration schema
**16**; standalone Pattern Preset format is **8**. Projects remain current-only.
Standalone Pattern Presets carry an optional typed artifact and remain
current-only. Source-free `.toniator-preset` Document Presets are the explicit
pre-release compatibility exception: recognized older formats retain former raw
Start and initialized End response intent in a typed, unpublishable pending
import. A valid current neutral shell supplies exact active and hidden ALL
bindings for headless saved-companion reuse or bounded calibration. Only after
every required companion is present does conversion map each former raw
endpoint `a` to its binding's measured `C(a)`, reconstruct ALL and explicit
named authority where distinct curves require it, and validate a complete
current configuration. An old raw endpoint above one is not clipped before
conversion. Missing data, cancellation, or a stale publication guard leaves
the existing document and original Preset bytes unchanged. Conversion
preserves active effective endpoint coverage and easing for ALL and named
channels; it does not reproduce former raw-size interpolation or former
uncalibrated SourceColorAlpha intermediate footprints. It preserves authored scalar Density and
explicit Scatter distances; old Raw/Jittered/Clustered recipes without an
authored spacing receive the current `10.24` default. Obsolete dormant
per-Pattern response endpoints have no live current authority and do not
override current Advanced response on later Pattern replacement. Original
archives are retained when personal files are converted. Unknown or
unrepresentable revisions fail with a precise diagnostic rather than silently
discarding active settings.

---

## 18. Headless Curve Motif prerequisite

The separately accepted headless Curve Motif prerequisite follows Stage 21A and
precedes any resumed Stage 21B wizard work. `CurveMotifPaths` wraps exactly one
one-dimension generalized straight-guide **Along Guides** recipe and one
document-owned validated authored **open** path. Closed, empty, coincident-end
point, two-guide, wrong-family, missing-resource, and nonfinite layout inputs
reject; there is no external motif file, per-tile path authority, smoothing, or
renderer-specific motif path.

Adjacent Along Guides sites define the longitudinal cadence. Each authored
motif copy maps its distinct endpoints exactly to adjacent sites, and stable
guide-instance provenance groups copies into one continuous open path per row.
Even rows retain the authored path. Odd rows use Euclidean guide-index parity
and may mirror across their tangent axis and then translate by an optional
finite fraction strictly between zero and one of one site interval. A phased
row receives one extra automatic guard interval before final clipping. Existing
guide spacing/phase, Density/Density aspect, rotation, and translation remain
the only row-layout authority.

Curve Motif uses the existing source-driven connected stroke response,
canonical variable-width path realization, paint, preview, PNG, and SVG
pipeline. Mirror and optional phase are exact typed descriptor/command fields
with `Realization` invalidation; an edit of the referenced motif resource also
invalidates realization unless another use requires the stronger Family
boundary. Parallel row work uses the shared Rayon pool, checked/fallible growth,
cancellation polling, and coalesced monotonic completed segment/row progress.

This prerequisite advances current-only persistence to document schema **v7**
and preset format **v4** while retaining container v1. v6 documents and v3
presets reject without adapters. Preset-v4 embeds motif geometry and layout.
The sixteen-entry bundled catalog remains unchanged; a future Curve Motif
gallery card and personal-preset/shape-library work remain separately gated in
Stage 21B.

## Native Windows filesystem infrastructure

`toniator-windows-fs` is a platform-specific infrastructure leaf for safe native
Windows filesystem operations. It may encapsulate unsafe Windows ABI calls and
may be consumed by `toniator-io` and `toniator-sampling`; existing headless core
crates retain their unsafe-code prohibition. It has no frontend, GTK, domain,
rendering, or product-policy authority. Non-Windows builds contain no platform
implementation or disabled product-feature fallback.

The boundary preserves retained-directory and file identity, exclusive private
creation, no-follow relevant reparse leaves, caller-available capacity, full
synchronization, atomic publication, and ownership-bounded explicit deletion.
Filename policy and recovery decisions remain with callers. Publication revokes
staging cleanup authority before parent synchronization; synchronization failure
does not authorize deletion of an already published output. Drop never removes
caller-retained partials or recovery artifacts.


## 22. Artistic geometry controls and certified construction (2026-10-19)

This explicitly approved coordinated TON-021/TON-023/TON-024 contract supersedes
conflicting earlier sampling, connection-selection and current-format statements
only within the scope below. The bounded execution contract is
`docs/ARTISTIC_GEOMETRY_CONTROL_PLAN.md`; phase evidence never implies acceptance.

GuidePaths, ParametricPaths and CurveMotifPaths own one finite positive tone-sample
spacing multiplier, default 1. Connected-point paths and mazes do not expose it.
The requested arc-length interval is the multiplier times `max(canvas.width,
canvas.height)/100` times the ratio of `DensityMetric2D::default_for_canvas(canvas)
.density` to effective Density. It scales proportionally with Feature size, without
a decoded-source-pixel floor, Automatic mode, numerical tolerance change or legacy
sampling branch. Existing point-spacing authority continues to scale with Feature
size. Complete logical paths retain structural cadence anchors across pieces,
subdivision and construction/clipping windows. Genuine endpoints are sampled;
geometric refinement evaluates a linear reconstruction of completed normalized
thickness between retained source-response knots and adds no artwork knots. Existing
alpha/mapping/cutoff/response authority remains; sampled zeros, true corners and seams
remain explicit. Artistic coarsening may omit source features between knots.

ConnectionSelectionIntent is distinct from candidate adjacency and ConnectionProgram.
It owns optional positive maximum final degree (None by default), cycle policy Allow
or Forbid (Allow by default), and minimum component size in distinct family points
(at least 1, default 1). Final degree and cycle constraints include rescue joins.
Joining singleton/undersized components is automatic, with no separate toggle. A
join targets any existing family vertex within the existing maximum connection
distance, introduces no crossing or overlapping selected centerline, and creates no
new vertex. Rescue ties use (distance, source point ID, target point ID); original-program seeds
do not randomize rescue ties. Ordinary connection crossings retain their existing
behavior, and painted thick-stroke overlap is not prohibited. Join before component
filtering and count combined distinct points.
Unresolved singleton components surviving minimum 1 render an ordinary canonical
circle from the same connected response/source/basis as neighboring points; bias is
inapplicable. One connected output carries strokes and canonical circles together
through every shared consumer; zero-length stroke substitutes are prohibited.
Selection edits invalidate Family; tone-spacing edits invalidate Realization, with
existing stronger copy-on-edit invalidation where required.

Construction may prune off-canvas authored-shape materialization, straight-grid
candidates and curve/connection stroke outlines only behind the approved semantic
boundary. Full logical usage, family/topology, centerline and source profile remain
authoritative. Complete profiles and global compaction decisions precede certified
envelope outline materialization. Exterior half-plane excursion connectors preserve
nonzero fill without artificial caps; unproved cases use full fallback. True caps,
zeros, seams, extrema/anchor metadata and the raster quantization frame remain.
Numerical accuracy, cancellation, checked/fallible growth, shared canonical outputs
and strict calibration criteria do not weaken.

Current project schema is 18, Document Preset configuration schema 17 and standalone
Pattern format 9; container and Document Preset envelope versions stay unchanged.
Projects remain current-only. Supported older source-free Document Presets adopt
new defaults deterministically and preserve original bytes; prior artwork behavior
need not survive. Changed calibration bindings/assignments and pools are prepared
anew while preserving desired coverage endpoints for ALL, channels and animation.
Legacy raw-response origin is explicit and cannot be inferred merely from schema
age. Tone cadence leaves uniform calibration identity only after constant-probe and
uniform-source equivalence proofs; structural spacing/selection remains bound.

## 23. Persistent coverage intent and transformed generation (2026-10-19)

This explicitly approved correction supersedes earlier recipe-only CoverCanvas
materialization and corresponding current-format statements. Section 22 remains
authoritative for artistic controls, full logical construction and certified pruning.

ParametricCurveSource persists the existing Fixed or CoverCanvas spiral policy
alongside its authored curve and repetition. Recipes, definitions, copies, history,
configuration, persistence and calibration reconstruct the same policy. Fixed keeps
the authored finite curve exactly; no gallery name, identifier, turn count or saved
measurement may infer CoverCanvas for an existing finite curve.

CoverCanvas retains authored turns as a minimum. Evaluation resolves Feature-scaled
spacing and turns, then generates at least the larger of that authored extent and
the extent required by the inverse-transformed canvas plus finite support and guard
requirements. Round spirals use a radial bound and a complete outer revolution;
square spirals use their own square/max-norm coverage bound. Rotation, translation,
phase and winding remain authored. Extending coverage preserves the existing inner
curve prefix and complete logical tone anchor; it changes neither artistic sampling
cadence nor numerical tolerances.

Every canvas-covering or repeated family must plan sufficient transformed generation
before certified pruning. Existing finite source semantics remain finite: an authored
arc or single-axis finite repetition does not become an infinite two-dimensional
family. Stochastic and artwork-weighted gaps are not themselves coverage failures.
The implementation retains bounded cancellation, checked/fallible growth, complete
family/topology/source-profile authority, downstream final clipping and shared
canonical preview/PNG/SVG construction while avoiding unneeded materialization.

Strict calibration binds the explicit policy and authored structural intent.
Whole-Pattern X/Y translation and rotation do not change the calibration binding,
profile, numeric artifact or desired response endpoints and require no new fit;
they change only the required derived generation envelope.
CoverCanvas uses producer suffix `+cover-envelope-v1` after applicable existing
producer suffixes and receives a genuine measurement for its exact reference
binding. A finite Fixed companion cannot be relabelled as CoverCanvas, including
when their centered geometry happens to coincide. Unaffected Fixed profiles and
numeric artifacts retain their meaning. Optional former CoverCanvas numeric
companions are discarded only through the recognized source-free resource route;
unknown profiles still reject and desired response endpoints remain unchanged.

Current project schema is **19**, Document Preset configuration schema **18** and
standalone Pattern format **10**. Container **2** and Document Preset envelope **3**
remain unchanged. Ordinary projects and standalone Patterns remain current-only.
Supported older source-free Document Presets deterministically make an absent
mechanism policy explicitly Fixed, preserving authored finite intent, existing
artistic controls, desired ALL/named/End endpoints and original archive bytes.
An explicitly stored generic recipe policy stays explicit; lost gallery intent is
never reconstructed by heuristic. Missing or changed strict bindings are genuinely
prepared before publication. A reported older ordinary project remains an immutable
before witness; any corrected current fixture receives coverage intent explicitly.

Maze construction uses the complete requested guard-site domain before topology,
including sites and bounded cells outside the canvas. This explicitly supersedes
the canvas-only admission and permitted fringe in the historical Stage 20M plan;
the canvas never supplies artificial walls, openings or boundary cells. The finite
Maze retains its connected bounded-cell dual, seeded spanning tree, two genuine
perimeter openings and solution even when its perimeter lies outside the final
canvas. Requested admission follows the family planner's Euclidean distance to
the original canvas, reconstructed from the actual request rather than a broader
cached generation domain. Broader-support reuse is permitted only when it preserves
the exact canonical topology, walls, openings, solution and fingerprint; otherwise
Maze regenerates the exact requested family while other products retain their
existing cache and certified-pruning policies.

Straight Maze and Guide Faces reserve complete-cell reach before family generation.
The bound uses the selected document-space guide normals and physical pitches,
including the existing aspect and rotation transforms. For each independent pair,
its reciprocal cell basis bounds the cell diameter by the longer of its two
fundamental-parallelogram diagonals; a third selected dimension may subdivide those
cells. This derived reach does not replace nominal mark normalization or alter
artistic response. It supplies intrinsic complete-cell support even with zero
configured guard steps; existing extra guard runway and positive outward-treatment
support remain. Arbitrary authored curved arrangements retain their separate
existing generation contract; the straight reciprocal-lattice proof does not claim
to certify arbitrary curved cells or stochastic Voronoi populations.

Corrected Maze geometry uses producer contract
`toniator-wall-maze-guard-topology-v2` and strict calibration suffix
`+maze-guard-topology-v2`; the changed Guide Faces reference producer adds
`+cell-reach-v1`. Applicable existing and CoverCanvas suffixes retain their order.
Former numeric companions are not relabelled. Supported source-free conversion
recognizes complete former typed producer profiles exactly, including mixed ordered
outputs, and drops obsolete numeric companions while preserving authored responses;
unknown or partial producer profiles still reject. New reference bindings receive
genuine measurements. Placement edits thereafter retain the same profile, numeric
artifact, binding and response endpoints without fitting.

The stable authored-guide core frame depends on canvas dimensions and authored
structural baseline orientation, and excludes both whole-Pattern rotation and X/Y
translation. Placement changes only the inverse-transformed generation demand.
NormalOffset extends one unchanged scaled authored core with two finite outer
terminal rays, sufficient for the maximum requested signed offset travel; it does
not introduce hidden longitudinal tiles, repeated source lobes or altered seam
handles. Genuine Tile repetition remains a separate authored operation. Original
controls, source intervals, the original-core tone zero and anchored cadence remain
authoritative. The changed authored-guide coverage producer uses
`+guide-ray-coverage-v1`; former measurements are not relabelled.

Ordinary Voronoi outputs from complete typed straight-intersection families also
reserve finite defining-neighbor reach before generation. For an independent guide
pair with reciprocal cell diameter bound D, the unmerged lattice has covering bound
R = D/2, and canvas-relevant complete cells require a 3R guard. The current direct
merge into an unchanged representative within authored epsilon gives the local
finite-population bound rho = D/2 + epsilon, so generation reserves at least
3rho = 1.5D + 3epsilon in addition to authored margin, existing extra guard runway
and outward-treatment support. Degenerate/nonfinite arithmetic, cancellation and
ordinary resource limits still reject atomically. The typed producer suffix is
`+voronoi-cell-reach-v1`; arbitrary curved, finite and random populations do not
acquire this straight-lattice contract.

Positive-epsilon greedy merging can depend on earlier exterior input. The exact
requested finite family therefore owns positive-epsilon straight-intersection
Maze and Voronoi topology. Their upstream cache admits exact support keys and
rejects broader-support substitution, while unaffected producers retain existing
broader reuse. This finite local coverage proof does not promise an infinite merged
pattern or identical topology between different requested supports. Independent
unmerged reference envelopes compare the same source/facet topology within the
existing canonical numerical policy; same-request cache and canonical consumer
comparisons remain exact. Lower-level Maze membership maps only exact absolute
contributor identities into an accepted family and fails atomically on ambiguity
or absence; coordinates and cache-local ordinals do not establish equivalence.
All newly applicable producer revisions participate together in exact prior-profile
recognition. Genuine new measurements precede publication; subsequent placement
continues to reuse unchanged artifacts and responses without fitting.
