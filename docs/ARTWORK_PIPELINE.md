# Toniator Artwork Pipeline Architecture

**Status:** Stage 0 architecture contract; confirmed facts are recorded in `ARTWORK_PIPELINE_AUDIT.md`

**Primary issue:** TON-012 — Separate artwork source sampling, output model, and channel assignment

**Related issues:** TON-008, TON-009, TON-010, TON-011

**Audience:** Toniator maintainers, Codex agents, reviewers, and future contributors

> **Stage 1A implementation note (2026-07-21):** the independent vocabulary
> and migration-bounded adapters now live in `src/artwork_pipeline.rs`. They
> are not yet authoritative `Document` state, serialized state, GTK state, or
> renderer input. Stage 1A deliberately supersedes this document's earlier
> unshipped `encoded_rec709_luma_darkness_v1` compatibility spelling with the
> canonical ID `source.legacy_brightness.encoded_rec709_inverted_v1`. The
> semantic formula remains the audited inverted encoded Rec.709 luma.

---

## 1. Purpose

This document defines Toniator’s intended artwork-processing architecture.

Its primary goal is to separate concepts that the current application partially combines:

1. **Artwork Source** — what information is sampled from imported artwork.
2. **Source Alpha Policy** — how source transparency participates in sampling and coverage.
3. **Output Model** — which printable or display-oriented channel system Toniator generates.
4. **Channel Assignment** — where sampled scalar information is routed.
5. **Pattern Generation** — how each resolved output-channel field becomes marks or paths.
6. **Channel Compositing** — how generated channels combine into ordinary artwork.
7. **Output Treatment** — optional post-compositing production treatment such as DTF.
8. **Presentation and Export** — how resolved artwork is previewed and serialized.

The architecture must support the current CMYK and RGB workflows while providing stable foundations for:

- TON-008 RGB completion;
- TON-010 extensible halftone patterns;
- TON-011 Advanced Pattern Mixing;
- TON-009 DTF output treatment.

This document is normative for new design work unless a later architecture decision explicitly supersedes part of it.

---

## 2. Scope

This document defines:

- domain terminology;
- processing order;
- state ownership;
- stable identifiers;
- model invariants;
- renderer boundaries;
- persistence requirements;
- migration expectations;
- integration boundaries among TON-008 through TON-012;
- testing and review expectations.

This document does **not** specify:

- every GTK widget or visual layout;
- exact Rust type names;
- a printer-specific CMYK separation algorithm;
- an ICC color-management system;
- the complete TON-010 pattern catalog;
- the complete TON-009 DTF implementation;
- a node-based compositor;
- arbitrary layer graphs.

Implementation details may differ from the conceptual examples, but they must preserve the stated responsibilities and invariants.

---

## 3. Architectural principles

### 3.1 Separate source interpretation from output generation

Choosing what Toniator reads from an image must not implicitly choose what Toniator generates.

Examples that must be valid:

```text
Full Color + CMYK Print
Full Color + RGB Screen
Perceptual Lightness + CMYK Print
Perceptual Lightness + RGB Screen
Red Source Channel + CMYK Print
Alpha + RGB Screen
```

### 3.2 Separate scalar routing from color-model selection

A scalar source such as Lightness does not inherently mean:

- one ink;
- all inks;
- CMYK;
- RGB;
- Crosshatch.

Routing is an independent decision.

### 3.3 Patterns consume resolved channel fields

A pattern generator receives a channel-specific scalar field and coverage information.

A pattern generator must not independently:

- choose the output model;
- reinterpret the original image;
- infer whether it is generating CMYK or RGB from UI labels;
- inspect GTK state;
- apply DTF treatment.

### 3.4 Ordinary artwork resolves before output treatment

CMYK or RGB channel generation and compositing produce a complete ordinary artwork result.

Optional production treatments consume that resolved result afterward.

DTF is therefore an output treatment, not an output color model and not a pattern.

### 3.5 Preview and exports share canonical data

Live preview, PNG export, and SVG export must consume equivalent canonical geometry and resolved channel state.

No output path may silently reinterpret source values or regenerate a pattern under different rules.

### 3.6 Stable semantic identities replace UI indexes

Serialized state and renderer logic must use stable identifiers, not:

- dropdown positions;
- translated labels;
- widget ordering;
- enum declaration order when migration would become unsafe.

### 3.7 Basic workflows remain simple

The standard workflow should expose only the controls needed for ordinary use.

Advanced per-channel source and pattern overrides may exist later, but must not make the default workflow difficult to understand.

### 3.8 Migration preserves existing artwork

Legacy documents and presets must retain their established output where practical.

A cleaner new name must not be substituted for an old behavior until the old behavior’s actual formula and semantics are known.

---

## 4. Terminology

### 4.1 Source artwork

The imported raster or vector-derived image data from which Toniator samples color, tone, and transparency.

### 4.2 Artwork Source

The data extracted from the source artwork before output-channel generation.

Examples:

- Full Color;
- Red Channel;
- Green Channel;
- Blue Channel;
- Value;
- Perceptual Lightness;
- Alpha.

### 4.3 Source Alpha Policy

The rule controlling whether source opacity limits or modifies artwork generation.

Initial policies:

- Preserve Source Alpha;
- Ignore Source Alpha.

### 4.4 Source field

A sampled scalar or color field derived from the source artwork.

A source field is conceptually independent of the pattern that later consumes it.

### 4.5 Output Model

The channel system Toniator generates.

Initial models:

- CMYK Print;
- RGB Screen.

### 4.6 Output channel

A stable semantic destination such as:

- Cyan;
- Magenta;
- Yellow;
- Black;
- Red;
- Green;
- Blue.

### 4.7 Channel Assignment

The rule that maps sampled source data into output-channel fields.

Initial assignment modes:

- Automatic Color Separation;
- Active Channel;
- All Channels.

### 4.8 Channel field

A normalized scalar field for one output channel after source sampling, alpha handling, and channel assignment.

Conceptually, a channel field answers:

