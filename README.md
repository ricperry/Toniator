# Toniator vector halftone POC

This is a dependency-free browser demo for testing vector halftone generation before extracting the core logic into an Inkscape extension.

## Run

Install dependencies:

```bash
npm install
```

Start the Vite dev server:

```bash
npm run dev
```

Then open <http://localhost:5173>.

For a production build:

```bash
npm run build
```

Run the generator regression sweep:

```bash
npm test
```

To preview the production build locally:

```bash
npm run preview
```

## What works

- A root-level `built-in-sample.svg` loads automatically so the demo is usable immediately. Replace that file to change the default sample; if it is missing or cannot load, Toniator falls back to an embedded copy.
- Load PNG, JPG, WebP, or SVG input.
- SVG input is rendered to an internal canvas for sampling and preview comparison so the displayed source and sampled source share the same raster basis.
- Sample on a proportional grid using “cells on long edge”.
- When source aspect preservation is enabled, editing either Width or Height updates the other dimension from the source aspect ratio; changing dimensions changes output/sampling size without distorting the source.
- Generate real SVG output grouped by channel:
  - `toniator-cyan`
  - `toniator-magenta`
  - `toniator-yellow`
  - `toniator-black`
  - each channel group is marked as an Inkscape layer with a readable channel label
- Mapping modes:
  - RGB to CMYK-like values
  - luminance driving all enabled channels
  - luminance split across enabled channels for monochrome curve-mode crosshatching; one enabled channel receives the full grayscale value, while multiple enabled channels divide the density budget across hatch directions
  - inverted luminance driving all enabled channels
  - grayscale into one selected channel
- Shape mode with preset or custom SVG path marks.
- Shape presets are intentionally limited to circle, rectangle/square, triangle, pentagon, and hexagon.
- Shape-mode X/Y offsets wrap modulo the current grid cell width/height as halftone screen phase values; mark placement and image sampling move together.
- Curve mode with preset or custom SVG path centerlines.
- Curve mode supports an explicit active edit target. In synchronized mode, the shared base curve is editable and channel cards are ghosted previews. In independent mode, each CMYK channel has its own Edit button and only one channel accepts node/handle edits at a time.
- Active curve editors include visible anchors, handles, handle lines, selected node/handle state, a node-type selector, and scoped keyboard handling for select/deselect, delete/collapse, escape, and node-type cycling.
- Curve endpoint behavior is explicit per active target: open curves, connected curves with smooth seam tangents, and connected curves with intentionally broken seam tangents.
- Full-width/full-height curve layouts scale the user-defined curve from its start point to its end point across the selected document dimension.
- Document-scale curve output is exported as filled variable-width SVG outline paths, not production `stroke-width`; halftone resolution controls the generated virtual samples along the curve edges and the perpendicular repeat spacing.
- Motif-pattern curve layout supports auto artboard coverage. Each row is chained by placing every motif start point on the previous motif end point, so rows are continuous by endpoint geometry rather than by an arbitrary spacing value. Auto mode computes tile and stack counts from motif endpoint advance, row spacing, row angle offsets, grid rotation, output size, and bleed; manual mode keeps explicit tile/stack counts. A zero row angle offset stacks rows perpendicular to the along-curve direction.
- Motif rows use a powerstroke-style filled SVG outline. Curve Output Quality controls row resampling density, smoothing variable-width transitions across chained motif segments without changing the number of exported centerline-row paths.
- Curve mode can import unclosed contours from Bezziator `.bezvg`/legacy `.bezziator` working files, SVG files, copied `<path>` elements, or raw SVG path `d` data.
- Saved `.tntr` preset files can be placed in the root-level `presets/` folder to populate the Presets menu. Selecting a preset applies it immediately; if current settings have unsaved changes, Toniator prompts to save or discard those changes first.
- Synchronized mark geometry for all channels with separate per-channel rotation/offset.
- Independent arbitrary preset/custom SVG path geometry per channel.
- Combined CMYK controls can adjust shared render-time deltas and multipliers across enabled channels. In motif-pattern curve layout, the combined panel also includes coverage mode and alternate tile transform overrides so broad density, tiling, and stacking choices can be dialed in before per-channel cleanup.
- Channel settings hide controls that do not affect the active mode/layout, including disabled-channel controls, shape-only/curve-only controls, motif-only curve tiling/stacking controls, unused grid-pivot controls, and channel color in monochrome crosshatch mode.
- Per-channel color, rotation, scale, resolution multiplier, threshold, max size/stroke, x/y offset, and opacity.
- Per-channel resolution multipliers let channels render at different grid densities, such as low-resolution K with higher-resolution CMY.
- Live preview, SVG export, and PNG export.
- SVG and PNG exports prompt for a filename before download.
- PNG export prompts for output size. Users can enter DPI or pixel width/height; DPI derives pixels from embedded source physical size when available, or from source pixel dimensions treated as 96 DPI when no physical size metadata is available. Exported PNGs include DPI metadata.
- PNG export can preserve transparency with an alpha channel, or export each enabled CMYK channel as its own PNG for post-processing, recoloring, or re-vectorizing elsewhere.

## Current limitations

- CMYK conversion is intentionally simple and browser-side; it is not color-managed.
- SVG input is rasterized for sampling and preview comparison. The exported halftone SVG is vector, but it is based on sampled pixels.
- Custom SVG paths should be authored around the origin and roughly fit a `-0.5..0.5` coordinate box.
- Large cell counts can still generate large SVGs because every mark is vector geometry, but curve output clips off-artboard row geometry and simplifies redundant variable-width samples before export.
- Document-scale curve sampling uses browser SVG path measurement when available, with a Bezier-aware parser fallback for non-browser tests.
- The preview background toggle is for visual comparison only; exported SVGs do not embed the source raster/SVG.
- Embedded physical-size metadata detection is limited to SVG length units, PNG `pHYs`, and JPEG JFIF density. Other raster metadata falls back to 96 DPI.
- The Presets menu is populated from files bundled or served from `presets/`. In a production build, add preset files before running `npm run build`; during development, refresh the app after moving a saved `.tntr` into that folder.

## Code layout

- `src/imageLoader.js` — raster/SVG file loading, SVG rasterization, and physical-size metadata extraction.
- `built-in-sample.svg` — replaceable default sample loaded by the app on startup and by the "Load built-in sample" button, with an embedded fallback in `src/imageLoader.js`.
- `presets/` — `.tntr` JSON preset files shown in the Presets menu.
- `src/sampling.js` — proportional grid calculation and canvas sampling.
- `src/color.js` — RGB to CMYK-like conversion and luminance mapping.
- `src/presets.js` — built-in curve and shape path presets.
- `src/svgGenerator.js` — channel grouping and SVG mark generation.
- `src/main.js` — UI state, live rendering, mode-specific control visibility, and export.
