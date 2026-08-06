# TON-010 Stage 5 Manual Acceptance

## Build identity

- Branch: `TON-010-Stage5-Framework-Restart`
- Commit: `1051a6d2a1c0b18aee3144f8eb4141cdfe4eb0f7`
- Preservation tag: `TON-010-stage5-framework-pre-compositor-fix`
- Test date: 2026-07-29
- Platform: Fedora 44 / GNOME 50

## Framework checkpoint

The Stage 5 framework restart is preserved before the compositor and
direct-positive-polygon corrections.

The preserved implementation includes:

- reusable deterministic point placement in `src/site_distribution.rs`;
- pure Voronoi geometry in `src/voronoi_geometry.rs`;
- Weighted Voronoi integration in `src/weighted_voronoi.rs`;
- canonical preview, PNG, and SVG output routing;
- semantic RGB and CMYK channel support;
- persistence, presets, undo/redo, and Blueprint UI integration.

Shapes and Curves do not yet use the new generation framework.

## Manual test artifacts

Local evidence directory:

```text
.codex-work/evidence/ton-010-stage5-manual/
````

Known reference artifact:

```text
rgb-expected-krita.png
SHA-256: 127e8f1c8660faaced89699fa63f2b0ee7281e24ce2be860dd3dfa7e0f5e3537
```

Additional screenshots, individual channel exports, actual composites, SVG
exports, and settings should be added to the same evidence directory.

## Confirmed findings

### Individual channel generation

* Each RGB channel appears correct when rendered individually.
* Each CMYK channel appears correct when rendered individually.
* Channel-specific source weighting and Voronoi placement therefore appear
  correct.
* The confirmed defect occurs after individual channel geometry has been
  generated.

### Krita reference composition

The individual Toniator channel outputs were imported into Krita and composed
manually.

The resulting RGB and CMYK composites match the intended output substantially
better than Toniator's own preview and PNG composite.

The exact Krita layer order, blending modes, opacity settings, background
policy, and alpha procedure still need to be recorded below.

#### RGB Krita settings

* Layer order:
* Blend mode:
* Layer opacity:
* Document background:
* Alpha handling:
* Other processing:

#### CMYK Krita settings

* Layer order:
* Blend mode:
* Layer opacity:
* Document background:
* Alpha handling:
* Other processing:

## Confirmed raster compositing defects

### RGB

Toniator's combined RGB preview and PNG render overlapping RGB channels too
darkly.

Expected additive RGB behavior:

* red plus green produces yellow;
* red plus blue produces magenta;
* green plus blue produces cyan;
* red plus green plus blue produces white.

The SVG export appears to exhibit the correct RGB overlap behavior, making it a
useful reference for correcting the raster compositor.

### CMYK

Toniator's combined CMYK preview and PNG do not preserve the expected
independent ink contributions.

A likely failure mode is that one channel's subtractive geometry is erasing
previously rendered sibling channels on the shared final destination.

Subtraction associated with a semantic channel must affect only that channel's
coverage before the completed channel surfaces are combined.

## Confirmed SVG findings

The SVG contains separate named semantic-channel layers, including examples
such as:

```text
Weighted Voronoi channel.rgb.red
Weighted Voronoi channel.rgb.green
Weighted Voronoi channel.rgb.blue
```

At extreme zoom, RGB channel combinations appear correct in Inkscape.

Each channel layer currently contains:

* a clip, apparently used for the page or document boundary;
* a mask, apparently used to subtract the difference between the raw Voronoi
  cell and its final inset polygon.

Releasing the mask in Inkscape is impractical because the operation freezes or
becomes prohibitively expensive with the large number of paths and nodes.

This structure complicates downstream vector editing.

## Confirmed cell-boundary artifact

A faint antialiased line remains along the original Voronoi cell boundary.

This is not the intended visible edge of the final inset polygon.

The artifact can be removed manually by thresholding the alpha channel, which
indicates residual partial-alpha coverage along the original cell boundary.

A global alpha-threshold operation is not an acceptable application fix because
it would damage legitimate antialiasing at actual visible edges.

## Required canonical geometry correction

Weighted Voronoi already computes the final boundary-derived inset polygon.

The final canonical output should therefore contain:

```text
final inset polygon -> positive canonical region
```

It should not preserve the construction as:

```text
raw cell positive region
minus boundary-ring mask
equals final visible cell
```

The raw cell and boundary ring are intermediate geometry. They should not
survive into final editable artwork when the final visible polygon is already
known.

Required result:

* final inset polygons are emitted directly as positive canonical geometry;
* preview, PNG, and SVG consume those same polygons;
* no cell-sizing mask is required in SVG;
* genuine holes or knockouts may continue to use canonical subtraction;
* the original Voronoi centerline has no residual alpha;
* legitimate antialiasing remains at actual visible polygon edges;
* downstream vector applications can directly access the visible polygons.

## Preferred SVG structure

Each semantic channel should preferably contain:

```text
named semantic-channel group or Inkscape layer
    optional page/domain clip
    compound path containing final visible cell polygons