> At each source location, how strongly should this output channel participate?

### 4.9 Pattern

A registered generator that converts a channel field into canonical marks or paths.

Examples:

- rectangular dot grid;
- triangular dot grid;
- wave line field;
- pointillism;
- custom SVG motif recipe.

### 4.10 Pattern instance

A configured use of one registered pattern, including parameters, transforms, seed state, and optional custom assets.

### 4.11 Ordinary artwork

The resolved CMYK or RGB result before optional production treatment.

### 4.12 Output treatment

A post-compositing transformation or production preparation step.

DTF is the first planned output treatment.

### 4.13 Preview Surface

A viewing background used to help users judge transparent artwork.

It is not source data, not an output channel, and not exported unless separately represented as an explicit Export Background.

### 4.14 Export Background

An optional background intentionally included in applicable exports.

It is distinct from Preview Surface.

---

## 5. Processing pipeline

The normative processing order is:

```text
Source artwork
    ↓
Source decoding and normalization
    ↓
Artwork Source sampling
    ↓
Source Alpha Policy
    ↓
Channel Assignment or automatic color separation
    ↓
Resolved output-channel fields
    ↓
Pattern generation
    ↓
Canonical marks and paths per output channel
    ↓
Channel appearance and compositing
    ↓
Resolved ordinary artwork
    ↓
Optional output treatment
    ↓
Preview and export
```

A more detailed form is:

```text
Imported document or image
    ↓
Decode pixels, color values, alpha, document bounds, and transforms
    ↓
Sample Full Color or a selected scalar component
    ↓
Apply or intentionally ignore source transparency
    ↓
Convert or route sampled data into CMYK or RGB channel fields
    ↓
Generate per-channel marks or paths through registered patterns
    ↓
Apply channel visibility, color, opacity, clipping, and compositing
    ↓
Produce resolved Standard Artwork
    ↓
Optionally hand Standard Artwork to TON-009
    ↓
Render preview, PNG, SVG, or production-specific exports
```

### 5.1 Required boundary

The boundary between channel assignment and pattern generation must be explicit.

Pattern generators should consume resolved channel data rather than perform their own color separation.

This allows the same pattern to work with:

- Cyan;
- Magenta;
- Yellow;
- Black;
- Red;
- Green;
- Blue;
- future compatible output channels.

### 5.2 Confirmed legacy sampling boundary

Current source preparation and field sampling are two distinct resize stages.
`decode_source(..., 2400)` first caps the decoded source's long edge at 2400
pixels: oversized rasters use Lanczos3, while SVG is rendered by `resvg` at the
capped scale. Raster decoding supplies straight-alpha RGBA. The current SVG path
instead wraps `tiny_skia`'s premultiplied RGBA bytes without unpremultiplying.
Web Shapes and Curves then assume straight alpha, premultiply encoded RGBA,
resize to the requested field grid with Triangle filtering, and unpremultiply
nonzero-alpha samples. Stage 2 must preserve this complete boundary with
oversized raster/SVG and translucent-SVG fixtures unless an intentional sampling
change is separately approved.

---

## 6. Domain model

The following Rust-like types are conceptual.

```rust
enum ArtworkSource {
    FullColor,
    Red,
    Green,
    Blue,
    Value,
    PerceptualLightness,
    Alpha,
    LegacyBrightness(LegacyBrightnessKind),
}

enum SourceAlphaPolicy {
    Preserve,
    Ignore,
    LegacyCurrentV1,
}

enum OutputModel {
    CmykPrint,
    RgbScreen,
}

enum ChannelAssignment {
    AutomaticColorSeparation(SeparationStrategyId),
    ActiveChannel,
    AllChannels,
    Compatibility(ArtworkCompatibility),
}

struct ArtworkSamplingSettings {
    source: ArtworkSource,
    alpha_policy: SourceAlphaPolicy,
    assignment: ChannelAssignment,
    active_output_channel: Option<OutputChannelId>,
}

struct DocumentOutputSettings {
    output_model: OutputModel,
}

struct ResolvedChannelField {
    channel_id: OutputChannelId,
    values: ScalarField,
    coverage: CoverageMask,
}

struct ChannelAppearance {
    visible: bool,
    color: RenderColor,
    opacity: f32,
    blend_mode: BlendMode,
}

struct ResolvedArtwork {
    output_model: OutputModel,
    channels: Vec<ResolvedChannelArtwork>,
    bounds: DocumentBounds,
    transparency: TransparencyState,
}
```

The exact implementation may combine or split these structures.

The required architectural property is that:

- source selection;
- alpha handling;
- output model;
- channel assignment;
- pattern configuration;
- channel appearance;
- output treatment;

remain separately represented.

---

## 7. Stable identifiers

Recommended serialized identifiers follow a dotted semantic namespace.

### 7.1 Artwork sources

```text
source.full_color
source.red
source.green
source.blue
source.value
source.perceptual_lightness
source.alpha
source.legacy_brightness.<variant>
```

### 7.2 Source alpha policies

```text
source_alpha.preserve
source_alpha.ignore
source_alpha.legacy_current_v1
```

**Compatibility requirement:** `source_alpha.legacy_current_v1` preserves the
audited path-dependent alpha behavior of migrated work. It must not be presented
as equivalent to Preserve Source Alpha.

### 7.3 Output models

```text
output.cmyk_print
output.rgb_screen
```

### 7.4 Output channels

```text
channel.cmyk.cyan
channel.cmyk.magenta
channel.cmyk.yellow
channel.cmyk.black

channel.rgb.red
channel.rgb.green
channel.rgb.blue
```

### 7.5 Channel assignments

```text
assignment.automatic
assignment.active_channel
assignment.all_channels
assignment.compatibility
```

When `assignment.active_channel` is used, the selected stable channel ID must be serialized separately.

`assignment.compatibility` must serialize its compatibility payload. Stage 1
uses `compat.crosshatch.progressive_kcmy_v1`; it is mutually exclusive with
Automatic, Active Channel, and All Channels.

### 7.6 Pattern identifiers

