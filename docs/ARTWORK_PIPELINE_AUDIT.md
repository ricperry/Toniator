# Toniator Artwork Pipeline Audit

**Audit date:** 2026-07-21

**Audited commit:** `08459db` on `debug/ton-008-menu-crashes`

**Scope:** TON-012 Stage 0 only

**Architecture baseline:** `docs/ARTWORK_PIPELINE.md` (untracked at the audited starting state)

> **Historical note — Stage 1A decision (2026-07-21):** this Stage 0 audit
> recorded `source.legacy_brightness.encoded_rec709_luma_darkness_v1` while
> naming the discovered formula. Stage 1A intentionally chose the canonical,
> unshipped compatibility identity
> `source.legacy_brightness.encoded_rec709_inverted_v1` instead. The old text
> below remains evidence of the audit; the new parser does not accept both
> spellings. No live source formula or legacy document behavior changed.

## 1. Status and evidence rules

This document records current behavior, not behavior inferred from labels such as
“Brightness,” “Color,” or “RGB.” Code references use line numbers from commit
`08459db` before this audit's documentation-only edits.

The dispositions used below are:

- **Preserve:** keep the established output during migration.
- **Migrate:** represent the same behavior through the new authoritative model.
- **Replace:** remove an indirect or competing source of truth after its compatibility adapter exists.
- **Defer:** decide or implement in a later stage or issue.

No application code or behavior changed during this audit.

## 2. Confirmed current pipeline

```text
GTK controls
  Pattern Type + Artwork Mapping + Output + target/visibility/appearance
        |
        v
DocumentEditor
  Document.output_mode
  Document.render = Shapes settings OR Curves settings
  saved Shapes/Curves + inactive CMYK/RGB treatment caches
        |
        +---------------------------+
        |                           |
        v                           v
Shapes generator                 Curves generator
sample_web_image                sample_web_image (once per enabled layer)
map_web_pixel                   map_web_pixel + cubic interpolation
MarkSet                         CurveGeometry
        |                           |
        +-------------+-------------+
                      |
          +-----------+-----------+
          |           |           |
          v           v           v
       Preview       PNG          SVG
       rasterizes    regenerates  regenerates Marks/CurveGeometry
       canonical     then         and serializes editable layers
       form          rasterizes
          |           |           |
Preview Surface +     +----- Export Background -----+
Export Background
shown in preview
```

There is no current `ArtworkSource`, `SourceAlphaPolicy`,
`ChannelAssignment`, `ResolvedChannelField`, or stable semantic pattern ID.
The proposed pipeline in `docs/ARTWORK_PIPELINE.md` is therefore a target, not a
description of the present implementation.

## 3. Model and state inventory

| File and symbol | Lines | Confirmed current behavior | Baseline agreement | Disposition |
|---|---:|---|---|---|
| `src/model.rs` `OutputMode` | 9-25 | Authoritative document output choice: `CmykInks` exposes C/M/Y/K and `RgbScreen` exposes R/G/B. | Agrees with two explicit output models, but current serialized IDs are enum-derived `cmyk-inks` and `rgb-screen`, not the proposed dotted IDs. | Migrate. |
| `src/model.rs` `ValueMode` and `ValueModeOutputMode` | 138-170 | One combined enum represents source formula, scalar routing, Crosshatch, and explicit output-mode selection. `Cmyk` and `Rgb` are classified as output-changing; the other three are neutral. | Conflicts with independent source, output, assignment, and pattern state. | Replace behind a compatibility facade. |
| `src/model.rs` `Ink` | 172-212 | One enum contains seven CMYK/RGB destinations; short IDs are `c/m/y/k/r/g/b`. | Semantic variants are sound; IDs do not yet match the target stable namespace. | Migrate, preserving channel meaning. |
| `src/model.rs` `WebShapeChannel(s)` | 283-399 | Seven stored channel records each contain visibility, color, opacity, geometry, transform, threshold, and sampling-density state. Renderers use the channels chosen indirectly by output/mapping. | Per-channel state agrees; output membership is indirect. | Preserve values; route by authoritative output model. |
| `src/model.rs` `WebShapeSettings` | 436-532 | Owns `value_mode`, `single_channel`, `crosshatch_color`, shared/independent mark state, `base_channel`, and effective channels. `base_channel` is the All-target adjustment anchor; renderers consume `channels`. | Contains several target concepts in one pattern-specific object. | Migrate mapping/routing out; preserve pattern and channel values. |
| `src/model.rs` `WebCurveChannel(s)` | 632-759 | Seven stored channel records contain visibility, color, opacity, path, transform, threshold, resolution, and motif state. | Same partial agreement as Shapes. | Preserve values; route by authoritative output model. |
| `src/model.rs` `WebCurveSettings` | 761-878 | Duplicates `value_mode`, `single_channel`, `crosshatch_color`, dimensions, sampling controls, `base_channel`, and channel state; also owns layout and path configuration. | Conflicts with one source/routing model and one resolved-field boundary. | Migrate mapping/routing out; preserve curve behavior. |
| `src/model.rs` `RenderVariant` | 880-891 | `WebShapeV1` versus `WebCurveV1` is a document-wide mutually exclusive render choice. | Accepted only as a pre-TON-010 compatibility boundary. | Preserve through TON-012; defer pattern registry changes. |
| `src/model.rs` `DocumentAppearance` | 55-91 | Authoritative `PreviewSurface` and `ExportBackground` are distinct, alpha-capable document fields. | Agrees. | Preserve. |
| `src/model.rs` `Document` | 922-944 | Authoritative active state includes source bytes, appearance, output mode, active render, saved Shapes/Curves, and inactive CMYK/RGB caches. | Output/appearance ownership agrees; source semantics and assignment are absent. | Extend in Stage 1 with one authoritative compatibility model. |
| `src/model.rs` `OutputTreatmentCache` and `switch_output_mode` | 909-920, 966-1037 | Switching output caches and restores the complete inactive treatment. A first RGB switch derives an RGB treatment and changes the untouched white preview surface to black. | Mode-specific restoration agrees. Appearance mutation is a current compatibility behavior that must be tested explicitly. | Preserve output and appearance behavior during migration. |
| `src/model.rs` `TreatmentState` | 1577-1586 | Undo/redo snapshots output mode, render, appearance, saved variants, and both inactive caches together. | Agrees with atomic document edits. | Preserve. |
| `src/ui.rs` `AppState::syncing_controls` and `AppUi` widgets | 1503-1620, 5296-5318 | GTK widgets are synchronized from a cloned document snapshot under a recursion guard; widgets are not authoritative. | Agrees with document-owned state and P0 rules. | Preserve. |

### 3.1 Authoritative versus derived state

- `Document.output_mode` is authoritative for the selected output model.
- The active render's `settings.value_mode` is authoritative for the current
  combined Artwork Mapping behavior.
- The active render's `settings.single_channel` is authoritative for one-channel
  routing, but its `Ink` can belong to the other output model. The renderer then
  aliases RGB and CMYK through the same four scalar slots (`src/render.rs:808-817`).
