<a href="assets/icon-final.svg"><img src="assets/appicon.png" alt="Toniator icon" width="96" /></a>

# Toniator

**Turn images into artwork made from dots, flowing curves, scattered shapes, cells, and mazes.**

Open an image, explore a Pattern, and make it your own. Toniator is a native
desktop app with independent color channels, a visual Pattern Wizard, and PNG
or editable SVG output. It is free software under [GPL-3.0-only](LICENSE).

[![Toniator 0.4.5 editing a portrait made from source-colored Poisson-disk marks](docs/examples/showcase/screenshots/editor.png)](docs/examples/showcase/screenshots/editor.png)

*The actual Toniator 0.4.5 editor. Source for the artwork above:*

<a href="docs/examples/showcase/source-portrait.png"><img src="docs/examples/showcase/source-portrait.png" alt="Original continuous-tone painted portrait used in the editor" width="180" /></a>

The portrait source was created with AI for these demonstrations. Every pattern
effect below is rendered by Toniator; screenshots are captured from the real app.
Click any image to inspect its full size.

## Get Toniator

Download the **0.4.5 Linux prerelease** from
[GitHub Releases](https://github.com/ricperry/Toniator/releases/tag/v0.4.5).
Choose an AppImage or Flatpak:

```bash
# AppImage
chmod +x Toniator-0.4.5-x86_64.AppImage
./Toniator-0.4.5-x86_64.AppImage

# Or Flatpak
flatpak install --user ./Toniator-0.4.5-x86_64.flatpak
flatpak run io.github.ricperry.Toniator
```

[Installation, upgrades, and Windows](docs/INSTALLING.md) ·
[Build from source](docs/BUILDING.md) · [Release notes](docs/releases/v0.4.5.md)

## Make your first artwork

1. Choose **Start New Project** in the welcome window and open an image.
   Try the included [raster](assets/raster-sample.png) or
   [vector](assets/vector-sample.svg) artwork. In the editor, **New** starts an
   empty document; **Open artwork…** adds your image.
2. Pick a **Model**: **RGB** for colored light on black, **CMYK** for overlapping
   ink colors on white, or **Source color + alpha** to paint shapes with the
   image's own colors and transparency.
3. Click **Change…** in **Pattern Recipe**. Select a gallery card and choose
   **Apply Pattern**, or **Customize** to explore how it is constructed.
4. Start with **All** channels. Adjust **Feature size**: smaller values add finer
   detail; larger values emphasize the pattern. Try **Rotation** and offsets,
   then select an individual channel to give it a different arrangement.
5. Use **Preview / Source** to compare with the original. Zoom in to inspect
   details, drag to pan when zoomed beyond Fit, and choose **Fit** to see it all.
6. Open **Main Menu → Save** to keep an editable `.toniator` project, including
   its source artwork. Choose **Export image…** for PNG or SVG.

**A quick starting point:** after opening your image, choose
**New options → Select preset…** to try a complete built-in look such as
Magazine CMYK, Skinny Curve, or Controlled Splatter CMYK.

## Explore the pattern behind the image

The gallery supplies a starting point; the Wizard exposes its construction.
Choose **Guides**, **Scatter**, or **Spiral curve**, adjust the family options,
choose what it draws, and review before applying. **Back** lets you experiment;
**Apply Pattern** changes the document in one undoable step.

| Choose a Pattern | Shape its distribution |
| --- | --- |
| [![Real Pattern Gallery with Poisson Disk Marks selected](docs/examples/showcase/screenshots/pattern-gallery.png)](docs/examples/showcase/screenshots/pattern-gallery.png) | [![Real Pattern Wizard showing scatter spacing, Poisson-disk distribution, and Lloyd relaxation](docs/examples/showcase/screenshots/distribution-wizard.png)](docs/examples/showcase/screenshots/distribution-wizard.png) |

The Wizard studies every Pattern against the same built-in black-to-white
source shown here, so its small preview is easy to compare across Patterns.
Your portrait remains behind the dialog.

<a href="assets/stage20s-preset-icon-source.svg"><img src="assets/stage20s-preset-icon-source.svg" alt="Black-to-white gradient source used by the Pattern Wizard and gallery studies" width="96" /></a>

For scattered points, try **Random**, **Poisson disk**, **Jittered cells**, or
**Clustered**. Change the variation number for another repeatable arrangement;
use **Point density** to follow the artwork, **Keep points apart** for separation,
and **Relax points (Lloyd)** to smooth the distribution. These control where
points live; drawing and Advanced settings control what appears at those points.

## Gallery: one source, many constructions

These are **20 full-detail PNG exports from Toniator 0.4.5**, with no image
editing or pixel upscaling. The files are 2048×2048, 1800×1240, or 2508×2508;
the small images here link to the original exports. Each group shows its source
once. [Open the editable projects, settings, and reproduction guide](docs/examples/README.md).

### Portrait studies

**Source — original painted portrait, 1254×1254:**

<a href="docs/examples/showcase/source-portrait.png"><img src="docs/examples/showcase/source-portrait.png" alt="Source for all seven portrait studies: a continuous-tone woman with dark curls and foliage" width="260" /></a>

Source-colored marks retain the painting's palette. RGB screens and CMYK waves
turn the same image into bolder graphic textures.

| Source color · Poisson marks | Curve Motif · repeated custom curve | Clustered Connections |
| --- | --- | --- |
| [![Source-colored Poisson marks](docs/examples/showcase/portrait-source-color.png)](docs/examples/showcase/portrait-source-color.png) | [![Portrait drawn with repeated curve motifs](docs/examples/showcase/portrait-motif.png)](docs/examples/showcase/portrait-motif.png) | [![Portrait drawn with short clustered random connections](docs/examples/showcase/portrait-clustered-links.png)](docs/examples/showcase/portrait-clustered-links.png) |
| **One Guide Lines** | **Grid Voronoi** | **Three-Guide Maze** |
| [![Portrait drawn with straight line screens](docs/examples/showcase/portrait-lines.png)](docs/examples/showcase/portrait-lines.png) | [![Portrait drawn as scaled Voronoi cells on a grid](docs/examples/showcase/portrait-grid-voronoi.png)](docs/examples/showcase/portrait-grid-voronoi.png) | [![Portrait drawn as three-guide maze walls](docs/examples/showcase/portrait-three-guide-maze.png)](docs/examples/showcase/portrait-three-guide-maze.png) |
| **CMYK custom waves** | | |
| [![Portrait drawn with curved CMYK guide paths](docs/examples/showcase/portrait-waves.png)](docs/examples/showcase/portrait-waves.png) | | |

### Curves, spirals, and scattered points

**Source — included raster artwork, 1024×1024:**

<a href="assets/raster-sample.png"><img src="assets/raster-sample.png" alt="Source for all seven renders below: colored gradients and transparency around a dark center" width="260" /></a>

| Round Spiral Line | Round Spiral Marks | Square Spiral |
| --- | --- | --- |
| [![Raster source drawn as a continuous round spiral](docs/examples/showcase/raster-round-spiral-line.png)](docs/examples/showcase/raster-round-spiral-line.png) | [![Raster source drawn as marks along a round spiral](docs/examples/showcase/raster-round-spiral-marks.png)](docs/examples/showcase/raster-round-spiral-marks.png) | [![Raster source drawn as a continuous square spiral](docs/examples/showcase/raster-square-spiral.png)](docs/examples/showcase/raster-square-spiral.png) |
| **Poisson Disk Marks** | **Source-Weighted Voronoi** | **Two-Guide Maze** |
| [![Raster source drawn with an irregular evenly spaced dot arrangement](docs/examples/showcase/raster-scatter.png)](docs/examples/showcase/raster-scatter.png) | [![Raster source drawn with artwork-weighted irregular Voronoi cells](docs/examples/showcase/raster-voronoi.png)](docs/examples/showcase/raster-voronoi.png) | [![Raster source drawn as two-guide maze walls](docs/examples/showcase/raster-maze.png)](docs/examples/showcase/raster-maze.png) |
| **Fine CMYK waves** | | |
| [![Raster source drawn with a fine curved CMYK line screen](docs/examples/showcase/raster-curves.png)](docs/examples/showcase/raster-curves.png) | | |

### Vector artwork becomes new geometry

**Source — included SVG artwork, 900×620:**

<a href="assets/vector-sample.svg"><img src="assets/vector-sample.svg" alt="Source for all six renders below: a shaded blue sphere, a dark T panel, and a translucent white wave over a color gradient" width="380" /></a>

| Custom curved guides | Magazine CMYK dots | Triagrid Diamond Marks |
| --- | --- | --- |
| [![Vector source rendered with curved RGB guides](docs/examples/showcase/vector-curves.png)](docs/examples/showcase/vector-curves.png) | [![Vector source rendered with fine CMYK dot screens](docs/examples/showcase/vector-dots.png)](docs/examples/showcase/vector-dots.png) | [![Vector source rendered with diamond shapes on a triangular grid](docs/examples/showcase/vector-diamonds.png)](docs/examples/showcase/vector-diamonds.png) |
| **Three-Guide Cells** | **Two-Guide Cells** | **Triagrid Spanning Tree** |
| [![Vector source rendered as triangular guide-face regions](docs/examples/showcase/vector-three-guide-cells.png)](docs/examples/showcase/vector-three-guide-cells.png) | [![Vector source rendered as inset two-guide cells](docs/examples/showcase/vector-two-guide-cells.png)](docs/examples/showcase/vector-two-guide-cells.png) | [![Vector source rendered as branching spanning-tree connections](docs/examples/showcase/vector-spanning-tree.png)](docs/examples/showcase/vector-spanning-tree.png) |

## Mix Patterns by channel

Each channel can have its own construction and transform. This example reuses
the **SVG source displayed above**, combining straight lines, scattered circles,
and diamonds. Select a named channel before changing its Pattern or offsets.

| Mixed RGB artwork · 1800×1240 | G channel selected in the real editor |
| --- | --- |
| [![Vector artwork with different Patterns and transforms on R, G, and B](docs/examples/print-animation/vector-mixed-channels.png)](docs/examples/print-animation/vector-mixed-channels.png) | [![Green channel inspector showing its Random-family Pattern and independent transform](docs/examples/print-animation/screenshots/mixed-channel-G.png)](docs/examples/print-animation/screenshots/mixed-channel-G.png) |

| Channel | Pattern | Feature size | Rotation | X / Y offset |
| --- | --- | ---: | ---: | --- |
| R | One Guide Lines | 1.5 | 15° | +12 / −8 |
| G | Poisson Disk Marks | 0.95 | −10° | −16 / +10 |
| B | Triagrid Diamond Marks | 1.25 | 30° | +8 / +18 |

[Open this editable project](docs/examples/print-animation/vector-mixed-channels.toniator)
or [see the exact channel settings](docs/examples/print-animation/README.md#mix-patterns-and-transforms).

## Keep experimenting

**Change one channel at a time.** Select R/G/B or C/M/Y/K to vary its Pattern,
rotation, spacing, paint, or response. Return to **All** for shared edits.
**Advanced Settings…** separates **Fill response** (how much is drawn) from
**Source weighting** (where artwork-dependent points gather).

**Save the right thing.** A `.toniator` project holds artwork and editable state.
A personal **Pattern** saves a reusable construction from the Wizard.
A **Document Preset** saves the whole channel/design configuration without source
artwork: use **New options → Save preset**, then **Load preset…** on another
image. Loading a Document Preset keeps your current artwork and canvas.

**Compare before exporting.** The Source view shows the original in the same
canvas framing; it does not alter the design. Here it displays the SVG source
used by the vector gallery above:

[![Toniator Source view showing the original vector artwork](docs/examples/showcase/screenshots/source-comparison.png)](docs/examples/showcase/screenshots/source-comparison.png)

## Export your artwork

Use **Main Menu → Export image…**, choose PNG or SVG and a destination.
For PNG, select background, antialiasing, and **1× native, 2×, 4×, 8×, or Custom**
size. Export resolution is independent of preview zoom. SVG preserves editable
vector geometry and a transparent background.

[![Toniator PNG export options with a real 2x preview of the curved vector example](docs/examples/showcase/screenshots/png-export.png)](docs/examples/showcase/screenshots/png-export.png)

*PNG options for the custom curved-guides result in the vector gallery; its
original SVG source is displayed above.*

## Alpha and DTG/DTF preparation

**Source color + alpha** retains source transparency and leaves gaps between
marks. Here the same portrait has an AI-extracted transparent cut-out as its
source; Toniator creates the open Poisson-disk marks.

| Transparent source · 1254×1254 | Native transparent PNG · 2000×2000 |
| --- | --- |
| [![Transparent cut-out source for the print examples](docs/examples/print-animation/source-cutout.png)](docs/examples/print-animation/source-cutout.png) | [![Toniator source-colored portrait with genuine transparent space between marks and around the silhouette](docs/examples/print-animation/portrait-alpha.png)](docs/examples/print-animation/portrait-alpha.png) |

For DTG/DTF preparation, enable **Print preparation → Prepare for garment
printing** in PNG options. Set the physical size and DPI, then inspect alpha and
optional small-mark, narrow-feature, and gap corrections at a useful zoom.
The garment color below is **preview-only**; the exported PNG retains transparency.

[![Real PNG print-preparation dialog showing the portrait on a blue garment preview backdrop](docs/examples/print-animation/screenshots/prepared-preview.png)](docs/examples/print-animation/screenshots/prepared-preview.png)

Both close-ups below use the same **2000×2000 target, 200% zoom, and pan**.
These are native preview-region captures; click for the complete app screenshot.

| Ordinary antialiased alpha | Binary alpha + optional corrections |
| --- | --- |
| [![Native ordinary PNG preview close-up with antialiased mark edges](docs/examples/print-animation/screenshots/alpha-zoom.png)](docs/examples/print-animation/screenshots/alpha-zoom-full.png) | [![Matched prepared PNG preview close-up with corrected narrow features and gaps](docs/examples/print-animation/screenshots/prepared-zoom.png)](docs/examples/print-animation/screenshots/prepared-zoom-full.png) |

This example uses **200×200 mm at 254 DPI**, then 4 px minimum feature width,
3 px isolated-mark removal, and 2 px gap fill. These are illustrative settings
for this artwork; choose thresholds for your printing process. Preparation
thresholds alpha at 128, then applies the enabled corrections.

[Ordinary RGBA PNG](docs/examples/print-animation/portrait-alpha.png) ·
[Binary alpha only](docs/examples/print-animation/portrait-binary.png) ·
[Prepared RGBA PNG](docs/examples/print-animation/portrait-prepared.png) ·
[Editable project](docs/examples/print-animation/portrait-alpha.toniator) ·
[Settings and pixel checks](docs/examples/print-animation/README.md#alpha-and-garment-print-preparation)

## Animate the artwork

Author **Start** and **End**, choose interpolation and timing, then use
**Export video…** for a PNG sequence, lossless FFV1/Matroska, or AV1/WebM.
Still artwork, videos, animated JPEG XL, and imported image sequences can supply
the source. [Animation walkthrough](docs/USING_TONIATOR.md#animate-artwork).

### Change scale and rotation

Using the **original portrait in [Portrait studies](#portrait-studies)**, this three-second study
moves Feature size from approximately 1.538 to 0.625 and Rotation from 0° to 60°.
Toniator renders all intermediate frames with smooth-in/out interpolation.

[![Animated source-colored portrait changing mark scale and rotation](docs/examples/print-animation/portrait-motion.gif)](docs/examples/print-animation/portrait-motion.webm)

[Native 1254×1254 video](docs/examples/print-animation/portrait-motion.webm) ·
[Editable animation project](docs/examples/print-animation/portrait-motion.toniator) ·
[Source](docs/examples/showcase/source-portrait.png)

### Move channels independently

The mixed-channel **SVG artwork above** keeps its three Patterns and Feature
sizes fixed. Only each channel's Rotation and X/Y offsets animate, producing
independent motion through the image.

[![Animated vector artwork with independent R, G, and B rotations and translations](docs/examples/print-animation/vector-channel-motion.gif)](docs/examples/print-animation/vector-channel-motion.webm)

[Native 900×620 video](docs/examples/print-animation/vector-channel-motion.webm) ·
[Editable animation project](docs/examples/print-animation/vector-channel-motion.toniator) ·
[Source](assets/vector-sample.svg) ·
[Channel endpoints and reproduction](docs/examples/print-animation/README.md#animate-independent-channels)

Both native exports contain **48 frames at 16 fps**. The inline GIFs are
720-pixel-wide sharing derivatives: 24 sampled frames at approximately 8 fps,
with palette conversion. They contain no invented intermediate frames.

## More help

[Detailed usage guide](docs/USING_TONIATOR.md) ·
[Example projects and exact settings](docs/examples/README.md) ·
[Command-line rendering](docs/BUILDING.md#command-line-rendering) ·
[Known issues](ISSUES.md) · [Package guide](packaging/README.md)

Toniator is prerelease software; keep original artwork and important projects.
Developers can start with the [architecture](Project%20Specification/ArchitectureSchema.md)
and [normative Addendum](Project%20Specification/Addendum.md). The GUI and headless
CLI share the same evaluation and rendering core.

If Toniator is useful to you, an optional
[Ko-fi tip](https://ko-fi.com/silentbutdigitaldesigns) supports development.
Artwork and design products are available from
[Silent But Digital Designs](https://silentbutdigitaldesigns.com).