TON-010 should define a separate namespace, for example:

```text
pattern.grid.rectangular_dots
pattern.grid.triangular_dots
pattern.lines.wave
pattern.stochastic.pointillism
pattern.custom.<uuid>
```

Display names may change without changing identifiers.

### 7.7 Confirmed compatibility identifiers

**Confirmed current behavior:** Stage 0 established the following stable IDs for
behavior that must survive migration without being renamed to a different
formula or concept:

```text
source.legacy_brightness.encoded_rec709_inverted_v1
separation.cmyk.encoded_rgb_max_black_v1
separation.rgb.direct_encoded_components_v1
compat.crosshatch.progressive_kcmy_v1
```

### 7.8 Stage 1A canonical identifier decision

The prior Stage 0 spelling is retained only in the historical note at the top
of this document. It was never shipped as a persisted identifier.
`ArtworkSource::LegacyBrightness` and
`LegacyBrightnessKind::EncodedRec709InvertedV1` use exactly:

```text
source.legacy_brightness.encoded_rec709_inverted_v1
```

The Stage 1A parser intentionally rejects the older spelling rather than
silently accepting two identities for one compatibility behavior. The module
also implements the other public semantic IDs listed in this architecture:
sources, source-alpha policies, output models, output channels, assignment
kind IDs, automatic-separation payload IDs, and the Crosshatch compatibility
payload ID. Labels are separate `label()` methods; no API depends on enum
order, legacy short ink IDs, or GTK positions.

### 7.9 Stage 1A domain and migration contract

`ArtworkPipelineSettings` independently contains `ArtworkSource`,
`SourceAlphaPolicy`, `OutputModel`, `ChannelAssignment`, and an optional stable
`OutputChannelId`. `validate()` is strict: automatic assignments require Full
Color and the strategy for the selected model; scalar Active/All assignments
require a scalar source; an Active assignment requires a compatible channel;
and Crosshatch is the exclusive
`LegacyCompatibilityAssignment::CrosshatchProgressiveKcmyV1` combination with
legacy Brightness and `LegacyCurrentV1`. A retained active channel is always
membership-checked. Validation does not repair input.

`normalize_legacy_active_channel()` is the explicit migration repair API and
`transition_output_model()` is the explicit user-transition API. Neither is
called by the live application in Stage 1A. CMYK channel order is C/M/Y/K and
RGB is R/G/B. Legacy scalar slots map only C/R, M/G, Y/B, and K; RGB slot 3 and
all other slots, including GTK invalid positions, are errors.

`LegacyPipelineSnapshot` and `LegacyPipelineConversion` are visibly
migration-bounded records. A snapshot retains the coupled legacy mapping,
serialized and current output, scalar destination and slot, Shapes/Curves (or
unsupported Native Basic) treatment, Crosshatch presence, and active/saved/
inactive origin. `pipeline_from_legacy()` maps only the five confirmed legacy
states, errors on unavailable/ambiguous/mismatched inputs, and keeps Crosshatch
outside RGB channel membership even when its stored output is RGB.

`project_legacy_value_mode()` projects valid settings back only to the five
representable legacy modes. Future source choices, modern alpha policies, and
other combinations return `LegacyProjectionError::UnsupportedReverseProjection`;
it never invents renderer behavior. Structured errors also cover unknown IDs,
strategy/model incompatibility, invalid or missing channels, invalid source /
assignment combinations, invalid slots, malformed snapshots, and unsupported
Crosshatch combinations.

Stage 1B remains responsible for making these settings authoritative and
migrating document/preset caches. No source sampling, alpha behavior, output
formula, Crosshatch formula, schema, persistence, UI, preview, PNG, or SVG
behavior changes in Stage 1A.

---

## 8. Artwork Source definitions

### 8.1 Full Color

Full Color preserves source RGB color information before output conversion.

It is the default Artwork Source.

#### Full Color + CMYK Print

Source RGB is converted into the documented CMYK channel fields.

**Confirmed current behavior:** The compatibility conversion operates directly
on normalized encoded RGB. It calculates `K = 1 - max(R, G, B)` and, except for
near-black where only K is 1, calculates each remaining component as
`(1 - component - K) / (1 - K)`, clamped to `[0, 1]`. It is identified as
`separation.cmyk.encoded_rgb_max_black_v1`.

**Target architecture:** New separation implementations may be added later, but
migration must preserve this formula unless the creator explicitly chooses a
different separation. Printer-profile support remains outside this baseline.

#### Full Color + RGB Screen

Source Red, Green, and Blue components map to their corresponding RGB output-channel fields.

**Confirmed current behavior:** Migration identifies this direct encoded-byte
component mapping as `separation.rgb.direct_encoded_components_v1`; it is a
versioned automatic-separation strategy, not an unversioned inference from RGB
output.

The output uses the established TON-008 Screen-compositing behavior.

### 8.2 Red, Green, and Blue

These sources produce scalar fields from the corresponding source RGB component.

Example:

```text
Artwork Source: Red Channel
Output Model: CMYK Print
Assignment: Active Channel → Cyan
```

This is valid. The source name does not constrain the output model.

### 8.3 Value

The target definition for new documents is:

```text
Value = max(R, G, B)
```

where RGB components are normalized consistently.

This corresponds to the Value component of HSV.

Value is not perceptually uniform.

### 8.4 Perceptual Lightness

The target definition for new documents is the `L` component of the perceptual color model Toniator adopts for internal perceptual operations.

The preferred initial model is OKLab lightness.

The implementation must document:

- source color encoding assumptions;
- whether conversion occurs from encoded or linear RGB;
- normalization range;
- numerical clamping.

The UI should use the creator-facing name:

```text
Perceptual Lightness
```

Help text should identify the actual model.

### 8.5 Alpha

Alpha uses source opacity as the scalar source field.

This supports silhouette and coverage-driven generation.

When Alpha is selected:

- alpha becomes sampled content;
- it must not be multiplied into the result a second time by the Source Alpha Policy;
- the resulting coverage behavior must be explicitly defined;
- preview, PNG, and SVG must agree.

