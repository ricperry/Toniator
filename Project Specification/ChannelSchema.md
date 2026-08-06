# Toniator Channel Schema

**Status:** Normative architecture specification  
**Applies to:** Greenfield Toniator rewrite  
**Related documents:** [PatternSchema.md](PatternSchema.md), [ArchitectureSchema.md](ArchitectureSchema.md), [ModuleStructure.md](ModuleStructure.md)

---
Noted exceptions can be found in `Addendum.md`.
---

## 1. Purpose

This document defines the per-channel schema and the responsibilities of the channel editor.

The pattern editor defines the structural pattern:

- Grid or random family.
- One to four guide dimensions.
- Guide prototypes and baseline angles.
- Site-generation rules.
- Mark, connected, or region output.
- Voronoi as a downstream construction.
- Region-offset support.

The channel editor controls how one channel instantiates, sizes, transforms, colors, and displays that pattern:

- Density.
- Density aspect and aspect locking.
- Rotation.
- X and Y offset.
- Shape size.
- Curve or network thickness.
- Region or Voronoi cell inset.
- Channel color.
- Opacity.
- Visibility.
- Source-channel mapping and related channel presentation settings.

The channel editor must not create a second pattern state or duplicate pattern mathematics.

---

## 2. Architectural invariants

1. Every channel owns one `ChannelPatternInstance`.
2. A channel instance references one structural `PatternDefinition`.
3. Pattern definitions may be shared across channels.
4. Density, transform, geometry response, color, opacity, and visibility remain channel-specific.
5. All user-authored and serialized numeric values are `f64`.
6. Literal pixel spacing is never the canonical persisted density representation.
7. Actual generated site counts are derived internal values.
8. Changing density, rotation, offset, aspect, or source dimensions invalidates and regenerates family output.
9. Visibility does not destroy or reset channel state.
10. Color and opacity do not alter canonical geometry.
11. The channel editor dispatches document commands; widgets do not mutate hidden renderer or adapter state.
12. Undo and redo operate on authoritative document commands.
13. Channel output is deterministic for the same document, source, pattern definition, channel instance, and seed.

---

## 3. Channel state

```rust
pub struct ChannelState {
    pub id: ChannelId,
    pub name: String,

    pub appearance: ChannelAppearance,
    pub source_mapping: ChannelSourceMapping,
    pub pattern: ChannelPatternInstance,
}
```

```rust
pub struct ChannelAppearance {
    pub visible: bool,
    pub color: ColorValue,
    pub opacity: f64,
}
```

`visible` is Boolean state. All numeric components of `ColorValue`, including alpha where represented numerically, serialize as `f64`.

---

## 4. Channel pattern instance

```rust
pub struct ChannelPatternInstance {
    pub definition_id: PatternDefinitionId,
    pub layout: ChannelPatternLayout,
    pub geometry_response: ChannelGeometryResponse,
}
```

The instance contains only channel-level configuration. Structural guide definitions, site-generation rules, connection topology, and region-source selection remain in the referenced pattern definition.

---

## 5. Density model

### 5.1 Canonical density metric

```rust
pub struct DensityMetric2D {
    pub across_x: f64,
    pub across_y: f64,
    pub aspect_locked: bool,
}
```

For a 900 × 600 document:

```text
across_x = 90.0
across_y = 60.0
```

When aspect is locked, this represents equal nominal spacing in document coordinates.

Derived axis spacing is:

```text
spacing_x = canvas_width / across_x
spacing_y = canvas_height / across_y
```

These spacing values are transient evaluator results. They are not the canonical persisted setting.

### 5.2 Aspect lock

When `aspect_locked = true`, changing one axis derives the other so nominal document-space spacing remains isotropic:

```text
across_y = across_x × canvas_height / canvas_width
```

or:

```text
across_x = across_y × canvas_width / canvas_height
```

The channel command must identify which control was authoritative during the edit.

When `aspect_locked = false`, `across_x` and `across_y` are independent.

### 5.3 Why density is not a stored site count

The same density metric must support:

- Random sites.
- One-guide families.
- Two-guide grids.
- Three-guide triangular grids.
- Four-guide grids.
- Curved and warped guide layouts.
- Along-guide samples.
- Grid intersections.
- Voronoi based on any family-generated site layout.

A two-guide orthogonal grid may produce approximately:

```text
90 × 60 = 5,400 intersections
```

A random family may interpret the same metric as an expected site population near 5,400.

A one-guide family has no meaningful row-and-column count.

A triangular grid has three directional guide frequencies and cannot be represented faithfully as only X rows and Y columns.

Therefore, the persisted state remains a continuous two-dimensional density metric.

### 5.4 Family-specific UI presentation

The channel editor presents the same underlying metric differently according to pattern metadata.

```rust
pub enum DensityPresentation {
    PlanarAxes,
    ScalarEquivalent,
    AlongAndAcrossGuide,
    ScalarWithAdvancedAnisotropy,
}
```

#### Two-guide grid

```text
Across X: 90.0
Across Y: 60.0
Lock spacing aspect: On
```

#### Random family

```text
Density: 5,400.0 equivalent sites
Advanced anisotropy: hidden by default
```

The scalar can be derived from the metric, commonly through area-equivalent density:

```text
equivalent_density = across_x × across_y
```

#### One-guide family

```text
Across guides: 60.0
Along guides: 90.0
Lock density aspect: On
```

The evaluator resolves these controls into the shared metric relative to the guide’s local tangent and normal.

#### Three- or four-guide grid

```text
Density: 75.0
Aspect: 1.0
Advanced X/Y density: optional
```

The UI must not imply that the scalar is an exact count of sites across a canvas axis.

---

## 6. Pattern layout transform

```rust
pub struct ChannelPatternLayout {
    pub density: DensityMetric2D,
    pub rotation_degrees: f64,
    pub offset_x: f64,
    pub offset_y: f64,
}
```

### 6.1 Rotation

Rotation applies to the pattern coordinate frame.

Requirements:

- Rotation is not applied to a precomputed finite grid.
- Coverage planning receives rotation before guide or site generation.
- Rotation values may be normalized for display but should preserve stable serialized behavior.

### 6.2 Offset

X and Y offset move the pattern coordinate frame.

Requirements:

- Periodic families may normalize offsets to a stable phase.
- Nonperiodic families retain the literal offset.
- Offset participates in coverage planning.
- Large offsets must not cause unnecessary unbounded generation.

### 6.3 Aspect

Ordinary pattern aspect belongs to the density metric, not a second affine X/Y stretch.

This avoids two competing systems:

- Independent X/Y density.
- Post-generation geometric scaling.

A later advanced affine deformation may be added as a separate feature, but it must not replace the standard density-aspect controls.

---

## 7. Coverage invalidation and regeneration

Changing any of the following requires family regeneration:

- `across_x`
- `across_y`
- Aspect lock
- Rotation
- X offset
- Y offset
- Canvas size
- Source artwork dimensions when document coordinates depend on them
- Pattern definition
- Guide prototype
- Guide baseline angle
- Repetition mode
- Random seed
- Weighting strength
- Site-generation rule

Required evaluation order:

```text
Channel density and layout
→ padded canvas
→ inverse transform into pattern-local coordinates
→ directional frequency resolution
→ family generation over the complete local domain
→ document-coordinate transform
→ pattern realization
→ channel geometry response
→ exact canvas clipping
```

The implementation must never generate a finite grid for an unrotated canvas and then rotate it afterward.

---

## 8. Channel geometry response

```rust
pub enum ChannelGeometryResponse {
    Marks(MarkGeometryResponse),
    Connected(ConnectedGeometryResponse),
    Regions(RegionGeometryResponse),
}
```

Only the branch compatible with the active pattern output is valid.

### 8.1 Marks and shapes

```rust
pub struct MarkGeometryResponse {
    pub minimum_size: f64,
    pub maximum_size: f64,
    pub response_curve: ResponseCurve,
    pub polarity: Polarity,
    pub rotation_offset_degrees: f64,
}
```

The UI label should use the concrete term:

```text
Shape size
```

The size unit is a document-space scalar or normalized value defined by the canonical geometry contract. It is not tied to screen pixels.

