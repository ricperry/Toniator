# Toniator Channel Schema

**Status:** Normative architecture specification  
**Applies to:** Greenfield Toniator rewrite  
**Related documents:** [PatternSchema.md](PatternSchema.md), [ArchitectureSchema.md](ArchitectureSchema.md), [ModuleStructure.md](ModuleStructure.md)

---
Noted exceptions can be found in `Addendum.md`.
---

## 1. Purpose

This document defines the channel schema, the document-to-channel effective
pattern boundary, and the responsibilities of the channel editor.

The pattern editor defines the structural pattern:

- Grid or random family.
- One to four guide dimensions.
- Guide prototypes and baseline angles.
- Site-generation rules.
- Mark, connected, or region output.
- Voronoi as a downstream construction.
- Region-offset support.

The document owns the base recipe and common pattern settings. The channel
editor controls a selected channel's optional recipe replacement, applicable
typed deltas, translation, source mapping, color, and display:

- Density/detail delta.
- Pattern-rotation delta.
- X and Y offset.
- Output-specific geometry-response deltas.
- Channel color.
- Opacity.
- Visibility.
- Source-channel mapping and related channel presentation settings.

The channel editor must not create a second pattern state or duplicate pattern
mathematics. The domain resolves and validates every effective value.

---

## 2. Architectural invariants

1. The document owns one base `DocumentPatternSettings` value.
2. Every channel resolves one `EffectiveChannelPatternInstance` from that base
   setting plus its optional replacement recipe and typed deltas.
3. A channel recipe replacement references one structural `PatternDefinition`;
   pattern definitions may be shared across channels.
4. Rotation, density/detail, shape rotation, and output-specific response
   ranges are inherited unless a channel supplies an applicable additive delta.
   Translation, source mapping, color, opacity, and visibility remain
   channel-specific.
5. All user-authored and serialized numeric values are `f64`.
6. Literal pixel spacing is never the canonical persisted density representation.
7. Actual generated site counts are derived internal values.
8. Changing an effective density, rotation, translation, aspect, or source
   dimension invalidates and regenerates family output.
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
pub struct DocumentPatternSettings {
    pub definition_id: PatternDefinitionId,
    pub density: DensityMetric2D,
    pub pattern_rotation_degrees: f64,
    pub shape_rotation_degrees: f64,
    pub geometry_response: PatternGeometryResponse,
}
```

`DocumentPatternSettings` is the persisted base. `PatternGeometryResponse`
is an output-specific typed union; it does not merge marks, paths, and regions
into one untyped fill field.

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
    pub definition_override: Option<PatternDefinitionId>,
    pub layout_delta: ChannelPatternLayoutDelta,
    pub shape_rotation_delta_degrees: Option<f64>,
    pub geometry_response_delta: Option<ChannelGeometryResponseDelta>,
}
```

The instance contains only optional channel-level variation. An absent override
or delta inherits the document value; reset removes the stored override/delta.
Structural guide definitions, site-generation rules, connection topology, and
region-source selection remain in the resolved pattern definition.

`shape_rotation_delta_degrees` adds to the document's
`shape_rotation_degrees` only for mark realization. It is absent for inherited
shape rotation and is invalid for outputs that do not realize marks.

The domain exposes the resolved value as an
`EffectiveChannelPatternInstance`. Consumers do not reconstruct it from
widgets, serialized defaults, or cached preview state.

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

Both the document base-density command and a channel density-delta command must
identify which control was authoritative during the edit. A document edit
changes the base `DensityMetric2D`; a channel edit changes only its additive
`DensityMetricDelta2D`. In either case, the command derives the companion axis
when aspect lock is enabled rather than persisting a broken effective metric.

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
pub struct ChannelPatternLayoutDelta {
    pub density: Option<DensityMetricDelta2D>,
    pub rotation_degrees: Option<f64>,
    pub offset_x: f64,
    pub offset_y: f64,
}
```

```rust
pub struct DensityMetricDelta2D {
    pub across_x_delta: f64,
    pub across_y_delta: f64,
}
```

`density` and `rotation_degrees` are typed additive deltas. A density delta
must preserve the base metric's aspect-lock invariant. `offset_x` and
`offset_y` remain channel-owned translations. A later implementation may use a
more compact serialized representation, but it must preserve the same explicit
inherit/reset semantics and must not silently materialize a base value.

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
Effective density, pattern rotation, and channel translation
→ padded canvas
→ inverse transform into pattern-local coordinates
→ directional frequency resolution
→ family generation over the complete local domain
→ document-coordinate transform
→ pattern realization
→ effective geometry response
→ exact canvas clipping
```

The implementation must never generate a finite grid for an unrotated canvas and then rotate it afterward.

---

## 8. Effective geometry response

```rust
pub enum PatternGeometryResponse {
    Marks(MarkGeometryResponse),
    Connected(ConnectedGeometryResponse),
    Regions(RegionGeometryResponse),
}

pub enum ChannelGeometryResponseDelta {
    Marks(MarkGeometryResponseDelta),
    Connected(ConnectedGeometryResponseDelta),
    Regions(RegionGeometryResponseDelta),
}

pub struct MarkGeometryResponseDelta {
    pub minimum_size_delta: Option<f64>,
    pub maximum_size_delta: Option<f64>,
}

pub struct ConnectedGeometryResponseDelta {
    pub minimum_thickness_delta: Option<f64>,
    pub maximum_thickness_delta: Option<f64>,
}

pub struct RegionGeometryResponseDelta {
    pub minimum_inset_delta: Option<f64>,
    pub maximum_inset_delta: Option<f64>,
}
```

