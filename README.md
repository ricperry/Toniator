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

- A built-in sample loads automatically so the demo is usable immediately.
- Load PNG, JPG, WebP, or SVG input.
- SVG input is rendered to an internal canvas for sampling.
- Sample on a proportional grid using “cells on long edge”.
- When source aspect preservation is enabled, editing either Width or Height updates the other dimension from the source aspect ratio; changing dimensions changes output/sampling size without distorting the source.
- Generate real SVG output grouped by channel:
  - `halftone-cyan`
  - `halftone-magenta`
  - `halftone-yellow`
  - `halftone-black`
- Mapping modes:
  - RGB to CMYK-like values
  - luminance driving all enabled channels
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
- Tiled-width/tiled-height curve layouts connect each repeated curve copy end-to-start before rotating by the channel angle.
- Document-scale curve output is exported as filled variable-width SVG outline paths, not production `stroke-width`; halftone resolution controls the generated virtual samples along the curve edges and the perpendicular repeat spacing.
- Curve mode can import unclosed contours from Bezziator `.bezvg`/legacy `.bezziator` working files, SVG files, copied `<path>` elements, or raw SVG path `d` data.
- Connected-cell curve mode auto-connects adjacent active cells into horizontal, vertical, or diagonal chains based on channel rotation.
- Synchronized mark geometry for all channels with separate per-channel rotation/offset.
- Independent arbitrary preset/custom SVG path geometry per channel.
- Per-channel color, rotation, scale, resolution multiplier, threshold, max size/stroke, x/y offset, and opacity.
- Per-channel resolution multipliers let channels render at different grid densities, such as low-resolution K with higher-resolution CMY.
- Live preview and SVG export.

## Current limitations

- CMYK conversion is intentionally simple and browser-side; it is not color-managed.
- SVG input is rasterized for sampling. The exported halftone is vector, but it is based on sampled pixels.
- Custom SVG paths should be authored around the origin and roughly fit a `-0.5..0.5` coordinate box.
- Large cell counts can generate very large SVGs because every mark and curve connector is emitted as vector path geometry.
- Connected-cell curve chaining estimates custom curve endpoints from the first and last coordinate pair in the SVG path.
- Document-scale curve sampling uses browser SVG path measurement when available, with a Bezier-aware parser fallback for non-browser tests.
- The preview background toggle is for visual comparison only; exported SVGs do not embed the source raster/SVG.

## Code layout

- `src/imageLoader.js` — raster/SVG file loading.
- `src/sampling.js` — proportional grid calculation and canvas sampling.
- `src/color.js` — RGB to CMYK-like conversion and luminance mapping.
- `src/presets.js` — built-in curve and shape path presets.
- `src/svgGenerator.js` — channel grouping and SVG mark generation.
- `src/main.js` — UI state, live rendering, and export.