- Per-channel `enabled`, `color`, and `opacity` fields are authoritative for the
  active render. Output membership is derived from `output_mode` and
  `value_mode`.
- `base_channel` is authoritative only as the numeric anchor shown for the
  All-target UI. All-target edits calculate a delta from it and mutate effective
  channels; renderers never consume it (`src/ui.rs:3294-3368`).
- GTK dropdown indexes and labels are derived views, except that callbacks map
  indexes back to enums. They are not serialized directly.
- Preview and export geometry are derived from the document each time.

### 3.2 Multiple mutable representations of related semantics

The following are not all byte-for-byte duplicates, but they are independently
mutable representations of the same conceptual state and therefore migration
hazards:

1. `Document.output_mode` and explicit `ValueMode::Cmyk`/`ValueMode::Rgb` both
   express output-model intent.
2. Shapes and Curves independently store `value_mode`, `single_channel`,
   `crosshatch_color`, global sampling values, a base channel, and seven effective
   channels.
3. Active render, `saved_web_shape`, `saved_web_curve`, `inactive_cmyk`, and
   `inactive_rgb` can each contain separate mapping and active-channel values.
4. `base_channel` and effective per-channel values jointly encode All-target
   editing state; only the latter render.
5. The output model and the active channel are semantic enums, but UI control
   positions also select them. CMYK and RGB destinations share numeric scalar
   slots in `map_web_pixel`.

## 4. Exact source sampling and formulas

### 4.1 Encoding and resize assumptions

Source preparation and field sampling form a two-stage resize path. First,
`decode_source(..., 2400)` caps the decoded image's long edge at 2400 pixels
(`src/render.rs:173-189`, `495-518`). Oversized raster input is decoded to
straight-alpha 8-bit RGBA and resized with Lanczos3. SVG input is parsed by
`usvg` and rendered by `resvg` directly at the capped scale
(`src/render.rs:212-226`). `tiny_skia::Pixmap` owns premultiplied RGBA bytes, but
`decode_svg` wraps `Pixmap::take()` directly as an `RgbaImage` without
unpremultiplying. Neither path performs color-profile conversion. All formulas
operate on the resulting encoded byte components, not linear-light RGB; no
current formula performs sRGB electro-optical decoding.

Second, Web Shapes and Curves call `sample_web_image_cancellable` on that
prepared image (`src/render.rs:681-730`). It:

1. assumes straight-alpha input, converts encoded RGB bytes to `[0,1]`, and
   premultiplies each component by alpha;
2. downsizes with triangle filtering;
3. returns `[0,0,0,0]` when filtered alpha is effectively zero;
4. otherwise unpremultiplies RGB, rounds RGB and alpha back to bytes, and clamps
   RGB to `[0,255]`.

For decoded raster input, this is one balanced
premultiply/filter/unpremultiply sequence. For SVG, the incoming RGB is already
premultiplied, so this step premultiplies it again and the later step
unpremultiplies only once. Translucent SVG RGB therefore remains alpha-attenuated
before the source formula. This accidental source-format distinction is part of
`source_alpha.legacy_current_v1`, not the target Preserve behavior.

Migration fixtures must include oversized raster and SVG sources whose long edge
exceeds 2400 pixels. They must lock both the first-stage Lanczos3/scaled-resvg
result and the later Triangle-filtered grid fields; testing only sources below
the cap would not preserve the complete sampling contract.

### 4.2 CMYK color separation

`rgb_to_cmyk` and the web CMYK branch use the same encoded-RGB formula
(`src/render.rs:23-43`, `741-755`). For normalized encoded components
`r`, `g`, `b`:

```text
k = 1 - max(r, g, b)

if k >= 0.999:
    (c, m, y, k) = (0, 0, 0, 1)
else:
    c = clamp((1 - r - k) / (1 - k), 0, 1)
    m = clamp((1 - g - k) / (1 - k), 0, 1)
    y = clamp((1 - b - k) / (1 - k), 0, 1)
    k = clamp(k, 0, 1)
```

This is a simple device-independent encoded-RGB conversion, not ICC-managed
print separation. Preserve it as the legacy CMYK formula.

### 4.3 RGB color mapping

For `ValueMode::Rgb` (`src/render.rs:757-764`):

```text
a = alpha / 255
R_field = (R / 255) * a
G_field = (G / 255) * a
B_field = (B / 255) * a
fourth scalar slot = 0
```

The encoded RGB components are not inverted. Fully transparent pixels return
zero before this branch. For straight-alpha raster input, partial alpha is
applied once to field intensity. For SVG, `R`, `G`, and `B` reaching this formula
are already attenuated by the unbalanced premultiplication above, and this branch
multiplies by alpha again.

### 4.4 Legacy “Brightness”

Every current web Brightness mapping uses encoded Rec.709-weighted luma
coefficients and then inverts the result (`src/render.rs:766-767`):

```text
encoded_luma = (0.2126 * R + 0.7152 * G + 0.0722 * B) / 255
legacy_brightness = darkness = 1 - encoded_luma
```

The range is `[0,1]`: encoded white is `0`, encoded black is `1`. It is not HSV
Value, HSL Lightness, linear-light relative luminance, arithmetic mean, or the
maximum component. The present enum name `Luminance` is therefore misleading.
The stable compatibility ID is confirmed as:

```text
source.legacy_brightness.encoded_rec709_luma_darkness_v1
```

Shapes and Curves share this numerical function, but their sampling frequency,
interpolation, and partial-alpha handling differ.

### 4.5 Scalar routing formulas

- `Luminance`: returns `[darkness; 4]`.
- `SingleChannel`: writes `darkness` into the scalar slot selected by
  `single_channel`, leaving the other slots zero.
- `CrosshatchLuminance`: orders enabled layers K, C, M, Y; with `n` enabled
  layers and `span = 1/n`, layer `i` receives
  `clamp(darkness - i*span, 0, span)`, snapped only at numerical endpoints
  (`src/render.rs:776-805`). It is not divided by `span`. Visibility therefore
  changes the progressive partition and the remaining layers' geometry; it is
  not composition-only in Crosshatch.

After source mapping, each channel's threshold applies
(`src/render.rs:820-827`):

```text
value < threshold      -> 0
threshold >= 0.999     -> 1 only when value >= threshold
otherwise              -> (value - threshold) / (1 - threshold)
```

Shapes use the thresholded value directly to size marks
(`src/render.rs:609-626`). Curves cubically interpolate mapped grid values,
clamp the interpolation to `[0,1]`, then apply the same threshold before
calculating path width (`src/curve_render.rs:900-973`).

### 4.6 Exact alpha behavior

There is no user-selectable or persisted alpha policy. For ordinary decoded
straight-alpha raster input, current behavior is this path-dependent
compatibility rule:

