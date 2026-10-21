# Using Toniator

Start with the [README walkthrough and gallery](../README.md). This guide covers
controls you can return to as your artwork develops.

## 0.4.6 project and Preset formats

Toniator 0.4.6 includes the accepted artistic geometry, coverage-intent and renderer
changes described below. Current `.toniator` project schema is **19**, standalone
Pattern format **10**, and Document Preset configuration schema **18**. Project
container **2** and Document Preset envelope **3** remain unchanged. The published
0.4.5 projects use schema 17, Pattern format 8 and Preset configuration schema 16;
intermediate development schemas are also superseded. Ordinary projects are
current-only: older projects, including the published gallery projects, are
rejected. Keep 0.4.5 to reproduce those examples, or open their source artwork
in 0.4.6 and create a new project; there is no general project migration.
Current schema 19 projects saved by the unpublished app remain valid even if its
software version label was 0.4.5. Compatibility follows project schema, not that
label. Historical example files have not been regenerated.

Supported older source-free `.toniator-preset` files convert deterministically
while preserving original bytes and desired coverage for All, named channels,
and animation End. Conversion adopts the new defaults and can change appearance.
For coverage conversion, a former mechanism without a stored policy becomes
**Fixed**; an explicit generic **Cover** policy stays Cover. Authored controls and
All/named/End responses are retained. Recognized obsolete numeric companions are
cleared and genuinely prepared again; unknown profiles are rejected. Current
exact fits remain reusable without fresh measurements. The raw-response conversion
route uses explicit envelope/schema origin, not age alone: `(1, 7 or 8)` or `(3, 10 through
15)`; schema 16 already stores desired coverage.

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

