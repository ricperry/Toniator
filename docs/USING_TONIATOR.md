# Using Toniator

Start with the [README walkthrough and gallery](../README.md). This guide covers
controls you can return to as your artwork develops.

## Open, compare, and save

**Start New Project** opens an image or existing `.toniator` project from the
welcome window. **Recent Files** provides quick access later. Click the editor
behind welcome, press Escape, or close welcome to work with an empty document.
In the editor, **New** makes an empty document, **Open artwork…** adds an image,
and **New options** contains Open, image-sequence import, and Document Presets.
Saving a project and exporting artwork require source artwork.

Still-image inputs include PNG, SVG, JPEG, WebP, BMP, TIFF, OpenEXR, AVIF, and
JPEG XL. SVG live text depends on available fonts. JPEG XL imports preserve the
original encoded source while normalizing orientation, ICC conversion, grayscale,
16-bit input, and straight alpha to Toniator's sRGB8 source representation.
HDR/high-bit-depth output is not retained; primary premultiplied alpha is rejected.

**Preview / Source** compares the design with its original. Zoom in to inspect
marks and drag with the primary mouse button to pan beyond Fit. **Fit** restores
the whole image. These view changes do not change export resolution.

Use **Main Menu → Save** or **Save As…** for a portable `.toniator` project with
embedded source and editable settings. **Close** returns to startup; **Exit**
quits. Both prompt about unsaved document changes. Undo/Redo applies to document
edits. **F1**, **Help and feedback…**, and **About Toniator** provide help links.

## Color and channel controls

Choose **RGB**, **CMYK**, or **Source color + alpha** under **Model**. RGB uses
colored light on black; CMYK uses overlapping process paints on white. Source
color + alpha samples the image's own color and transparency for supported
marks/regions; guide-path output requires solid channel paint.

**All** edits shared settings; individual R/G/B or C/M/Y/K channels can use
different Patterns, rotations, offsets, responses, and paints. Each channel's
Pattern draws one kind of output: shapes, lines, or filled areas. For multiple
independent renders within one channel, export them separately and composite
outside Toniator.

**Feature size** makes the construction finer or coarser across families.
Smaller values tighten spacing and add detail; larger values expose its structure.
The minimum is 0.01. **Stretch X / Y**, **Rotation**, **Shape rotation** where
applicable, and offsets adjust the design. Pattern-specific variation numbers
produce repeatable alternatives.

**Settings** shows or hides the inspector. In a narrow window it occupies the
full width; **Back to canvas** or Escape returns to the artwork while retaining
settings and view. Recipe details, Output options, and Print preparation begin
collapsed.

## Patterns and the Wizard

**Change…** opens the Pattern Gallery. Choose a built-in card and **Apply Pattern**,
or **Customize** it. **Create new** starts a new construction. The Wizard follows
four decisions: **Family → Family options → Drawing → Review**. It validates the
current construction at document size before Apply. Applying is one undoable
change; Cancel discards private edits after confirmation when needed.

The Wizard's neutral gradient study explains the Pattern, not its appearance on
your artwork. Thin blue guides and orange point indicators show the underlying
construction; those overlays are never exported. The Wizard chooses structure;
Advanced Settings owns fill/thickness ranges, region sampling, and mark sizing.

Scatter offers **Random**, **Poisson disk**, **Jittered cells**, and **Clustered**.
Pattern point spacing controls authored scale. **Point density** determines
whether placement follows artwork; **Keep points apart** adds an exclusion rule.
Cluster centers are specified per 100 points, and spread controls group size.
**Relax points (Lloyd)** moves points toward local cell centers. **Follow artwork
density** makes relaxation follow the weighting field; otherwise it aims for even
spacing. More relaxation can soften clusters.

**Save Pattern…** stores a reusable construction in your personal library.
Saving is separate from applying. Personal Patterns can be updated, copied,
renamed, or moved to recoverable trash through **Manage Pattern Library…**.

## Document Presets

A **Pattern** is a construction. A **Document Preset** is a complete source-free
design configuration, including channel choices and settings.

- **New options → Select preset…** offers fourteen built-in document looks.
- **Save preset** writes a source-free `.toniator-preset` file.
- **Load preset…** accepts that file or an intact `.toniator` project and applies
  its configuration to your current artwork. Open artwork first.

Loading retains the current artwork, canvas, project identity, and timing. It
applies settings in one undoable change. Saving a Preset does not save a project.
Projects use the current development format; supported older Document Presets
have a deterministic conversion path that preserves original files.

## Advanced response and appearance

**Fill response** determines how sampled tone changes mark size, path thickness,
or region fill. **Source weighting** independently determines how source-dependent
placement follows artwork. Each has component, inversion, gain/offset, black/white
points, gamma, contrast, and cutoff; Weighting has its own strength and curve.
Artwork-weighted placement is reevaluated at rotated/offset positions.