| Path | Fully transparent RGB | Partial source alpha | More than one final alpha multiplication? |
|---|---|---|---|
| Native Basic CMYK | skipped | CMYK intensity multiplied once by alpha after contrast | No |
| Native Basic RGB | zero contribution | encoded component multiplied once by alpha | No |
| Web Shapes, direct RGB | zero contribution | component field multiplied once by alpha | No |
| Web Shapes, Brightness in RGB output | zero contribution | darkness multiplied once by alpha in the Shapes generator | No |
| Web Shapes, CMYK or Brightness in CMYK output | zero contribution | nonzero alpha is treated as fully present | No proportional application |
| Web Curves, direct RGB | zero contribution | component field multiplied once by alpha | No |
| Web Curves, Brightness in RGB or CMYK output | zero contribution | nonzero alpha is treated as fully present | No proportional application |
| Crosshatch | zero contribution | nonzero alpha is treated as fully present | No proportional application |

Evidence: `src/render.rs:335-459`, `681-795`; Shapes-only RGB Brightness
adjustment at `src/render.rs:598-608`; Curves call `map_web_pixel` without that
adjustment at `src/curve_render.rs:976-1003`.

Consequences:

- fully transparent pixels never expose stored hidden RGB;
- `Ignore Source Alpha` does not exist;
- the target default `Preserve Source Alpha` is not equivalent to every legacy
  path;
- Stage 1 needs one explicit compatibility alpha-policy value rather than
  silently modernizing old output;
- the table above is not universal for SVG sources.

For SVG source artwork, `resvg` supplies premultiplied RGBA and Toniator does not
convert it back to straight alpha (`src/render.rs:212-226`). Native Basic CMYK
therefore separates premultiplied color bytes and then applies alpha to mark
intensity; Native Basic RGB applies alpha to already-premultiplied components.
Web sampling multiplies those premultiplied components by alpha again, filters,
and unpremultiplies once. Direct RGB then performs its normal field-alpha
multiplication, while Brightness and CMYK formulas receive the already
alpha-attenuated sampled RGB. The exact filtered web component is conceptually
`filter(straight_rgb * alpha^2) / filter(alpha)` before later mapping. Thus
translucent SVG can have effective repeated alpha attenuation even though the
same mapping branch has only one explicit final multiplication. Add translucent
SVG fixtures for Native Basic, Shapes, and Curves across direct RGB, CMYK,
Brightness, and Crosshatch before altering this compatibility path.

## 5. Mapping behavior matrix

| Current GTK value / serialized `ValueMode` | Source/formula | Output model effect | Routing and channel count | Shapes/Curves treatment | Visibility/compositing | Persistence and UI synchronization | Migration |
|---|---|---|---|---|---|---|---|
| Color → CMYK Inks / `cmyk` | Full encoded RGB; legacy CMYK formula | `set_render_variant` forces `CmykInks` | Automatic C/M/Y/K; four candidate channels | Leaves current Shapes or Curves form | Enabled C/M/Y/K; Multiply raster/SVG | Index 0; serialized inside active/saved/cached render settings; preset load also infers CMYK | Full Color + legacy CMYK separation + CMYK Print + Automatic |
| RGB Color → Screen / `rgb` | Full encoded RGB; direct alpha-weighted components | Forces `RgbScreen` | Automatic R/G/B; three candidate channels | Leaves current Shapes or Curves form | Enabled R/G/B; Screen raster/SVG | Index 4; serialized by document/native render. Raw legacy preset parser does not accept `rgb`, but `nativeRender`/v2 serde does | Full Color + RGB Screen + Automatic |
| Brightness → One Ink/Channel / `single-channel` | Encoded Rec.709 luma darkness | Preserves current model | One scalar slot selected by `single_channel`; active destination is pattern-specific state | Leaves form unchanged | Only the routed output can receive geometry; normal channel visibility still filters it | Index 1; label changes with output; serialized in render settings | Legacy Brightness + legacy alpha policy + preserved model + Active Channel |
| Brightness → All Inks/Channels / `luminance` | Same luma darkness | Preserves current model | Replicates four scalar slots; renderer consumes C/M/Y/K or R/G/B | Leaves form unchanged | Every enabled output channel receives the same scalar before channel thresholds | Index 2; label changes with output; serialized in render settings | Legacy Brightness + legacy alpha policy + preserved model + All Channels |
| Brightness → Crosshatch / `crosshatch-luminance` | Same darkness, progressively partitioned across enabled K/C/M/Y | Preserves `Document.output_mode`, even RGB | Always K/C/M/Y layer records in K,C,M,Y order; enabled count changes partition | Selecting from Shapes converts to Curves; selecting in Curves calls `configure_crosshatch` | Current code uses four monochrome CMYK `Channel` records. Migration classifies them as legacy compatibility layers, outside ordinary RGB membership; raster uses Multiply by identity while SVG uses document output mode, creating an RGB-mode parity risk | Index 3; serialized in curve settings; legacy shape form normalizes to Curves on load/preset import | Legacy Brightness + legacy alpha policy + explicit Crosshatch compatibility state; preserve stored output model but render as legacy K/C/M/Y compatibility layers |

### 5.1 Active-channel aliasing risk

`single_channel` stores an `Ink`, but `ink_index` aliases C/R to slot 0, M/G to
slot 1, Y/B to slot 2, and K to slot 3 (`src/render.rs:808-817`). Mapping-driven
output switches can commit the pre-switch render after the mode cache is swapped
(`src/model.rs:1702-1720`). Thus a stored destination from the other model can
survive until the Single Channel mapping is selected. GTK later maps enum
variants back to positions (`src/ui.rs:5415-5424`, `5781-5790`). Stage 1 must
validate or deliberately translate the active stable channel whenever output
model changes; scalar-slot aliasing must stop being semantic routing.

## 6. Output-model selection and inference points

The following current locations select or infer CMYK/RGB and must stop doing so
indirectly after the Stage 1 compatibility adapter:

| File and symbol | Lines | Current inference | Required disposition |
|---|---:|---|---|
| `src/model.rs` `ValueMode::output_mode_classification` | 150-169 | `Cmyk` means explicit CMYK; `Rgb` means explicit RGB. | Replace with facade translation at the deliberate legacy user/preset action. |
| `src/model.rs` `DocumentEditor::set_render_variant` | 1702-1720 | Any committed render variant may switch output from its `value_mode`. | Stop renderer/model mutation from inferring output. |
| `src/model.rs` `Document::new_rgb_treatment` | 982-1006 | A first RGB switch derives `ValueMode::Rgb`, Red active channel, and enabled RGB channels. | Keep as explicit output-mode initialization, but populate new authoritative fields directly. |
| `src/render.rs` Shapes output selection | 539-551 | Crosshatch forces C/M/Y/K; otherwise output is RGB when output mode is RGB **or** mapping is `Rgb`. | Route only from authoritative output model plus explicit compatibility Crosshatch handling. |
| `src/curve_render.rs` Curves output selection | 62-74 | Same inference as Shapes, with K/C/M/Y Crosshatch order. | Same. |
| `src/render.rs` Native Basic branch | 506-512 | Chooses RGB/CMYK render function from `Document.output_mode`. | Preserve; later use new `OutputModel` name. |
| `src/render.rs` raster blend | 1360-1372 | Blend mode is inferred from each `Channel` variant. | Preserve as resolved channel appearance, not a source inference. |
| `src/svg_export.rs` SVG blend and descriptions | 51-87, 133-186 | Blend mode and description are inferred from `Document.output_mode`, even for Crosshatch K/C/M/Y geometry. | Consume resolved channel appearance; fix only in later implementation. |
| `src/ui.rs` Output dropdown callback | 2435-2458 | Position 0/1 explicitly calls `set_output_mode`. | Preserve as an explicit user action; later map stable IDs. |
| `src/ui.rs` mapping callbacks | 2467-2482, 2717-2734 | Dropdown index becomes `ValueMode`; editor classification may switch output. | Keep temporary facade, translating once into authoritative fields. |
| `src/persistence.rs` document migration | 60-126 | Versions 1-4 are forced to CMYK; v5 reads explicit output mode. | Preserve migration behavior and add the next schema adapter. |
| `src/preset.rs` parser + UI application | 22-117; `src/ui.rs:4203-4247` | Preset provides render settings only; `set_treatment`/`set_render_variant` infers output for `cmyk` and `rgb`, neutral mappings preserve current output. | Make preset migration return explicit compatibility intent; do not infer in generic render setters. |