### 8.2 Curves and networks

```rust
pub struct ConnectedGeometryResponse {
    pub minimum_thickness: f64,
    pub maximum_thickness: f64,
    pub response_curve: ResponseCurve,
    pub polarity: Polarity,
}
```

The UI label adapts to the output:

- Curve thickness.
- Line thickness.
- Network thickness.
- Maze thickness.

### 8.3 Regions and cells

```rust
pub struct RegionGeometryResponse {
    pub minimum_inset: f64,
    pub maximum_inset: f64,
    pub response_curve: ResponseCurve,
    pub polarity: Polarity,
}
```

For Voronoi, the UI may say:

- Cell size.
- Cell inset.
- Cell fill.

Internally, the preferred operation is a signed region offset using reusable Bezziator-style shrink/grow infrastructure.

The channel response does not change where Voronoi sites were generated.

---

## 9. Color and visibility

### 9.1 Color

```rust
pub struct ColorValue {
    pub red: f64,
    pub green: f64,
    pub blue: f64,
    pub alpha: f64,
}
```

The UI may accept:

- Hex.
- RGB components.
- CMYK-mapped channel defaults.
- Preset channel colors.

The canonical internal representation must be documented and consistently converted for preview and export.

### 9.2 Opacity

```text
0.0 <= opacity <= 1.0
```

Opacity is channel presentation state. It must not be baked into source sampling or geometry generation.

### 9.3 Visibility

```rust
pub bool visible;
```

Visibility controls whether the channel contributes to:

- Composite preview.
- Export, subject to export policy.
- Optional solo or isolation modes.

Visibility does not:

- Delete geometry settings.
- Reset the pattern.
- Alter density.
- Change source mapping.
- Remove the channel from undo history.

### 9.4 Selection and solo state

Selected-channel and temporary solo/isolation state are UI-session state unless the product specification explicitly requires persistence.

They should not be confused with the authoritative `visible` setting.

---

## 10. Source mapping

```rust
pub struct ChannelSourceMapping {
    pub source_component: SourceComponent,
    pub inversion: bool,
    pub sampling_gain: f64,
    pub sampling_bias: f64,
}
```

Potential source components include:

- Cyan.
- Magenta.
- Yellow.
- Black.
- Red.
- Green.
- Blue.
- Alpha.
- Luminance.
- User-defined derived field.

Source mapping controls the field sampled by modulation and weighted site distribution. It does not own the pattern family.

---

## 11. Shared versus independent pattern definitions

```rust
pub enum PatternLinkMode {
    SharedDefinition,
    IndependentDefinition,
}
```

### Shared definition

Multiple channels reference the same structural `PatternDefinition`.

Per-channel settings remain independent:

- Density.
- Rotation.
- Offset.
- Shape size or line thickness.
- Cell inset.
- Color.
- Opacity.
- Visibility.
- Source mapping.

### Independent definition

A channel references its own copied or separately edited definition.

Changing link mode must be an explicit document command and must never silently overwrite another channel’s definition.

---

## 12. Channel editor layout

Recommended inspector structure:

```text
Channel
├── Visibility
├── Color
│   ├── Hex
│   ├── Color picker
│   └── Opacity
├── Source
│   ├── Source component
│   ├── Invert
│   └── Gain/Bias
├── Pattern
│   ├── Pattern selection
│   ├── Link mode
│   └── Edit Pattern…
├── Density
│   ├── Family-appropriate density controls
│   ├── Aspect lock
│   └── Derived estimate
├── Layout
│   ├── Rotation
│   ├── X offset
│   └── Y offset
└── Geometry
    └── Shape size, line thickness, or cell inset
```

The UI must show only controls compatible with the active family and output schema.

`Edit Pattern…` opens the structural pattern editor described in `PatternSchema.md`.

---

## 13. Authoritative commands

Every edit becomes a document command.