**Variable** marks vary size with fill. Eligible artwork-weighted mark Patterns
use **Uniform** sizing by default: a calibrated full-tone size plus
source-dependent survival. Uniform size scale (0–2, default 1) remains an artistic
adjustment. Its optional Fill mask is Off at cutoff 0. Minimum-center-distance
exclusion supports Uniform; Visible Mark Margin requires Variable sizing.

Saved Pattern calibration compensates for opaque geometric coverage, including
overlaps, for eligible RGB/CMYK responses. Artwork, Feature size, channel rotation,
and response edits reuse it; finishing a structural change replaces it.
**Recalibrate** updates the selected channel or every channel in All.
Calibration estimates overall coverage; it does not certify local color fidelity,
ICC behavior, or physical printing. Shape rotation can intentionally alter overlap
and tone. Explanatory tooltips and Fine adjustments describe individual controls.

## PNG and SVG output

**Main Menu → Export image…** opens a destination chooser. Choose PNG or SVG in
the format dropdown. PNG then provides background, antialiasing, and output size
(**1× native**, **2×**, **4×**, **8×**, or **Custom**).

Automatic PNG backing is black for RGB, white for CMYK, and transparent for
Source color + alpha. SVG output is transparent and preserves editable geometry.
PNG streams ordinary exports in bounded row bands; dense geometry can still use
substantial memory. **Cancel export** preserves an existing destination before
publication. Export uses the artwork captured when the operation starts.

### Print preparation

PNG **Print preparation → Prepare for garment printing** is off by default.
Enable it to set maximum physical width/height in mm or inches and DPI. One
aspect-preserving pixel size is calculated for the complete canvas; PNG density
metadata follows it. Preparation makes alpha ≥128 opaque and lower alpha
transparent. Correction thresholds are whole pixels (0–256); isolated-mark
removal cannot exceed minimum feature width. Optional corrections remove isolated marks, thicken narrow features,
or fill/grow narrow gaps; zero disables each correction.

Nonzero corrections currently support up to 32,000,000 pixels. The preview uses
the same prepared pixels as export when its size is supported. Zoom/pan and a
resizable divider help inspect it; narrow windows stack the controls and preview.
**Preview backdrop** and Garment color help inspect contrast. **Use garment color**
copies the viewer swatch once to a background or custom fill; these choices do
not change the project or ordinary exports. Preparation does not certify print
safety. See the [garment preparation documentation](GARMENT_PRINT_PREPARATION_PLAN.md)
for process-specific limits and the separate headless advisory preflight.

See the [transparent portrait and matched preparation zooms](examples/print-animation/README.md#alpha-and-garment-print-preparation)
for a 200 mm / 254 DPI worked example, original RGBA outputs, and exact checks.
These thresholds are illustrative and depend on artwork and printing process.

## Animate artwork

1. Open a video or animated image, or choose **New options → Import image
   sequence…** to review an ordered list and frame rate. A still image also works.
2. Edit **Start** with the normal Pattern/channel/appearance controls.
3. Select **End**; its first selection copies Start. Make your changes and toggle
   between the two states to inspect them. Property Animation disclosures choose
   interpolation for supported continuous values.
4. Use **Main Menu → Animation settings…** for exact frame rate and source interval.
5. Choose **Export video…** for a PNG sequence, lossless FFV1/Matroska, or optional
   AV1/WebM. The export dialog supplies timing, output-size, and subset controls.

Try the [scale/rotation animation](examples/print-animation/README.md#animate-scale-and-rotation)
or [independent channel motion](examples/print-animation/README.md#animate-independent-channels),
with editable projects, native videos, and exact endpoints.

Saving the project retains source media and animation settings. Current animation
uses Start/End interpolation without playback or scrubbing. Exports are silent;
source audio remains inside the original media. Variable-rate media can repeat or
skip frames at the selected project rate. Animated JPEG XL uses one finite pass;
embedded loops do not repeat playback. Image-sequence entries must be stills.

Development builds require FFmpeg/ffprobe for video, GIF, animated AVIF, and video
export. JPEG XL decoding is native. Linux packages include software media tools;
see the [package guide](../packaging/README.md).

## Preview status and help

Very fine Patterns can take longer. If preview fails, **Preview unavailable**
retains the last successful pixels; **No preview yet** identifies a first failure.
**Retry preview** makes one attempt. **Dismiss preview error** hides the message
without restarting work. A changed setting, frame, or view requests a new preview.
The status tooltip gives the diagnostic; independent Save/operation errors remain.

[Known issues](../ISSUES.md), [release notes](releases/v0.4.5.md), and
[performance evidence](PERFORMANCE.md) record current limits. Use
**Main Menu → Help and feedback…** to reach documentation or report a problem.