No output model is inferred from channel count alone in persistence, but both
generators choose their channel list from a combination of output mode and
mapping. Dropdown positions are UI inputs, not serialized data. Cached treatment
contents are restored because the explicit output mode selects a cache; they do
not independently select the mode.

## 7. Channel assignment and shared state

### 7.1 Current assignment rules

- Automatic CMYK separation: `ValueMode::Cmyk` computes four fields.
- Automatic RGB mapping: `ValueMode::Rgb` computes three alpha-weighted fields.
- One-channel brightness: `ValueMode::SingleChannel` writes one of four shared
  scalar slots based on `single_channel`.
- All-channel brightness: `ValueMode::Luminance` writes all four slots; the
  renderer's chosen output channels determine whether four CMYK or three RGB
  channels consume them.
- Active output channel: stored separately inside Shapes and Curves settings,
  separately again in inactive-mode and saved-treatment caches.
- All Channels: not persisted as an explicit assignment; it is implied by
  `ValueMode::Luminance` and by target-dropdown position 0 for editing.
- Channel visibility: `channels.<ink>.enabled`; renderers omit disabled layers.
- Shared versus independent geometry: `use_shared_mark`/`use_shared_curve`
  choose shared geometry, while numeric `base_channel` updates are propagated as
  deltas to effective per-channel records.

### 7.2 Incorrect scalar assumptions to remove

Current code assumes, depending on `ValueMode`, that a scalar means exactly one
slot, all four slots, or progressive Crosshatch. The output generator then
assumes those slots mean CMYK or RGB from other state. Stage 1 must make
assignment explicit and Stage 2 must resolve stable destination fields before a
pattern sees them.

## 8. Crosshatch audit

Crosshatch is represented by all of the following current facts:

- `ValueMode::CrosshatchLuminance` in Shapes or Curves settings;
- `crosshatch_color` in both settings types;
- a Curves render variant in normalized/current UI use;
- four ordinary channel records K, C, M, Y;
- the K/C/M/Y enabled states and their channel parameters;
- UI relabeling of those channels as directional layers.

`WebCurveSettings::configure_crosshatch` forces Full Width Curves, one shared
straight path, open/unsmoothed ends, enables all four layers, and sets directions
K `45°`, C `-45°`, M `0°`, Y `90°` (`src/model.rs:826-849`). Selecting
Crosshatch while Shapes is active converts to Curves and saves a Shapes copy with
`Luminance` instead (`src/model.rs:1765-1778`; `src/ui.rs:4910-4922`). Loading a
legacy shape-based Crosshatch also normalizes it to Curves
(`src/model.rs:1074-1085`, `1354-1365`; `src/persistence.rs:128-145`).

Current code stores the layers as ordinary CMYK `Ink`/`Channel` values, but they
are semantically a monochrome progressive structure: all use
`crosshatch_color`, while each retains its own opacity, threshold, resolution,
transforms, and visibility. In the migrated authoritative model they are legacy
compatibility layers outside ordinary output-channel membership, especially when
the preserved output model is RGB. They are not RGB channels and cannot become a
hidden RGB active channel.

Preview and PNG generate Curves geometry and rasterize it. SVG regenerates the
same Curve outlines and labels them `Layer 1 (K)` through `Layer 4 (Y)`
(`src/svg_export.rs:106-203`). Project documents serialize the complete Curve
settings. Presets serialize a `nativeRender` plus web-compatible Curve fields;
legacy shape Crosshatch presets are normalized on import.

### 8.1 Smallest TON-012 compatibility representation

Use one authoritative, explicit compatibility state, for example:

```rust
enum ArtworkCompatibility {
    LegacyProgressiveCrosshatchV1,
}
```

`LegacyProgressiveCrosshatchV1` must carry or address the existing four layer
settings and shared monochrome color, select the legacy encoded-luma-darkness
source, use the legacy path-dependent alpha policy, preserve the progressive
K/C/M/Y partition, and require the current Curves compatibility generator. It
must not be an `ArtworkSource` or an `OutputModel`. TON-010 owns replacing this
adapter with a general pattern implementation.

The legacy document's `output_mode` must still round-trip. In Stage 1,
compatibility rendering must preserve today's path-dependent blend behavior:
raster Crosshatch uses Multiply from K/C/M/Y channel identity, while an
RGB-mode SVG uses Screen from the serialized document output mode. Add an
RGB-mode Crosshatch preview/PNG/SVG fixture before migration. Correcting SVG to
Multiply is an intentional visible behavior change deferred to the parity stage
and requires explicit approval; it is not part of the Stage 1 model extraction.

## 9. GTK controls and callback map

All listed callbacks first reject changes while `syncing_controls` is true. A
successful document edit clears rendered preview state, queues recovery,
synchronizes controls, requests a render, and updates actions
(`src/ui.rs:4994-5019`).