A reasonable initial rule is:

```text
Artwork Source = Alpha
→ sampled scalar = source alpha
→ coverage mask = document/source bounds without reapplying alpha
```

The exact implementation may differ, but double application is prohibited.

### 8.6 Legacy Brightness

**Confirmed current behavior:** Existing Shapes and Curves use encoded
Rec.709-weighted luma darkness:

```text
encoded_luma = 0.2126 * R + 0.7152 * G + 0.0722 * B
legacy_brightness = 1 - encoded_luma
```

where encoded RGB components are normalized to `[0, 1]`. Black is 1 and white
is 0. This is not HSV Value, HSL Lightness, linear-light luminance, arithmetic
mean, maximum RGB, or perceptual lightness.

**Compatibility requirement:** Preserve it as
`source.legacy_brightness.encoded_rec709_inverted_v1`. Do not automatically
rename it to Value or Perceptual Lightness. Its partial-alpha behavior is
path-dependent and is documented in `ARTWORK_PIPELINE_AUDIT.md`.

---

## 9. Source Alpha Policy

### 9.1 Preserve Source Alpha

This is the default.

Source alpha participates in artwork coverage.

Requirements:

- fully transparent source regions do not produce unintended visible geometry;
- partial alpha produces proportional or otherwise documented coverage;
- transparent edges and internal holes remain consistent;
- preview, PNG, and SVG agree.

### 9.2 Ignore Source Alpha

Source opacity does not suppress generation.

This mode requires care because fully transparent pixels may contain arbitrary hidden RGB values.

The implementation must define one of the following:

- hidden RGB values are sampled as stored;
- transparent pixels are replaced with a documented matte color;
- transparent pixels are excluded from Full Color sampling but treated by another rule.

The initial implementation should avoid adding a matte selector unless required.

The UI should warn or explain that ignored transparency may expose hidden source colors.

### 9.3 Interaction with Alpha source

When Artwork Source is Alpha, the alpha policy must not accidentally apply alpha twice.

The UI may disable or reinterpret the alpha-policy control in this state if that makes the behavior clearer.

---

## 10. Output Model

### 10.1 CMYK Print

CMYK Print provides exactly:

- Cyan;
- Magenta;
- Yellow;
- Black.

It uses subtractive, print-oriented channel behavior.

The exact compositing preview is a simulation of channel interaction, not a replacement for full printer-profile proofing.

### 10.2 RGB Screen

RGB Screen provides exactly:

- Red;
- Green;
- Blue.

It uses the Screen-compositing behavior established by TON-008.

RGB must never retain a hidden fourth Black channel.

### 10.3 Mode-specific state

CMYK and RGB may retain separate channel-specific settings.

Switching:

```text
CMYK → RGB → CMYK
```

must restore prior CMYK state where valid.

Switching:

```text
RGB → CMYK → RGB
```

must restore prior RGB state where valid.

The selected Artwork Source should remain unchanged unless it is incompatible.

### 10.4 No implicit switching from source selection

Selecting:

- Red;
- Green;
- Blue;
- Value;
- Lightness;
- Alpha;

must not automatically change Output Model.

A full workflow preset may intentionally set both.

---

## 11. Channel Assignment

Channel Assignment maps sampled source data into output-channel fields.

### 11.1 Automatic Color Separation

This is normally available for Full Color.

#### CMYK

Full Color is converted into Cyan, Magenta, Yellow, and Black fields.

#### RGB

Full Color’s Red, Green, and Blue components map directly into Red, Green, and Blue fields.

### 11.2 Active Channel

A scalar source drives one selected output channel.

Examples:

```text
Perceptual Lightness → Black
Red Source Channel → Cyan
Alpha → Blue
```

The active destination must be a valid channel for the current Output Model.

### 11.3 All Channels

A scalar source drives every participating output channel.

Each channel may still have independent:

- visibility;
- color;
- opacity;
- transforms;
- pattern parameters;
- source-response parameters.

### 11.4 Future explicit routing

TON-011 may later support richer per-channel source overrides or routing.

That extension should not replace the basic model.

A future representation could support:

```rust
enum ChannelAssignment {
    AutomaticColorSeparation(SeparationStrategyId),
    ActiveChannel,
    AllChannels,
    Explicit(BTreeMap<OutputChannelId, ChannelSourceRule>),
}
```

`Explicit` is deferred unless TON-011 requires it.

---

## 12. Resolved channel fields

After source sampling, alpha policy, and assignment, Toniator produces one `ResolvedChannelField` for every ordinary output channel.

Each field contains:

- stable channel ID;
- normalized scalar values;
- coverage mask;
- document-space bounds;
- source transform information where required;
- generation/version identity for cancellation and stale-result checks.

### 12.1 Normalization

Channel fields should use a documented normalized range, preferably:

```text
0.0 = no channel contribution
1.0 = full channel contribution
```

Pattern-specific inversion or response curves must be explicit.

### 12.2 Coverage and intensity are separate

A scalar contribution and a coverage mask are not always the same concept.

This distinction is important for:

- transparent source edges;
- Alpha as source;
- clipping;
- custom document bounds;
- later DTF support.

---

## 13. Pattern-generation boundary

TON-010 owns the extensible pattern framework.

A pattern consumes a resolved channel field and produces canonical geometry.

Conceptually:

```rust
struct PatternContext<'a> {
    channel_id: OutputChannelId,
    source_field: &'a ScalarField,
    coverage_mask: &'a CoverageMask,
    bounds: DocumentBounds,
    transform: PatternTransform,
    sampling: SamplingSettings,
    seed_state: Option<PatternSeedState>,
    cancellation: CancellationToken,
}

enum PatternOutput {
    Marks(CanonicalMarks),
    Paths(CanonicalPaths),
}
```

### 13.1 Pattern responsibilities

A pattern may control:

- placement;
- topology;
- primitive or motif;
- deformation;
- spacing;
- repetition;
- path shape;
- stochastic distribution;
- source-response mapping;
- mark size or path width.