```

A compound path per channel is preferred because it:

* reduces object count;
* avoids unnecessary mask complexity;
* reduces the chance of same-color rasterization cracks;
* remains editable through Path -> Break Apart in Inkscape.

## Artifact matrix

| Model | Artifact                   | Result  | Notes                                           |
| ----- | -------------------------- | ------- | ----------------------------------------------- |
| RGB   | Red solo                   | Pass    | Individual channel appears correct              |
| RGB   | Green solo                 | Pass    | Individual channel appears correct              |
| RGB   | Blue solo                  | Pass    | Individual channel appears correct              |
| RGB   | Toniator preview composite | Fail    | Overlaps become too dark                        |
| RGB   | Toniator PNG composite     | Fail    | Does not match Krita reference                  |
| RGB   | SVG viewed in Inkscape     | Partial | Color mixing correct; boundary artifact remains |
| CMYK  | Cyan solo                  | Pass    | Individual channel appears correct              |
| CMYK  | Magenta solo               | Pass    | Individual channel appears correct              |
| CMYK  | Yellow solo                | Pass    | Individual channel appears correct              |
| CMYK  | Black solo                 | Pass    | Individual channel appears correct              |
| CMYK  | Toniator preview composite | Fail    | Incorrect combined channel behavior             |
| CMYK  | Toniator PNG composite     | Fail    | Does not match Krita reference                  |
| CMYK  | SVG viewed in Inkscape     | Partial | Boundary artifact remains                       |

## Tests still to perform

### Exact test settings

Record the exact settings used for the supplied reference artifacts:

* source filename and SHA-256;
* source dimensions;
* output model;
* enabled channels;
* cell count;
* seed;
* shared or independent arrangement;
* uniform or source-weighted placement;
* density polarity;
* density response;
* weight response;
* minimum cell scale;
* boundary gap;
* background color;
* transparent export setting;
* antialiasing setting;
* export dimensions and DPI.

### Established-pattern comparison

Test Shapes and Curves with the same source and output model.

Record whether their preview and PNG composites exhibit the same RGB and CMYK
failures.

This determines whether the raster defect is:

* shared by the canonical compositor; or
* specific to Weighted Voronoi's canonical channel metadata.

### Boundary-gap tests

For a nonzero gap:

* verify normal antialiasing at the visible inset-cell edge;
* verify the middle of the gap is completely clear;
* verify whether residual alpha exists at the original Voronoi bisector.

For zero gap:

* verify same-channel cells cover continuously;
* check for antialiased cracks between adjacent cells.

### Background tests

Test both:

* opaque white background;
* transparent export.

Record whether the channel-composition defect changes with the background
policy.

## Required next correction

1. Emit final Weighted Voronoi inset polygons directly as positive canonical
   geometry.
2. Remove cell-sizing masks and boundary-ring subtraction from final Weighted
   Voronoi artwork.
3. Correct preview and PNG model-aware RGB and CMYK channel composition.
4. Keep genuine subtraction channel-local.
5. Preserve separate semantic-channel groups in SVG.
6. Preserve legitimate edge antialiasing without retaining residual centerline
   alpha.
7. Validate preview, PNG, and SVG against the recorded Krita and Inkscape
   references.

## Correction pass result (2026-08-01)

The targeted correction is implemented and covered by focused automated tests.
Weighted Voronoi now emits only its final boundary-derived inset polygons as
positive canonical regions. Raster composition isolates each semantic channel
before applying genuine subtraction and combines RGB channels additively or
CMYK channels multiplicatively; transparent output preserves uncovered alpha,
while an opaque canonical background is applied explicitly when requested.
Semantic SVG layers now contain one compound positive path per channel and no
Weighted cell-sizing mask. The artboard clip remains as the canonical
page/domain constraint; genuine subtractive regions still use a layer-local
mask.

The final code pass reported 168 library tests, 48 binary/UI tests, zero doc
tests, strict Clippy, a locked release build, explicit Blueprint compilation,
and the realized GTK regression passing. Automated Weighted SVG/raster parity
was within the existing mean-channel tolerance. Manual Inkscape Break Apart,
visual comparison to the supplied Krita RGB/CMYK references, and human
GNOME/Wayland acceptance remain required before claiming final perceptual
acceptance.