| Control | Widget -> callback -> mutation | Synchronization/render/persistence | Risks and constraints |
|---|---|---|---|
| Pattern Type | Shapes/Curves toggle buttons (`src/ui.rs:8378-8404`) -> `activate_shape_treatment` / `activate_curve_treatment` (`2347-2361`, `4891-4940`) -> restore saved settings through `set_render_variant` | `after_treatment_edit`; active/saved render and mode caches serialize | Restored `value_mode` can infer output. Preserve one undoable edit and no live-model replacement. |
| Shapes Artwork Mapping | `web_value_mode` -> selected-notify (`2467-2482`) -> index table; Crosshatch calls shape-to-curve conversion, others clone settings and call `set_render_variant` | Full sync + preview + recovery | Index is UI coupling; explicit mappings change output; Crosshatch changes treatment. Invalid position is ignored. |
| Curves Artwork Mapping | `curve_value_mode` -> selected-notify (`2717-2734`) -> index table; Crosshatch reconfigures Curves, others change value mode | Full sync + preview + recovery | Same index/output coupling. Invalid position is ignored. |
| Output | `output_mode` dropdown (`8847-8849`) -> callback (`2435-2458`) -> `DocumentEditor::set_output_mode` | Preview requested immediately; dropdown-list synchronization deferred with idle callback | Position other than 1 currently means CMYK. Preserve deferred synchronization and validate positions in new code. |
| Active output channel | Shapes/Curves Output Channel dropdown -> callbacks (`2484-2499`, `2736-2751`) -> `single_channel` | Full sync + preview + recovery | Position maps through current output model; invalid position ignored. Stored cross-model `Ink` can alias a scalar slot. |
| All Channels / active edit target | Shapes/Curves target dropdown -> selected-notify (`2460-2465`, `2768-2773`) -> no document mutation | Idle control resync only; position 0 selects every current output channel/layer for subsequent edits | Transient invalid positions must return `None`, never All Channels. |
| Channel visibility | four Shapes/Curves check buttons -> callbacks (`2642-2665`, `2836-2859`) -> `channels[resolved slot].enabled` | Full sync + preview + recovery | Fourth RGB control is hidden. Crosshatch visibility also changes its source partition, not just compositing. |
| Preview Surface | dropdown + alpha color button -> callbacks (`2385-2423`) -> `Document.appearance.preview_surface` | Full sync + preview + recovery; persisted in project only | Scope the `RefCell` borrow before `after_treatment_edit`; never sample or export this state. |
| Export Background | dropdown + alpha color button -> callbacks (`2400-2413`, `2425-2433`) -> `Document.appearance.export_background` | Full sync + preview + recovery; saved project, SVG, and default PNG | Same borrow rule; it is shown in preview but not sampled. |
| Load Preset | buttons/dialog -> `import_preset_source` (`4040-4090`, `4185-4289`) -> worker parses candidate -> `DocumentEditor::set_treatment` | Validates before mutation; latest-generation gate; then sync, preview, recovery | Generic setter currently infers output from mapping. Live document identity is checked before applying. |
| Save Preset | button/dialog (`4093-4149`) -> `document_treatment_preset_bytes` -> atomic write | No document mutation or preview request | Preset omits document appearance, source, caches, and explicit output mode. |
| Open project/artwork | file/drag path -> async `load_candidate_async` (`4151-4178`, `4292-4391`) -> `install_document_migrated` replaces `DocumentEditor` (`4421-4450`) | Dirty-transition gate, candidate generation gate, full sync, preview, optional recovery | Live-model replacement is intentional only after validation and dirty-state gating. |
| Save project | action -> `save_then` / `save_to_path` (`6864-6955`) -> atomic serialized `Document` | Marks editor clean and reconciles recovery only after successful write | Preserve atomicity and complete semantic state. |

### 9.1 P0 stabilization contract to preserve

The implementation at `08459db` already:

- updates an installed `gtk::StringList` with `splice` instead of replacing the
  live model (`src/ui.rs:9635-9665`);
- clamps a previous valid selection when a list shrinks;
- returns `None` for invalid list positions instead of treating them as All
  Channels (`src/ui.rs:9612-9633`);
- uses `syncing_controls` to block callback recursion;
- defers output-mode list changes until the active notification unwinds
  (`src/ui.rs:5002-5019`);
- scopes `RefCell` borrows before callbacks that re-enter state
  (`src/ui.rs:5022-5042`);
- validates candidates before replacing the live editor.

Stage 1 and Stage 4 must preserve these rules exactly.

## 10. Shapes and Curves comparison

| Concern | Shapes | Curves | Refactor boundary |
|---|---|---|---|
| Sampling | Caches one sampled grid per distinct `(cols, rows)` during a generation | Calls the same sampler separately for every enabled layer, even identical grids | Stage 2 resolver owns sampled fields/cache. |
| Mapping formula | Calls `map_web_pixel` per placed cell | Calls `map_web_pixel` through `raw_value`, then cubic interpolation | Resolver produces channel fields; Curves may interpolate the resolved field if that preserves output. |
| Alpha | Adds proportional alpha for RGB-output Brightness | Does not add that adjustment | Legacy alpha compatibility must be explicit before unifying. |
| Routing/output inference | `value_mode` + output mode choose C/M/Y/K or R/G/B | Same, with K/C/M/Y Crosshatch order | Remove from both generators. |
| Geometry | `MarkSet` with resolved shape per mark | `CurveGeometry` with outlines per layer | Keep separate pattern adapters through TON-012. |
| Compositing input | `InkLayer` per chosen channel | `CurveInkLayer` wraps the same `InkLayer` appearance | A shared resolved channel/appearance structure can feed both. |
| Persistence | `WebShapeSettings` active/saved/cached | `WebCurveSettings` active/saved/cached | Stage 1 adapters read legacy settings into one document-level semantic state; preserve pattern settings. |

The exact Stage 2 boundary is between `sample_web_image`/`map_web_pixel` and
`generate_web_shape_marks_for_output_mode`/`generate_curve_geometry_for_output_mode`.
Both generators should later receive resolved stable channel fields plus channel
appearance. Stage 0 does not implement that boundary.

## 11. Preview, PNG, and SVG map

| Path | Source and geometry | Output/alpha/compositing | Appearance | Parity status |
|---|---|---|---|---|
| Live preview | Clones and normalizes document; decodes source; generates `MarkSet` or `CurveGeometry` (`src/render.rs:1025-1097`) | Raster blend derives from channel identity | Non-default path renders transparent, then composites Preview Surface and Export Background. Legacy-white fast path rasterizes directly on white. | Same generation functions, but separate invocation and a legacy-white antialias path. |
| PNG | `png_bytes_cancellable` calls document output/export renderer, regenerating the same canonical form (`src/png_export.rs:57-104`) | Same raster renderer as preview; optional single-channel API filter | Document background or explicit transparent/white override; never Preview Surface | Strongest pixel parity; covered by tests. |
| SVG | Clones/normalizes; regenerates Marks or Curve outlines; emits editable channel groups (`src/svg_export.rs:16-103`, `106-203`) | Uses document output mode for group blend style | Emits optional named Export Background layer; never Preview Surface | Equivalent generator functions, not one retained geometry object; rasterization tolerance is tested only for selected fixtures. |

### 11.1 Existing parity risks

1. RGB-output Brightness alpha differs between Shapes and Curves.
2. A Crosshatch document can retain `RgbScreen`; raster uses Multiply from its
   K/C/M/Y channel identities, while SVG uses Screen from `Document.output_mode`.
3. Preview's legacy-white fast path composites during rasterization; other
   surfaces composite transparent artwork afterward, with acknowledged
   antialias differences (`src/render.rs:1035-1046`).
4. Preview, PNG, and SVG regenerate geometry separately. They use the same
   deterministic functions today, but no persisted/shared resolved-artwork
   object enforces identity.
5. Shapes cache identical sampling grids within one generation; Curves resample
   per enabled layer, increasing both drift risk after future changes and work.
6. Crosshatch visibility changes field partition before geometry, so hiding a
   layer changes other layer geometry.
7. SVG blend style derives from document output mode; raster blend derives from
   channel identity. Any mismatched state can diverge beyond Crosshatch.
8. Parity tests cover representative shapes/curves, not every mapping × alpha ×
   output-model combination.

## 12. Persistence and presets

### 12.1 Project document

`DOCUMENT_VERSION` is 5 (`src/model.rs:6-7`). Version 5 serializes the complete
`Document` with serde, including:

- explicit `output_mode`;
- combined `value_mode` and `single_channel` inside every active, saved, or
  cached Shapes/Curves setting;
- seven channel records per settings object;
- Crosshatch mode/color/settings;
- appearance;
- inactive output-mode treatment caches.

Versions 1-3 have no output mode and migrate to CMYK; version 4 is also forced to
CMYK even if an `output_mode` field is present. Versions 1-3 receive visible
white Preview Surface and Export Background. Version 5 round-trips explicit RGB
and inactive caches (`src/persistence.rs:55-145`, `452-504`). Unsupported document
versions fail visibly. Unknown serde enum values fail parsing; missing fields
with `#[serde(default)]` use documented defaults.

The project stores both a separate output mode and a combined mapping. It does
not store an independent Artwork Source, alpha policy, or assignment.

### 12.2 Treatment presets

- Native Basic uses preset version 2 with `settings` and `render`.
- Shapes and Curves save web-compatible preset version 1 plus a complete
  `nativeRender` (`src/preset.rs:255-289`, `351-487`).
- Presets do not persist source artwork, appearance, document identity,
  `Document.output_mode`, active/saved treatment caches, or an explicit
  assignment.
- Applying a preset calls `set_treatment`; `Cmyk`/`Rgb` mappings indirectly
  switch output while neutral mappings preserve the receiving document's output.
- The raw v1 parser accepts `cmyk`, `luminance`, `crosshatch-luminance`, and
  `single-channel`, and only `c/m/y/k` active inks
  (`src/preset.rs:1211-1227`). It does not accept raw `rgb` or `r/g/b`.
- Saved current RGB presets still work because `nativeRender` is preferred and
  serde can deserialize `ValueMode::Rgb` and RGB `Ink` variants.
- Unknown format/version/mode/color/range values reject the preset without
  mutating the live document.

### 12.3 Confirmed Stage 1 migration inputs

Stage 1 must accept:

1. project versions 1-3 with Native Basic or legacy render state and implicit
   CMYK/white appearance;
2. project version 4 with implicit CMYK;
3. project version 5 with explicit output mode plus combined active, saved, and
   cached mappings;
4. preset version 1 raw web fields without `nativeRender`;
5. preset version 1 with `nativeRender`;
6. preset version 2 native `render` and optional native settings;
7. legacy shape Crosshatch requiring Curve normalization;
8. all five serialized `ValueMode` strings generated by serde:
   `cmyk`, `rgb`, `luminance`, `crosshatch-luminance`, `single-channel`;
9. raw preset aliases limited to the four strings and CMYK ink IDs accepted by
   the current legacy parser;
10. all active/saved/inactive occurrences, not only `Document.render`.

## 13. Existing automated coverage and gaps

### 13.1 Coverage present

- Formula vectors for all five web modes, fully transparent pixels, and binary
  nonzero-alpha CMYK behavior (`src/render.rs:2023-2067`).
- Direct RGB component and alpha weighting (`src/render.rs:1662-1680`).
- RGB-output Shapes Brightness partial-alpha coverage
  (`src/render.rs:1722-1819`).
- CMYK reference conversion (`src/render.rs:1642-1660`).
- Output-mode caching, undo, and explicit/neutral mapping classification
  (`src/model.rs:1912-2023`).
- Project versions, output-mode round trips, appearance, inactive caches, and
  shape-to-curve Crosshatch migration (`src/persistence.rs:296-539`).
- Preset validation/round trips and genuine Crosshatch
  (`src/preset.rs:1647-1819`).
- PNG canonical raster equivalence and appearance overrides
  (`src/png_export.rs:192-345`).
- SVG editable layers, Shapes raster tolerance, Curve outline identity,
  Crosshatch labels, and Export Background (`src/svg_export.rs:390-616`).
- Mapping index table, invalid slots, live StringList identity, realized GTK
  selector callbacks, Crosshatch directions, and appearance controls
  (`src/ui.rs:12110-12646`).

### 13.2 Stage 1 evidence gaps

- No test directly compares RGB-output Brightness alpha between Shapes and Curves.
- No transparent/partial-alpha matrix covers every mapping × output × treatment.
- No parity test exercises RGB-mode Crosshatch across preview, PNG, and SVG.
- No oversized raster/SVG fixture locks the 2400-pixel source-preparation stage
  together with later Triangle-filtered field sampling.
- No translucent SVG fixture locks the current premultiplied-byte path and its
  effective repeated alpha attenuation.
- No test proves an active destination remains valid when a mapping-driven
  output switch commits settings cloned from the previous model.
- No raw legacy preset test covers the unsupported `rgb`/RGB-ink limitation as a
  migration contract.
- No test migrates every combined mapping occurrence in active, saved, and both
  inactive caches.
- No source/alpha/assignment model tests can exist yet because those types are absent.
- Realized GTK coverage is broad but remains an in-process callback regression,
  not an end-to-end visual inspection of every mapping's output.

No diagnostic test was added for Stage 0; code and existing locked tests provide
unambiguous formula evidence.

## 14. Confirmed migration table

### 14.1 Mapping values

| Legacy input | New Artwork Source | Alpha policy | Output Model | Assignment | Compatibility/treatment action |
|---|---|---|---|---|---|
| `ValueMode::Cmyk`, `cmyk`, Color → CMYK Inks | `source.full_color` | `source_alpha.legacy_current_v1` for migrated work | `output.cmyk_print` | `assignment.automatic(separation.cmyk.encoded_rgb_max_black_v1)` | Preserve Shapes/Curves pattern and channel state. |
| `ValueMode::Rgb`, `rgb`, RGB Color → Screen | `source.full_color` | `source_alpha.legacy_current_v1` | `output.rgb_screen` | `assignment.automatic(separation.rgb.direct_encoded_components_v1)` | Preserve Shapes/Curves pattern and RGB state. |
| `ValueMode::SingleChannel`, `single-channel`, Brightness → One | `source.legacy_brightness.encoded_rec709_luma_darkness_v1` | `source_alpha.legacy_current_v1` | Preserve owning document/cache output | `assignment.active_channel` plus validated stable channel ID | Preserve pattern state; translate cross-model slot aliases deliberately. |
| `ValueMode::Luminance`, `luminance`, Brightness → All | same legacy source | same legacy alpha policy | Preserve owning document/cache output | `assignment.all_channels` | Preserve pattern and enabled-channel state. |
| `ValueMode::CrosshatchLuminance`, `crosshatch-luminance`, Brightness → Crosshatch | same legacy source | same legacy alpha policy | Preserve serialized document/cache output | `assignment.compatibility(compat.crosshatch.progressive_kcmy_v1)`; active channel must be `None` | The compatibility assignment exclusively owns progressive K/C/M/Y routing. Normalize shape form to Curves exactly as today; preserve color, angles, visibility, thresholds, opacity, channel settings, and current path-dependent raster/SVG blending. |

`source_alpha.legacy_current_v1` is a compatibility identifier for the complete
path-dependent table in section 4.6. It is intentionally not mislabeled
“Preserve.” New documents may use `source_alpha.preserve`; migrated output must
not change merely to simplify the enum.