### 13.2 Pattern non-responsibilities

A pattern must not:

- select CMYK versus RGB;
- perform document-wide color separation;
- inspect GTK widgets;
- choose Preview Surface;
- add Export Background;
- derive DTF white base;
- perform final channel compositing;
- serialize project-wide output settings.

### 13.3 Shapes and Curves

After TON-010, Shapes and Curves should become pattern categories or output forms rather than mutually exclusive document-wide color-processing modes.

Before TON-010, compatibility adapters may preserve current behavior.

### 13.4 Crosshatch

Crosshatch is a pattern or specialized directional structure.

It is not an Artwork Source.

Until TON-010 supports it natively:

- existing Crosshatch documents must continue to work;
- the legacy source formula must be preserved;
- **Compatibility requirement:** `compat.crosshatch.progressive_kcmy_v1`
  maintains four monochrome Curves layers ordered K, C, M, Y, with default
  directions `45°`, `-45°`, `0°`, and `90°`;
- its progressive field partition and visibility-dependent redistribution must
  remain unchanged during TON-012;
- it is the exclusive `ChannelAssignment::Compatibility` value for migrated
  Crosshatch state, and the active output channel is absent;
- Stage 1 preserves today's blend-path mismatch: RGB-mode Crosshatch is Multiply
  in raster preview/PNG but Screen in SVG. A later parity stage may correct SVG
  only as an explicitly approved behavior change;
- TON-012 must stop treating Crosshatch as a source-mapping choice.

---

## 14. Channel appearance and compositing

Pattern generation produces geometry. Channel appearance determines how that geometry participates in the ordinary artwork result.

Per-channel appearance includes:

- visibility;
- display/export color;
- opacity;
- blend/compositing rule;
- clipping;
- semantic channel identity.

### 14.1 CMYK compositing

CMYK compositing must preserve established print-oriented behavior.

The renderer must use stable channel order:

```text
Cyan
Magenta
Yellow
Black
```

### 14.2 RGB compositing

RGB compositing must use stable order:

```text
Red
Green
Blue
```

and the Screen behavior established under TON-008.

### 14.3 Determinism

Render order must not depend on:

- active editor channel;
- widget ordering;
- registry insertion order;
- asynchronous worker completion order;
- edit history.

---

## 15. Resolved ordinary artwork

The output of ordinary channel compositing is `ResolvedArtwork`.

This is the authoritative result for:

- Standard Artwork preview;
- standard PNG export;
- standard SVG export;
- optional TON-009 handoff.

It must contain enough information to preserve:

- document bounds;
- transparency;
- channel grouping;
- canonical geometry references;
- output-model semantics;
- stable ordering.

---

## 16. Optional output treatments

Output treatments operate after ordinary artwork is resolved.

### 16.1 TON-009 ownership

DTF behavior belongs exclusively to TON-009.

TON-012, TON-010, and TON-011 must not independently implement:

- white base;
- knockout;
- garment preview;
- DTF-specific flattened export;
- layered DTF production output.

### 16.2 Optional availability

TON-009 may or may not be complete when other issues are implemented.

Therefore:

- ordinary artwork must work without TON-009;
- no placeholder DTF behavior should be invented;
- integration occurs only through TON-009’s established interface when available.

### 16.3 DTF handoff

When TON-009 is complete, the intended handoff is:

```text
Resolved ordinary artwork
    ↓
TON-009 knockout evaluation
    ↓
TON-009 merged white-base derivation
    ↓
TON-009 garment-preview or production export
```

DTF must not regenerate channel patterns.

### 16.4 Derived DTF elements are not output channels

The following must never appear as ordinary pattern-assignment targets:

- merged white base;
- knockout mask;
- garment-preview surface.

---

## 17. Preview and export architecture

### 17.1 Preview Surface

Preview Surface is composited only for display.

It must not affect:

- source sampling;
- channel fields;
- canonical geometry;
- PNG transparency;
- SVG transparency;
- DTF coverage.

### 17.2 Export Background

Export Background is an explicit export option.

It must be represented independently from Preview Surface.

### 17.3 PNG

PNG export consumes the same resolved ordinary artwork as preview, excluding Preview Surface.

It must preserve:

- transparency;
- channel appearance;
- compositing;
- export background when enabled.

### 17.4 SVG

SVG export consumes the same canonical geometry used by preview and PNG.

It should preserve stable, editable channel grouping.

Example:

```text
Toniator Artwork
  Cyan — <Pattern Name>
  Magenta — <Pattern Name>
  Yellow — <Pattern Name>
  Black — <Pattern Name>
```

or:

```text
Toniator Artwork
  Red — <Pattern Name>
  Green — <Pattern Name>
  Blue — <Pattern Name>
```

No RGB document may emit a hidden Black group.

### 17.5 Geometry parity

Preview, PNG, and SVG may rasterize or serialize differently, but they must represent equivalent geometry and channel state.

---

## 18. State ownership

### 18.1 Document-level state

Document-level state includes:

- source artwork reference or embedded data;
- Artwork Source;
- Source Alpha Policy;
- Output Model;
- Channel Assignment;
- document bounds;
- standard versus advanced pattern-assignment mode;
- optional output-treatment settings when implemented;
- export settings.

### 18.2 Output-model state

Mode-specific state includes:

- available channels;
- active channel;
- channel visibility;
- channel colors;
- channel opacity;
- mode-specific channel defaults;
- compatible pattern state where not shared.

### 18.3 Pattern-instance state

Pattern state includes:

- pattern ID;
- pattern version;
- parameters;
- transforms;
- seed policy;
- seed values;
- custom mark or path assets;
- inactive state retained for safe switching.

### 18.4 UI-only state

UI-only state includes:

- expanded/collapsed inspector sections;
- focus;
- open popovers;
- temporary hover state;
- transient selection while a model is being replaced;
- window geometry;
- user preference for offering advanced controls.

UI state must not silently alter document semantics.

### 18.5 Output-treatment state