The document stores a `PatternGeometryResponse`; the channel stores only a
matching optional `ChannelGeometryResponseDelta`. Only the branch compatible
with the effective pattern output is valid. Each response-delta field is an
optional additive minimum or maximum scalar; an absent response delta inherits
the complete document response.

### 8.1 Marks and shapes

```rust
pub struct MarkGeometryResponse {
    pub minimum_size: f64,
    pub maximum_size: f64,
    pub response_curve: ResponseCurve,
    pub polarity: Polarity,
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

The channel response delta does not change where Voronoi sites were generated.
Each delta applies only to a matching output-specific base response. A missing
delta inherits the document response, and a reset removes only that delta.
`response_curve` and `polarity` remain document-base, output-typed fields;
they are not additive channel deltas.

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

## 11. Inheritance and recipe replacement

Every channel starts with the document recipe. An explicit
`definition_override` replaces the recipe only for that selected channel; it
does not mutate or relink the document base or any sibling channel. The
replacement may reference a shared structural definition, including an exact
copy created by an explicit copy-on-edit command.

Per-channel state remains independent:

- optional recipe replacement;
- typed density/detail, pattern-rotation, shape-rotation, and response deltas;
- translation;
- color, opacity, and visibility; and
- source mapping.

Removing a replacement restores the document recipe. Changing an override or
copy relationship must be an explicit document command and must never silently
overwrite another channel's definition.

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
│   ├── Override document pattern
│   ├── Reset to document pattern
│   └── Edit Pattern…
├── Override document settings
│   ├── Density/detail delta
│   ├── Pattern/shape rotation delta
│   └── Typed response delta
├── Layout
│   ├── X translation
│   └── Y translation
└── Geometry
    └── Output-specific effective response summary
```

The UI must show only controls compatible with the active family and output schema.

`Edit Pattern…` opens the structural pattern workflow described in
`PatternSchema.md`. The workflow may edit the document base or create an
explicit selected-channel recipe replacement; it must not treat a channel as
an independent hidden copy of the document settings.

---

## 13. Authoritative commands

Every edit becomes a document command.

```rust
pub enum DocumentPatternCommand {
    SetDocumentPatternSettings {
        settings: DocumentPatternSettings,
    },
}

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

    SetDensityDelta {
        channel: ChannelId,
        delta: DensityMetricDelta2D,
    },

    ResetDensityDelta {
        channel: ChannelId,
    },

    SetPatternRotationDelta {
        channel: ChannelId,
        degrees: f64,
    },

    ResetPatternRotationDelta {
        channel: ChannelId,
    },

    SetShapeRotationDelta {
        channel: ChannelId,
        degrees: f64,
    },

    ResetShapeRotationDelta {
        channel: ChannelId,
    },

    SetOffset {
        channel: ChannelId,
        x: f64,
        y: f64,
    },

    SetGeometryResponseDelta {
        channel: ChannelId,
        delta: ChannelGeometryResponseDelta,
    },

    ResetGeometryResponseDelta {
        channel: ChannelId,
    },

    SetSourceMapping {
        channel: ChannelId,
        mapping: ChannelSourceMapping,
    },

    SetPatternDefinitionOverride {
        channel: ChannelId,
        definition_id: Option<PatternDefinitionId>,
    },
}
```

Requirements:

- Commands validate before commit.
- A document-pattern command resolves every affected channel through the
  domain before publication and reports the resulting ordered channels and
  strongest invalidation.
- A channel-delta command validates the effective value against the current
  document base; reset removes the delta instead of copying that base.
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

- Effective density/detail.
- Effective pattern rotation.
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
- Document density serializes as `across_x`, `across_y`, and `aspect_locked`.
- Channel density, pattern rotation, shape rotation, and geometry response
  serialize only as explicit typed deltas or an explicit inherit/reset state;
  they never serialize as copied effective base values.
- Literal derived pixel spacing is not serialized.
- Actual generated site counts are not serialized unless used only as an optional cache that can be discarded.
- Pattern IDs are stable.
- Channel IDs are stable.
- Shared-definition references survive save/load.
- Unknown schema versions fail clearly or migrate deterministically.
- Document settings, channel recipe replacements, channel deltas, translation,
  visibility, color, opacity, and source mapping round-trip exactly.

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

- A 900 × 600 source can persist 90.0 across X and 60.0 across Y in the
  document base metric.
- SVG output scales without changing the authored density relationship.
- Locking aspect maintains equal nominal document-space spacing.
- Unlocking aspect permits independent X and Y density.
- Random, one-guide, two-guide, three-guide, and four-guide families all consume the same density metric.
- Rotating or offsetting a channel causes regeneration with no uncovered canvas edges.
- Shape size, line thickness, and cell inset use output-specific document bases
  plus matching channel deltas, independently of family site placement.
- Color, opacity, and visibility can change without regenerating family geometry.
- Two channels may inherit one document recipe or select replacement recipes
  while using different typed deltas, translations, colors, and responses.
- Save/load and undo/redo preserve all channel state exactly.