### 14.2 Schema/container variants

| Input container | Confirmed migration rule |
|---|---|
| Project v1 | Native Basic, implicit CMYK, legacy white appearance; preserve native formula/alpha behavior. |
| Project v2-v3 | Migrate every render mapping; implicit CMYK; v3 saved Shapes/Curves included; preserve legacy white appearance. |
| Project v4 | Migrate every active/saved mapping; force CMYK as current loader does; preserve v4 appearance. |
| Project v5 | Migrate active render, saved Shapes/Curves, and both inactive output caches; use explicit output mode for each owning treatment context. |
| Preset v1 raw fields | Translate accepted raw `valueMode`/`singleChannel`; retain current rejection of unknown/RGB raw variants unless a deliberate new parser migration is added. |
| Preset v1 `nativeRender` | Migrate the embedded render, including RGB and RGB channel variants accepted by serde. |
| Preset v2 | Migrate embedded render and native settings; no explicit output mode exists, so preserve current facade rule: explicit CMYK/RGB mapping may request output, neutral mapping does not. |

## 15. Recommended Stage 1 design

### 15.1 One authoritative representation

Add one document-level semantic object, represented here conceptually:

```rust
struct ArtworkPipelineState {
    source: ArtworkSourceId,
    alpha_policy: SourceAlphaPolicyId,
    output_model: OutputModelId,
    assignment: ChannelAssignment,
    active_output_channel: Option<OutputChannelId>,
}

enum ChannelAssignment {
    AutomaticColorSeparation(SeparationStrategyId),
    ActiveChannel,
    AllChannels,
    Compatibility(ArtworkCompatibility),
}
```

Rules:

- `ArtworkPipelineState` is the only mutable source of truth for these concepts.
- `ValueMode` remains only as a deserialization/input compatibility type during
  migration; it is not stored independently beside the new state.
- `Document.output_mode` is migrated into `output_model` rather than retained as
  a second authoritative field.
- Shapes/Curves settings no longer authoritatively own source, alpha, assignment,
  or active destination. Temporary adapters may project the new state into old
  renderer calls until Stage 2.
- Automatic separation serializes its versioned strategy ID. Migrated CMYK and
  RGB mappings select the exact legacy strategy IDs below; the strategy is not
  inferred later from the output model.
- Active channel is a stable ID validated against the authoritative output
  model. It is present only for `ActiveChannel`; every other assignment requires
  `None`.
- Crosshatch is represented only by
  `Compatibility(LegacyProgressiveCrosshatchV1)`. That assignment exclusively
  owns progressive K/C/M/Y routing; it is not simultaneously Automatic, Active,
  or All, and it is not source or output state.
- For migrated v1-v5 documents, each existing mode cache is converted once to a
  complete, valid semantic snapshot plus its pattern/channel state. Selecting an
  output mode activates that owning cache snapshot and stores the previous
  active snapshot, exactly preserving today's mode-switch restoration. This is
  a versioned migration-only compatibility exception to the general rule that
  Output Model does not replace Artwork Source. New state does not independently
  mutate a cache, and no cache competes with the currently active snapshot.

### 15.2 Legacy Artwork Mapping facade

Keep the current five-entry GTK control unchanged through Stage 1. A deliberate
user selection is translated once:

```text
legacy dropdown action
  -> LegacyArtworkMappingAdapter
  -> one atomic ArtworkPipelineState update
  -> compatibility pattern/treatment transition when Crosshatch requires it
  -> existing document edit/undo/recovery/render flow
```

Control synchronization performs the reverse projection from authoritative
state to a legacy label only when the state exactly matches one of the five
legacy combinations. It must not store a second combined value. A nonrepresentable
future state needs an explicit “Custom”/unavailable presentation in Stage 4, not
an arbitrary first index.

Preset migration should use the same adapter at the parse boundary. Generic
render setters must not infer output model.

### 15.3 Stable identifiers

The proposed dotted identifiers are accepted with these additions:

```text
source.legacy_brightness.encoded_rec709_luma_darkness_v1
source_alpha.legacy_current_v1
separation.cmyk.encoded_rgb_max_black_v1
separation.rgb.direct_encoded_components_v1
compat.crosshatch.progressive_kcmy_v1
```

The existing proposals remain accepted:

```text
source.full_color
source.red
source.green
source.blue
source.value
source.perceptual_lightness
source.alpha
source_alpha.preserve
source_alpha.ignore
output.cmyk_print
output.rgb_screen
channel.cmyk.cyan / magenta / yellow / black
channel.rgb.red / green / blue
assignment.automatic
assignment.active_channel
assignment.all_channels
```

Pattern identifiers stay deferred to TON-010. Do not reuse legacy `Ink::id()` or
GTK positions as persisted stable IDs.

### 15.4 Stage boundaries

**Stage 1 — authoritative model and compatibility adapters**

- add the independent semantic types and stable IDs;
- add the single authoritative state and validation;
- serialize the automatic-separation strategy ID and compatibility assignment;
- translate all five legacy actions and parsed preset values atomically;
- remove output inference from generic render setters;
- validate active channels on output changes;
- represent legacy alpha and Crosshatch explicitly;
- migrate mode caches to complete semantic snapshots and preserve their
  versioned restoration exception;
- add the minimum project schema bump and read/write migration needed for the
  new state to survive save/reopen;
- add migration tests for all active/saved/inactive locations.

Minimal persistence must move into Stage 1. An authoritative state that is lost
or reconstructed from `ValueMode` after reopening is not authoritative. Stage 1
therefore needs a new document version, serialization of the semantic object,
and migration from versions 1-5. It does not need the full preset redesign.

**Stage 2 — resolved channel fields**

- centralize web sampling, formulas, alpha policy, separation, and routing;
- preserve both the 2400-pixel source-preparation stage and per-grid Triangle
  sampling, with oversized raster/SVG fixtures;
- produce stable resolved channel fields and appearance;
- make Shapes and Curves adapters consume those fields;
- remove slot aliasing and duplicate output inference;
- establish one canonical resolved object for preview/PNG/SVG generation.

**Stage 3 — preset migration and compatibility cleanup**

- introduce scoped preset semantics using the stable IDs introduced in Stage 1;
- migrate/rewrite saved treatment presets without relying on `nativeRender` as a
  hidden semantic source;
- define unknown-ID preservation/error behavior;
- finish cached/saved compatibility cleanup once Stage 2 no longer requires old
  renderer projections.

**Stage 4 — separate GTK controls**

- replace the facade with Artwork Source, Source Alpha, Output Model, Assignment,
  and active-channel controls;
- preserve all P0 callback, invalid-position, live-model, and deferred-sync rules;
- retain progressive disclosure and clear creative terminology.

**Later stages/issues**

- Preview/export parity hardening follows the resolved-field pipeline. The
  RGB-mode Crosshatch SVG Screen-to-Multiply correction belongs there and needs
  explicit behavioral-change approval.
- TON-008 RGB Curves resumes only after the corrected pipeline.
- TON-010 owns general Crosshatch/pattern registry work.
- TON-011 owns advanced per-channel source/pattern overrides.
- TON-009 remains a post-compositing output treatment.