TON-009-owned state includes:

- DTF enabled/disabled;
- garment-preview appearance;
- white-base policy;
- knockout settings;
- layered export settings.

---

## 19. Standard and advanced workflows

### 19.1 Standard workflow

The standard workflow should expose:

```text
Artwork Source
Source Alpha
Output Model
Apply To          [only when applicable]
Pattern
Channel controls
```

New documents default to:

```text
Artwork Source: Full Color
Source Alpha: Preserve
Output Model: CMYK Print
Channel Assignment: Automatic Color Separation
```

### 19.2 Conditional controls

Examples:

- Full Color + Automatic Separation does not need an Active/All selector.
- Scalar sources expose Active Channel or All Channels.
- Alpha source may hide or reinterpret Source Alpha.
- RGB hides Black-channel controls.
- CMYK hides RGB-only Screen terminology.

### 19.3 Advanced Pattern Mixing

TON-011 introduces deliberate per-channel pattern assignment.

The preferred extension model is:

```text
Standard:
  one document source configuration
  one shared pattern facade

Advanced:
  document source defaults
  optional per-channel source overrides
  one pattern instance per printable channel
```

Per-channel source overrides should be added only when TON-011 demonstrates the need.

The standard workflow must not expose advanced routing by default.

---

## 20. Presets

Presets must have explicit scope.

### 20.1 Source preset

May change:

- Artwork Source;
- Source Alpha Policy;
- source-sampling parameters.

Must not silently change Output Model.

### 20.2 Output preset

May change:

- Output Model;
- channel defaults;
- compositing-related settings.

Must not silently change Artwork Source.

### 20.3 Pattern preset

May change:

- pattern identity;
- pattern parameters;
- custom motif or path state;
- seed state where intentionally included.

Must not silently change source or output model unless defined as a complete workflow preset.

### 20.4 Complete workflow preset

May intentionally contain:

- source;
- alpha policy;
- output model;
- assignment;
- pattern;
- channel settings;
- export settings.

Its broader scope must be clear in name, UI, and undo behavior.

---

## 21. Persistence and versioning

### 21.1 Independent serialization

The document format must serialize independently:

- Artwork Source;
- Source Alpha Policy;
- Output Model;
- Channel Assignment;
- automatic-separation strategy ID when Assignment is Automatic;
- active channel;
- per-channel state;
- pattern state;
- output-treatment state when available.

### 21.2 Schema versions

At minimum, persist:

- document schema version;
- source semantics version where necessary;
- separation-strategy ID/version where assignment is automatic;
- output-model version where behavior may evolve;
- pattern schema and generator versions;
- custom recipe version;
- PRNG version for stochastic patterns.

### 21.3 Unknown values

Unknown stable identifiers must not silently become the first dropdown option.

Appropriate behavior includes:

- visible error;
- compatibility fallback with explicit warning;
- preserved opaque state for future recovery;
- disabled affected feature.

### 21.4 Portability

Custom pattern definitions and assets should be embedded or packaged when required to reopen a document on another system.

---

## 22. Legacy migration

TON-012 must audit the current implementation before finalizing migration.

### 22.1 Current combined mappings

**Confirmed compatibility requirement:**

| Legacy mapping | New Artwork Source | Alpha policy | New Output Model | New assignment / compatibility |
|---|---|---|---|---|
| Color → CMYK Inks / `cmyk` | Full Color | Legacy Current v1 | CMYK Print | Automatic using `separation.cmyk.encoded_rgb_max_black_v1` |
| RGB Color → Screen / `rgb` | Full Color | Legacy Current v1 | RGB Screen | Automatic using `separation.rgb.direct_encoded_components_v1` |
| Brightness → One Ink/Channel / `single-channel` | Legacy encoded Rec.709 luma darkness v1 | Legacy Current v1 | Preserve current model | Active Channel with a validated stable destination |
| Brightness → All Inks/Channels / `luminance` | Legacy encoded Rec.709 luma darkness v1 | Legacy Current v1 | Preserve current model | All Channels |
| Brightness → Crosshatch / `crosshatch-luminance` | Legacy encoded Rec.709 luma darkness v1 | Legacy Current v1 | Preserve serialized model | Exclusive Compatibility assignment `compat.crosshatch.progressive_kcmy_v1`; no active channel |

The complete formula, alpha, cache, project-version, and preset-container matrix
is normative in `ARTWORK_PIPELINE_AUDIT.md`.

### 22.2 Migration requirements

Migration must preserve:

- visible output;
- active channel where valid;
- channel state;
- pattern or treatment;
- presets;
- save/reopen behavior;
- legacy Crosshatch behavior.

For migrated v1-v5 documents, each legacy output-mode cache is converted once to
a complete semantic snapshot plus pattern/channel state. Selecting an output
mode may activate that owning snapshot and store the previous active snapshot,
preserving the historical behavior in which switching modes can also restore a
source/assignment combination. This versioned migration-only compatibility rule
is the sole exception to the general source/output independence invariant; new
documents and ordinary output-only edits must not create that coupling.

### 22.3 Legacy brightness audit

The audit must identify:

- formula;
- color encoding;
- alpha behavior;
- inversion;
- clamping;
- whether the value is sampled before or after resizing;
- whether Shapes and Curves use the same implementation;
- whether PNG and SVG share it.

### 22.4 No silent modernization

A legacy source may remain serialized as a compatibility variant until equivalence is demonstrated.

---

## 23. Concurrency, cancellation, and stale results

### 23.1 Generation identity

Every render request should have a generation identity.

A result may update the preview only when it matches the latest accepted generation.

### 23.2 Cooperative cancellation

Expensive stages must support cancellation:

- source-field generation;
- color separation;
- pattern generation;
- SVG geometry preparation;
- raster compositing;
- output treatment.

### 23.3 State safety

Cancelled work must not:

- alter saved source settings;
- advance seeds;
- replace canonical geometry;
- leave the UI permanently busy;
- damage export destinations.

### 23.4 GTK synchronization