Preview renders the whole artwork in the background at each visited eligible
zoom. Panning keeps that work running. The closest
available cached resolution appears first while detail at the requested zoom
arrives; returning to completed regions reuses their pixels. After an edit at a
zoomed view, a new full-image preview supplies coverage before detail resumes at
the same zoom and scroll position. Switching to **Source** and back retains cache
data for the same frame. Artwork edits, frame changes and a new workspace
invalidate it. Retention is automatic with an 8 GiB soft target, so older zooms
can be evicted; it is not saved with the project or retained after quitting.
Unsupported or oversized cache views use ordinary preview rendering. See
[measured performance](PERFORMANCE.md#046-retained-preview-regions).

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
It also stores the compatible calibration, measuring one if needed. Bundled
Patterns include their current calibrations and reuse them on Apply. Structural
customization can require a fresh measurement; applying alone does not update
the library entry. Saving is separate from applying. Personal Patterns can be
updated, copied, renamed, or moved to recoverable trash through **Manage Pattern
Library…**.

### Coverage and placement

Spirals persist **Fixed** or **Cover canvas** intent. Under Cover, **Revolutions**
is the authored minimum: after Feature size resolves spacing, generation extends
the shape-specific extent to the inverse-placed canvas with finite guards. Fixed
keeps its finite authored extent. Explicit arcs and finite repetition also remain
finite. These policies follow typed recipe data rather than Pattern names.

Whole-pattern X/Y offsets and Rotation preserve saved calibration, its exact
numeric artifact/profile and desired All, named-channel and animation End responses;
placement alone needs no new fitting. Corrected producers require one genuine
preparation before that reuse. Feature size scales tone sampling proportionally,
with the original authored-core tone anchor retained. Structural AlongGuides sites
keep their existing full-path-start phase; changing derived runway can change
those site positions.

Guard coverage also applies to other families and their derived output. Maze
constructs complete bounded cells, real perimeter openings and its solution before
final cropping; openings may lie outside the visible canvas. Straight Guide Faces
and straight-intersection Voronoi reserve proved cell reach. Random or
artwork-weighted gaps can be intentional; these checks do not establish a universal
coverage theorem for random or curved Voronoi cells.

### Tone sample spacing

For guide lines, parametric curves/spirals, and Curve Motif paths, **Tone sample
spacing** controls how often the artwork changes line thickness along a curve.
It is a positive finite multiplier, default **1**, expressed as × Feature size.
Larger values simplify tonal detail; smaller values retain more changes. This
control is absent from connected-point paths and mazes. Geometric point spacing
remains a separate control.

The requested interval in document units is
`multiplier × max(canvas width, canvas height) / 100 × F`, where
`F = default_for_canvas(canvas).density / effective_density`. At fixed canvas and
aspect, doubling Feature size doubles this interval. Tooltips and accessible help
show both the multiplier and resolved distance; export resolution does not change
it. The complete logical curve keeps its anchored arc-length sampling lattice
across clipping and subdivision, including genuine finite endpoints. Completed
mapped, alpha-associated, cutoff and calibrated thickness is interpolated linearly
between samples. Curve accuracy, corners, seams and sampled zero-width breaks
remain intact, but coarse spacing can miss artwork features between samples.
Preview, PNG and SVG share the result.

### Connection controls

Nearest, random and spanning-tree connections expose these controls; Random Walk
and mazes do not use this selection policy.

| Control | Meaning |
| --- | --- |
| **Candidate neighbors per point** | Searches 1–32 nearby candidates for the original program; this is separate from the final number of links. |
| **Limit links per point** | Off by default, leaving final degree unrestricted. Turning it on initializes the limit to 2. |
| **Maximum links per point** | Positive final degree limit, enabled by the switch above; values above 32 are supported. Two prevents branching; one permits disjoint pairs. |
| **Allow loops** | On by default. Off forbids graph cycles. A spanning tree shows false/disabled while preserving the preference for a later program switch. |
| **Minimum path length (points)** | At least 1, default 1; counts distinct connected points, including branches, rather than length in document units. |

After deterministic final selection, singletons and undersized groups try automatic
noncrossing joins within the original maximum connection distance. Joins obey the
final degree/cycle limits and use existing points only. Joining happens before
small groups are hidden. Separate groups can remain. Ordinary program crossings
are allowed and are not junctions; the noncrossing rule applies to rescue joins.
At minimum 1, an unresolved singleton becomes an ordinary circle using the
connected source response, with Bias ignored. Connected strokes and circles travel
together through preview and exports.

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
overlaps, for eligible RGB/CMYK responses. Artwork, Feature size, whole-pattern X/Y,
channel rotation and response edits reuse it; finishing a structural change replaces it.
**Apply** in Advanced Settings automatically measures any missing calibration
fits before applying your edits in one undoable change. Cancel keeps the document
unchanged; failed work is not kept. **Recalibrate** forces a fresh fit for the
selected channel or every channel in All, without applying your draft. It is not
a prerequisite for Apply.
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

The renderer corrects additive RGB SVG composition and keeps unique
editable geometry IDs. External SVG engines can still differ in edge coverage
and color; a known **Source color + alpha** overlap discrepancy remains. A
precision refusal leaves an existing destination intact. See the
[renderer audit](RENDERER_AUDIT_2026-10-20.md#remaining-findings-and-limits).

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

If encoding fails, the existing **Retry**, **Save PNG**, and **Discard** actions
handle retained frames. A successfully published video or complete PNG sequence
shows **Complete**, its saved path and **Close** even when a later durability or
temporary-cleanup step reports a warning. The warning describes any retained
recovery files. **Cancel** stops active encoding and cleans its generated
temporary workspace when cleanup succeeds.

Development builds require FFmpeg/ffprobe for video, GIF, animated AVIF, and video
export. JPEG XL decoding is native. Linux packages include software media tools;
see the [package guide](../packaging/README.md).

## Preview status and help

The thin bar at the bottom of the canvas shows **Background rendering progress**
across the eligible zooms you have visited. It disappears when that work completes
or stops. It has no setting or action, and does not take keyboard focus; you can
keep panning and editing while it runs.

Very fine Patterns can take longer. If preview fails, **Preview unavailable**
retains the last successful pixels; **No preview yet** identifies a first failure.
**Retry preview** makes one attempt. **Dismiss preview error** hides the message
without restarting work. A changed setting or frame requests an updated preview;
view changes reuse retained pixels where available.
**Details** contains the diagnostic. The canvas has no status hover hint;
independent Save/operation errors remain.

[Known issues](../ISSUES.md), [release notes](releases/v0.4.6.md), and
[performance evidence](PERFORMANCE.md) record current limits. Use
**Main Menu → Help and feedback…** to reach documentation or report a problem.