### 15.5 Recommended Stage 1 file scope

- `src/model.rs`: authoritative types/state, validation, legacy adapters, cache
  ownership, undo snapshot integration.
- `src/persistence.rs`: document version bump and v1-v5 migration.
- `src/preset.rs`: parse-boundary compatibility intent only; defer full preset
  schema redesign.
- `src/ui.rs`: keep the existing UI, route mapping/output callbacks through the
  adapter, preserve P0 synchronization.
- focused model/persistence/UI tests in those modules.

`src/render.rs`, `src/curve_render.rs`, `src/png_export.rs`, and
`src/svg_export.rs` should receive only the minimum adapter inputs required to
compile in Stage 1. Formula centralization and output changes belong to Stage 2.

## 16. Risks

1. Modernizing partial-alpha behavior during model extraction would change
   legacy output before Stage 2 has a parity matrix.
2. Leaving `ValueMode` mutable beside the new state would create two sources of
   truth.
3. Deferring all persistence would make the new state non-authoritative after
   reopen.
4. Mapping-driven output switches can carry an invalid cross-model active
   channel through shared scalar slots.
5. Crosshatch under an RGB document already exposes raster/SVG blend inference
   disagreement.
6. Moving Crosshatch directly into TON-010 would exceed TON-012 scope and risk
   changing its progressive formula.
7. Updating GTK models synchronously or replacing live models would regress the
   P0 crash fixes at `08459db`.
8. Migrating only the active render would silently lose semantics in saved
   Shapes/Curves or inactive output caches.
9. Calling encoded luma “luminance,” “Value,” or “Perceptual Lightness” would
   silently change either meaning or math.
10. Omitting the 2400-pixel Lanczos3/scaled-resvg preparation stage from fixtures
    would allow Stage 2 to change established sampled fields unnoticed.

## 17. Deferred decisions

- Whether new Perceptual Lightness uses OKLab `L` immediately.
- Exact new-document Alpha-source coverage semantics.
- Hidden-RGB versus matte behavior for `source_alpha.ignore`.
- Whether source selection remains document-wide in the first TON-011 release.
- Final pattern IDs and general Crosshatch representation under TON-010.
- Full preset scope/schema and whether workflow presets may intentionally carry
  output model in addition to treatment state.
- Whether legacy path-dependent alpha is retained indefinitely or upgraded only
  through an explicit user migration.
- Performance representation and cache invalidation for resolved channel fields.

The exact current Brightness formula, current CMYK formula, legacy alpha table,
stable legacy Brightness ID, minimum Crosshatch compatibility state, and need for
minimal Stage 1 document persistence are no longer provisional.

## Appendix A. Stage 1A implementation contract (2026-07-21)

Stage 1A adds `src/artwork_pipeline.rs` without changing `Document`, schema v5,
persistence, presets, GTK, renderers, source formulas, alpha behavior, or
exports. This appendix supplements, rather than rewrites, the Stage 0 evidence
above. The earlier `encoded_rec709_luma_darkness_v1` spelling remains historical
audit text only; the sole Stage 1A compatibility ID is
`source.legacy_brightness.encoded_rec709_inverted_v1`.

Implemented public types are `ArtworkSource`, `LegacyBrightnessKind`,
`SourceAlphaPolicy`, `OutputModel`, `OutputChannelId`,
`AutomaticSeparationStrategy`, `LegacyCompatibilityAssignment`,
`ChannelAssignment`, and `ArtworkPipelineSettings`. Migration-only types are
`LegacyPipelineSnapshot`, `LegacyPipelineConversion`, `LegacySnapshotOrigin`,
`LegacyTreatmentKind`, and `LegacyScalarTarget`; none is normal document state.

| Category | Stage 1A stable IDs |
|---|---|
| Sources | `source.full_color`, `source.red`, `source.green`, `source.blue`, `source.value`, `source.perceptual_lightness`, `source.alpha`, `source.legacy_brightness.encoded_rec709_inverted_v1` |
| Alpha | `source_alpha.legacy_current_v1`, `source_alpha.preserve`, `source_alpha.ignore` |
| Output | `output.cmyk_print`, `output.rgb_screen` |
| Channels | `channel.cmyk.cyan`, `.magenta`, `.yellow`, `.black`; `channel.rgb.red`, `.green`, `.blue` |
| Assignment | `assignment.automatic` with a required separation payload, `assignment.active_channel`, `assignment.all_channels`, `compat.crosshatch.progressive_kcmy_v1` |
| Separation | `separation.cmyk.encoded_rgb_max_black_v1`, `separation.rgb.direct_encoded_components_v1` |

`validate()` is strict and never normalizes: automatic requires Full Color and
the matching output strategy; Active/All require a scalar source; Active needs
a present member channel; retained channels are always output-membership checked;
Crosshatch exclusively requires legacy Brightness plus `LegacyCurrentV1` and
keeps its K/C/M/Y compatibility layers outside RGB membership. CMYK ordering is
C/M/Y/K, RGB ordering is R/G/B, and legacy slots map only C/R, M/G, Y/B, K;
RGB slot 3 and all invalid positions are errors.

For migration conversion, the explicit owning output plus scalar slot determines
the semantic channel. A legacy cross-model `Ink` alias with the same slot is
translated deliberately (Cyan/slot 0 under RGB becomes Red; Red/slot 0 under
CMYK becomes Cyan); only disagreeing slots are ambiguous.

`normalize_legacy_active_channel()` is a migration-only repair that preserves a
compatible retained editor channel. `transition_output_model()` is separate and
returns a validated result: Full Color Automatic receives the matching strategy;
scalar source, alpha policy, and Active/All assignment remain independent while
a compatible restored/same-slot/default channel is selected; Crosshatch remains
compatibility-only with no active channel.

`pipeline_from_legacy()` accepts a bounded snapshot with the coupled mapping,
serialized owning output, current output, scalar target (One/All), destination
and slot, treatment, Crosshatch flag, and origin. Active/Saved snapshots require
serialized and current output equality. Inactive CMYK caches own serialized
CMYK while the current output is RGB; inactive RGB caches own serialized RGB
while current is CMYK. Neutral One/All and Crosshatch always preserve that
serialized owning output. Explicit Color/RGB mappings must agree with their
forced CMYK/RGB outputs. Native Basic, missing/mismatched scalar targets,
contradictory origin/output, invalid slots, and ambiguous snapshots return
structured errors.

`project_legacy_value_mode()` reverses only Full Color CMYK/RGB automatic and
legacy Brightness One/All/Crosshatch combinations. It validates first, then
returns `UnsupportedReverseProjection` for new sources or modern alpha policy
instead of inventing legacy renderer semantics. Related structured errors are
`UnknownStableIdError`, `PipelineStateError`, `LegacySlotError`,
`LegacyPipelineConversionError`, and `LegacyProjectionError`.

Stage 1B is still deferred: it will make the domain state authoritative and
migrate active, saved, and inactive containers. No live application behavior
changed in Stage 1A, including the known RGB Crosshatch raster/PNG versus SVG
blend mismatch.