The architecture must preserve the P0 stabilization rules established before TON-012:

- do not replace live dropdown models during selection notification;
- update model contents while preserving identity where appropriate;
- defer mode-changing synchronization until callbacks unwind;
- reject or clamp invalid selection positions;
- avoid nested incompatible `RefCell` borrows;
- do not interpret invalid positions as real channels.

---

## 24. Invariants

The following are mandatory.

### 24.1 Source and output

- Artwork Source does not implicitly determine Output Model.
- Output Model does not silently replace Artwork Source.
- Source-only presets do not change Output Model.
- Output-only presets do not change Artwork Source.
- The only temporary exception is activation of a migrated v1-v5 mode-cache
  semantic snapshot as defined in section 22.2; it must not be generalized to
  new documents or ordinary output-only edits.

### 24.2 Channels

- CMYK has exactly four ordinary output channels.
- RGB has exactly three ordinary output channels.
- RGB never retains Black as a hidden active channel.
- Stable channel IDs are used throughout model, render, persistence, and export.
- Migrated Crosshatch K/C/M/Y records are legacy compatibility layers outside
  ordinary output-channel membership. This temporary representation does not
  add Black or a fourth ordinary channel to RGB, and it cannot be an active RGB
  destination.

### 24.3 Routing

- Scalar sources require explicit routing.
- Full Color normally uses automatic separation.
- Invalid channel assignments cannot enter persisted document state.

### 24.4 Patterns

- Patterns consume resolved channel fields.
- Patterns do not perform document-wide color separation.
- Crosshatch is not an Artwork Source.
- Mixed marks and paths must be possible after TON-010 and TON-011.

### 24.5 Output treatment

- DTF is not an Output Model.
- DTF-derived elements are not ordinary pattern channels.
- TON-009 may be absent without blocking standard artwork.

### 24.6 Presentation and export

- Preview Surface is never sampled.
- Preview Surface is not exported.
- Export Background is explicit.
- Preview, PNG, and SVG use equivalent resolved data.

### 24.7 Persistence

- UI indexes are never authoritative serialized identities.
- Unknown values do not silently map to arbitrary defaults.
- Save/reopen restores semantic state, not merely visible labels.

---

## 25. Prohibited couplings

New code must not introduce or retain these dependencies without an explicit compatibility boundary:

```text
Artwork Source → automatically changes Output Model
Artwork Source → selects Pattern
Output Model → selects Artwork Source
Pattern → reads GTK controls
Pattern → performs global source separation
Crosshatch → represented as a source sampler
Preview Surface → participates in source sampling
Export Background → participates in source sampling
DTF → represented as an ordinary output channel
Dropdown index → serialized channel identity
Async completion order → determines render order
```

---

## 26. Error handling

Errors should identify the affected stage.

Examples:

- source artwork could not be decoded;
- unsupported source semantics version;
- invalid output model;
- invalid active channel;
- incompatible assignment;
- pattern unavailable;
- custom pattern asset missing;
- channel generation failed;
- export serialization failed;
- output treatment unavailable.

The application must not silently:

- choose another source;
- switch output models;
- select the first channel;
- substitute another pattern;
- drop a failed channel and export incomplete artwork.

---

## 27. Testing strategy

### 27.1 Source tests

Test:

- Full Color;
- Red;
- Green;
- Blue;
- Value;
- Perceptual Lightness;
- Alpha;
- Preserve Alpha;
- Ignore Alpha;
- partial transparency;
- internal transparent holes.

### 27.2 Output tests

Test:

- CMYK;
- RGB;
- mode round trips;
- mode-specific state restoration;
- invalid active-channel recovery.

### 27.3 Assignment tests

Test:

- Automatic Color Separation;
- Active Channel;
- All Channels;
- invalid assignment rejection;
- source/output combinations.

### 27.4 Pattern-boundary tests

Test that the same resolved field can drive:

- mark patterns;
- path patterns;
- custom motifs;
- deterministic generators;
- stochastic generators.

### 27.5 Export parity

For representative documents, compare:

- preview;
- PNG;
- SVG;
- save/reopen;
- migrated legacy document.

### 27.6 GTK realized tests

Exercise actual production callbacks for:

- source changes;
- alpha-policy changes;
- output-model changes;
- assignment changes;
- active-channel changes;
- rapid repeated switching;
- invalid transient positions;
- preset load;
- save/reopen.

### 27.7 Migration tests

Create fixed fixtures for every legacy mapping.

Verify visual and numerical parity where feasible.

### 27.8 Cancellation tests

Test cancellation during:

- source sampling;
- channel separation;
- pattern generation;
- compositing;
- export.

---

## 28. Staged adoption plan

### Stage 0 — Audit and architecture confirmation

Before implementation:

- locate every legacy mapping definition;
- identify actual formulas;
- map UI callbacks;
- map renderer branches;
- map persistence and preset schemas;
- map Crosshatch special cases;
- identify output-model inference;
- identify duplicated Shapes/Curves sampling logic;
- update this document with confirmed facts.

No broad refactor should begin before this audit.

### Stage 1 — Independent domain model

Implement independent types for:

- Artwork Source;
- Source Alpha Policy;
- Output Model;
- Channel Assignment;
- stable channel IDs;
- versioned separation-strategy IDs;
- an exclusive Crosshatch compatibility assignment.

Preserve legacy behavior through adapters.

**Compatibility requirement:** Stage 1 also includes the minimum project schema
bump and v1-v5 migration needed to serialize the new authoritative state. It may
not reconstruct that state from a second mutable legacy mapping after reopen.

### Stage 2 — Resolved channel-field pipeline

Centralize:

- source sampling;
- alpha handling;
- channel separation;
- scalar routing.

Make Shapes and Curves consume the same resolved channel fields where practical.

### Stage 3 — Preset migration and compatibility cleanup

Implement:

- the complete preset migration and scoped preset semantics;
- remaining compatibility cleanup after the resolved-field pipeline exists;
- stable-ID references and unknown-ID handling in the preset schema;
- cleanup of the Stage 1 legacy compatibility variants when behaviorally safe;
- preset round-trip tests.

