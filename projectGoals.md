
I want to build a proof-of-concept vector halftone generator, with the long-term goal of turning it into an Inkscape extension.

The first version should be a standalone browser-based demo so the core idea can be tested visually and iterated quickly. Keep the architecture modular so the halftone logic can later be adapted for an Inkscape plugin or extension.

Project goal:

Create a tool that accepts either:

1. A raster image input, such as PNG or JPG.
2. An SVG file input.

For proof-of-concept purposes, SVG input may be rendered internally to a raster canvas for sampling, but the final halftone output should be vector SVG.

The tool should generate a vector halftone where each CMYK channel can use its own curve or shape as the halftone mark. The generated output should be previewed live and exportable as SVG.

Core behavior:

The input image should be sampled at a user-defined halftone resolution that is independent of the input image resolution. The sampling resolution should remain proportional to the input aspect ratio. For example, the user should be able to define an output width, output height, sample density, or number of cells across the long edge, and the other dimension should be calculated proportionally.

Convert the sampled image data into CMYK-like channel values. Since browser image data is RGB, use a reasonable RGB-to-CMYK conversion for the first version. Also include a luminosity-based mode where the same luminance value can drive all enabled channels, because I may want monochrome or stylized effects.

For each sampled cell and each enabled CMYK channel, generate a vector mark whose size, stroke thickness, radius, or scale is controlled by that channel’s mapped value. Darker or stronger channel values should produce larger/thicker marks. Very small values should be omitted based on a user-adjustable threshold.

Halftone mark modes:

Support two main mark modes:

1. Curve mode:

   * A halftone mark is based on an SVG path used as a centerline curve.
   * The channel value controls the stroke width or “thickness” of the curve.
   * Curves should use round caps and round joins by default.
   * The user should be able to use one shared curve for all channels, with separate per-channel rotations.
   * The user should also be able to define independent curves for C, M, Y, and K.
2. Shape mode:

   * A halftone mark is based on a closed SVG path or simple preset shape.
   * The channel value controls the radius, scale, or size of the shape.
   * The user should be able to use one shared shape for all channels, with separate per-channel rotations.
   * The user should also be able to define independent shapes for C, M, Y, and K.

The shape or curve definition UI does not need to be a full drawing editor in the first version. For proof of concept, provide practical ways to define marks, such as:

* Built-in presets: circle, ellipse, line, slash, diamond, square, triangle, arc, wave, short curve, cross, etc.
* Text areas where I can paste SVG path `d` values for custom curves or shapes.
* A small live preview of the selected mark for each channel.
* A toggle for “shared base mark with per-channel rotations” versus “independent mark per channel.”

Channel controls:

Provide separate controls for Cyan, Magenta, Yellow, and Black:

* Enable/disable channel.
* Channel color used in the preview and SVG output.
* Rotation angle.
* Scale multiplier.
* Minimum threshold.
* Maximum size or stroke width.
* Optional x/y offset.
* Optional opacity.
* Curve/shape selector or custom SVG path input.

Also provide global controls:

* Raster/SVG file input.
* Sampling resolution.
* Output SVG width/height.
* Preserve aspect ratio.
* Value mapping mode: CMYK, luminance, inverted luminance, or single-channel grayscale.
* Minimum and maximum mark size.
* Grid spacing.
* Background preview toggle.
* Preview quality setting if performance becomes an issue.
* Export SVG button.

Output requirements:

The generated result should be real SVG vector geometry, not a raster image embedded inside an SVG.

For curve mode, generate SVG paths with stroke width controlled by the sampled value.

For shape mode, generate transformed SVG paths or simple SVG primitives scaled according to the sampled value.

Group output by channel using SVG `<g>` elements, with clear IDs such as `halftone-cyan`, `halftone-magenta`, `halftone-yellow`, and `halftone-black`.

The exported SVG should preserve the output artboard size, viewBox, and proportional layout.

Implementation direction:

Start with a minimal but functional proof of concept:

1. Load an image file.
2. Draw it to an internal sampling canvas.
3. Sample it on a proportional grid.
4. Convert samples to CMYK-like values.
5. Generate SVG vector halftone marks.
6. Preview the SVG.
7. Export the SVG.

After that works, add:

1. SVG input support by rendering the SVG to canvas for sampling.
2. Curve mode.
3. Shape mode.
4. Shared mark with per-channel rotations.
5. Independent per-channel marks.
6. Custom SVG path input.
7. Presets.
8. Better UI organization.
9. Performance improvements if needed.

Keep the code readable, modular, and well-commented. Avoid over-engineering. I want a working proof of concept first, then we can iterate.

Use clear function boundaries for:

* Loading raster input.
* Loading SVG input.
* Sampling the image.
* RGB-to-CMYK conversion.
* Luminance calculation.
* Value mapping.
* Mark generation.
* SVG generation.
* Preview rendering.
* Export/download.

Also include a short README explaining how to run the prototype and what the current limitations are.

Important design intent:

This is not meant to produce ordinary circular halftone dots only. The main creative goal is to use curves or custom shapes as the repeating halftone marks, with each CMYK channel able to have its own curve/shape or at least its own rotation. The tool should let me get multiple visual looks by changing rotations, scale, thresholds, and channel settings without having to completely redraw the halftone marks every time.