```rust
pub enum ChannelCommand {
    SetVisibility {
        channel: ChannelId,
        visible: bool,
    },

    SetColor {
        channel: ChannelId,
        color: ColorValue,
    },

    SetOpacity {
        channel: ChannelId,
        opacity: f64,
    },

    SetDensity {
        channel: ChannelId,
        density: DensityMetric2D,
        edited_axis: DensityEditedAxis,
    },

    SetRotation {
        channel: ChannelId,
        degrees: f64,
    },

    SetOffset {
        channel: ChannelId,
        x: f64,
        y: f64,
    },

    SetGeometryResponse {
        channel: ChannelId,
        response: ChannelGeometryResponse,
    },

    SetSourceMapping {
        channel: ChannelId,
        mapping: ChannelSourceMapping,
    },

    SetPatternDefinition {
        channel: ChannelId,
        definition_id: PatternDefinitionId,
    },

    SetPatternLinkMode {
        channel: ChannelId,
        mode: PatternLinkMode,
    },
}
```

Requirements:

- Commands validate before commit.
- Undo records authoritative before-and-after state.
- Transient widget text is not authoritative state.
- Renderer adapters are read-only.
- Regeneration is scheduled from committed state changes.

---

## 14. Invalidation classes

```rust
pub enum ChannelInvalidation {
    PresentationOnly,
    GeometryResponse,
    FamilyRegeneration,
    FullSourceResample,
}
```

### Presentation only

- Visibility.
- Color.
- Opacity.

No geometry regeneration is required.

### Geometry response

- Mark size.
- Line thickness.
- Cell inset.
- Response curve.
- Polarity.

Family sites and guides may be reused if unchanged.

### Family regeneration

- Density.
- Rotation.
- Offset.
- Aspect.
- Seed.
- Pattern definition.
- Guide layout.
- Site-generation rules.

### Full source resample

- Source artwork change.
- Channel mapping change.
- Sampling-field change.
- Source dimensions affecting document coordinates.

The cache system must key results by the relevant authoritative inputs.

---

## 15. Serialization

Requirements:

- All authored numeric values serialize as `f64`.
- Color hex is a UI representation; canonical color components serialize numerically.
- Density serializes as `across_x`, `across_y`, and `aspect_locked`.
- Literal derived pixel spacing is not serialized.
- Actual generated site counts are not serialized unless used only as an optional cache that can be discarded.
- Pattern IDs are stable.
- Channel IDs are stable.
- Shared-definition references survive save/load.
- Unknown schema versions fail clearly or migrate deterministically.
- Visibility, color, opacity, density, transform, geometry response, and source mapping round-trip exactly.

---

## 16. Validation

The channel validator must reject:

- Non-finite numbers.
- Zero or negative density.
- Negative size or thickness ranges.
- Opacity outside `[0.0, 1.0]`.
- Invalid color components.
- A geometry response incompatible with the pattern output.
- A missing pattern definition.
- Unsupported source mappings.
- Aspect-lock edits that cannot be resolved from valid canvas dimensions.
- Transforms that cannot be inverted for coverage planning.
- Region inset settings unsupported by the active region treatment.

Validation must occur before authoritative state is committed.

---

## 17. Preview behavior

Interactive editing may use staged quality:

1. Commit authoritative channel state.
2. Invalidate the correct pipeline layer.
3. Render a fast preview using the same schema and canonical geometry contract.
4. Schedule full-quality reevaluation.
5. Replace the preview only when the result corresponds to the latest document revision.

A preview must not use a separate interpretation of density, rotation, or edge coverage.

---

## 18. Acceptance criteria

The channel schema is acceptable when:

- A 900 × 600 source can persist 90.0 across X and 60.0 across Y.
- SVG output scales without changing the authored density relationship.
- Locking aspect maintains equal nominal document-space spacing.
- Unlocking aspect permits independent X and Y density.
- Random, one-guide, two-guide, three-guide, and four-guide families all consume the same density metric.
- Rotating or offsetting a channel causes regeneration with no uncovered canvas edges.
- Shape size, line thickness, and cell inset are per-channel and independent of family site placement.
- Color, opacity, and visibility can change without regenerating family geometry.
- Two channels may share one pattern definition while using different density, rotation, offsets, colors, and size responses.
- Save/load and undo/redo preserve all channel state exactly.