### Stage 4 — UI refactor

Replace the combined Artwork Mapping control with separate controls.

Preserve GTK stabilization rules.

### Stage 5 — Preview and export parity

Verify:

- Shapes;
- Curves;
- CMYK;
- RGB;
- PNG;
- SVG;
- transparency;
- oversized raster and SVG sources with a long edge above 2400 pixels;
- mode round trips.

Treat correction of RGB-mode Crosshatch SVG from its current Screen blend to
Multiply as an intentional behavior change requiring explicit approval, not as
part of Stage 1 migration.

### Stage 6 — Resume TON-008 RGB Curves

Complete RGB Curves against the corrected source/output architecture.

### Stage 7 — Begin TON-010

Build the pattern registry against resolved channel fields.

### Stage 8 — Begin TON-011

Add Advanced Pattern Mixing using stable pattern instances and channel IDs.

### Optional later stage — TON-009 integration

Integrate only when TON-009 is complete.

---

## 29. Architecture decisions

### Accepted

1. Artwork Source and Output Model are independent.
2. Source Alpha Policy is independent from source selection.
3. Scalar routing is represented by Channel Assignment.
4. Patterns consume resolved channel fields.
5. Crosshatch is not a source sampler.
6. DTF is an optional output treatment.
7. Preview Surface is display-only.
8. Stable semantic identifiers replace UI indexes.
9. CMYK and RGB retain separate mode-specific state.
10. Standard workflow remains simpler than Advanced Pattern Mixing.
11. Legacy Brightness is encoded Rec.709-weighted luma darkness, identified as
    `source.legacy_brightness.encoded_rec709_inverted_v1`.
12. The initial compatibility CMYK separation is
    `separation.cmyk.encoded_rgb_max_black_v1`.
13. Direct encoded RGB automatic mapping is
    `separation.rgb.direct_encoded_components_v1`.
14. Migrated path-dependent alpha behavior uses the explicit
    `source_alpha.legacy_current_v1` compatibility policy.
15. Crosshatch uses `compat.crosshatch.progressive_kcmy_v1` until TON-010 owns a
    general pattern representation.
16. Minimal document persistence moves into Stage 1 so the new state can be
    authoritative across save/reopen.
17. Migrated mode caches restore complete semantic snapshots as a versioned
    compatibility exception; new output edits do not replace source state.

### Provisional pending audit

1. Whether Perceptual Lightness uses OKLab `L` immediately or after a compatibility period.
2. Exact Alpha-source coverage rule for new work.
3. Whether Ignore Alpha samples stored hidden RGB or uses a matte.
4. Whether source selection remains document-wide in the first TON-011 release.
5. Exact persistence schema layout beyond the minimum Stage 1 document state.
6. Full preset scoping and whether a workflow preset may intentionally include Output Model.

Any provisional decision resolved during implementation should update this section.

---

## 30. Examples

### 30.1 Default CMYK document

```text
Artwork Source: Full Color
Source Alpha: Preserve
Output Model: CMYK Print
Assignment: Automatic Color Separation
Pattern: Rectangular Dot Grid
```

### 30.2 Default RGB document

```text
Artwork Source: Full Color
Source Alpha: Preserve
Output Model: RGB Screen
Assignment: Automatic Color Separation
Pattern: Rectangular Dot Grid
```

### 30.3 Monochrome black halftone

```text
Artwork Source: Perceptual Lightness
Source Alpha: Preserve
Output Model: CMYK Print
Assignment: Active Channel → Black
Pattern: Wave Line Field
```

### 30.4 Same tonal source across CMYK channels

```text
Artwork Source: Value
Source Alpha: Preserve
Output Model: CMYK Print
Assignment: All Channels
Pattern: Pointillism
```

### 30.5 Alpha-driven RGB silhouette

```text
Artwork Source: Alpha
Output Model: RGB Screen
Assignment: Active Channel → Red
Pattern: Custom SVG Motif
```

### 30.6 Advanced mixed patterns

```text
Document source defaults:
  Artwork Source: Full Color
  Output Model: CMYK Print

Advanced Pattern Mixing:
  Cyan → Triangular Dot Grid
  Magenta → Wave Line Field
  Yellow → Pointillism
  Black → Custom SVG Pattern
```

TON-011 owns the per-channel pattern assignment. TON-012 supplies the source and output-channel data.

---

## 31. Review checklist

Before accepting an implementation stage, reviewers should ask:

### Model

- Are source, alpha, output, assignment, pattern, and treatment separate?
- Are stable identifiers used?
- Are invalid states unrepresentable or rejected?

### Rendering

- Is source sampling centralized?
- Do patterns consume resolved fields?
- Are preview and exports equivalent?
- Are CMYK and RGB channel orders deterministic?

### UI

- Are controls labeled by responsibility?
- Are irrelevant controls hidden?
- Can transient invalid GTK positions reach the model?
- Does switching output model preserve appropriate state?

### Persistence

- Are independent concepts serialized independently?
- Do legacy documents preserve output?
- Are unknown identifiers handled visibly?

### Extensibility

- Can TON-010 add patterns without changing source sampling?
- Can TON-011 assign patterns per channel without duplicating output-model logic?
- Can TON-009 consume resolved artwork without regenerating channels?

---

## 32. Definition of architectural readiness

The artwork pipeline is ready for TON-010 and TON-011 when:

- Artwork Source is independent from Output Model;
- Source Alpha behavior is explicit;
- Channel Assignment is explicit;
- CMYK and RGB use stable semantic channels;
- Shapes and Curves consume consistent resolved channel fields;
- legacy mappings migrate safely;
- preview, PNG, and SVG agree;
- GTK transitions remain stable;
- persistence restores semantic state;
- Crosshatch no longer defines source mapping;
- pattern generators do not own color separation;
- optional DTF integration remains post-compositing.

Until these conditions are met, new pattern and per-channel architecture should avoid depending on the legacy combined Artwork Mapping model.
