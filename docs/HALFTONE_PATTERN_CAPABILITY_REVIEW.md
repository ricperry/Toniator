# Halftone Pattern capability review

This planning review compares Toniator 0.4.7 with TheVectorLab’s Knockout Halftones and DTF-TONES on 2026-11-18. Build a shared Shape Library first, so artists choose or author marks within existing Patterns. Selective shirt-color removal remains the largest new capability.

## Current capability

Toniator has 16 structural Patterns and 14 source-free Document Presets, including Skinny Wave, Magazine CMYK and Moire Grayscale. Start through **Change… → Customize**, choose **Guides**, **Scatter** or **Spiral curve**, then the applicable **Shapes**, **Lines** or **Areas** drawing. Advanced Settings owns tone response and appearance. Every bundled starter must be rebuildable through exposed controls. [Guide](USING_TONIATOR.md#patterns-and-the-wizard), [registry](../crates/toniator-patterns/src/lib.rs#L911), [Addendum §8](../Project%20Specification/Addendum.md#8-presets-must-use-exposed-controls-only).

Each channel Pattern has exactly one drawing. Combining independent marks and strokes requires separate channels or external composition. Historic mixed-output types do not establish current capability. [Validator](../crates/toniator-domain/src/lib.rs#L16149).

One-ink output already works. The released CLI rendered disposable projects containing the original raster and SVG samples. Both used a density of 50, one visible solid-black RGB channel, inverted Luminance fill, transparent PNG backing and antialiasing off. The raster-source PNG has 741,962 opaque-black and 306,614 transparent pixels. The SVG-source PNG has 418,126 opaque-black and 139,874 transparent pixels. Those are the only two pixel values in either file. Both projects also exported as SVG. The digital test establishes the ink and transparency result. Matching the openings in deep shadows remains a separate visual test. [Raster-source PNG](../target/validation/pattern-capability-review-20261118/raster-one-ink.png) · [SVG-source PNG](../target/validation/pattern-capability-review-20261118/vector-one-ink.png)

## Competitor reference

Knockout Halftones advertises 21 options with levels, scale and angle, producing one-ink binary transparency. Its tutorial illustration shows grids, angled grids, grid dots, mesh waves, dot mesh waves, dot waves and rotated wave lines. These examples do not name all 21 options. [Product](https://thevectorlab.com/products/knockout-halftones), [illustration](https://cdn.shopify.com/s/files/1/0151/1081/files/Knockout-Halftones6.jpg?v=1787793003).

DTF-TONES templates target Photoshop/Affinity at 25, 35 and 45 LPI. Web offers seven screens, 15–65 LPI, angle, arbitrary garment key and reach, edge preservation, extra-color tolerance, tiny-dot handling, ink restoration in fades and erasing. It selectively halftones near-garment regions while retaining far-color original image regions. Transparent output and independent garment preview support production. [Templates](https://thevectorlab.com/products/dtf-tones), [Web](https://thevectorlab.com/pages/dtf-tones-web), [vendor comparison](https://thevectorlab.com/blogs/resources/choosing-a-halftone-tool-for-dtf-dtg-and-screen-printing).

ColorPop offers adjacent inspiration for a curated shelf, with ten adjustable screens, forty palettes and darkest/lightest controls. [Product](https://thevectorlab.com/products/colorpop-halftones).

## Shared Shape Library and structural examples

Under **Customize → Drawing → Shapes**, show the current mark and a thumbnail picker with **Built-in** and **My Shapes**. Start with Circle, Oval, Square, Triangle and Star. An artist could choose a star for a grid, draw a narrower version in **Edit Shape…**, save it to **My Shapes**, and then pick it for a spiral. **New Shape…** opens the existing closed-path editor. The same picker belongs on every Pattern that draws marks. [Fine CMYK waves](examples/showcase/raster-curves.png) · [Triagrid Diamond Marks](examples/showcase/vector-diamonds.png) · [Portrait with Curve Motif](examples/showcase/portrait-motif.png)

| Shape or structural study | Construction and useful adjustments |
| --- | --- |
| Round, ellipse, square, diamond | Shared library choices within the existing arrangement. Preserve typed Circle and its analytic path. Store general silhouettes as authored closed paths, including an ellipse or four-sided square. Independent Shape rotation turns that square into a diamond. |
| Line and angled grid | One or two straight guide directions drawn as Lines. Adjust guide angle, spacing and thickness response. Grid dots instead draw Shapes at intersections. |
| Wavy lines and wavy dots | Edit one curved guide direction. Draw Lines for continuous waves or Circle Shapes at along-guide sites for dots. Adjust point spacing and the actual curve. |
| Mesh wave and dot mesh wave | Edit two curved guide directions. Draw paths for mesh or Shapes at their intersections for dot mesh. Adjust crossing angle and curve geometry. |
| Chevron and scallop | Curve Motif repeats one open segment along one guide direction. Edit that segment, then use transforms, Mirror alternate rows and alternate-row offset. |
| Irregular stamps | Choose a library shape or author a closed path within Scatter. Adjust point spacing, repeatable variation and placement response. The silhouette is reusable in other compatible arrangements. |

Only one or two curved guide directions are editable. Test finite-curve coverage and continuity across Feature size and rotation. Dots follow real curved guides, not painted Curve Motifs. [Along-guide implementation](../crates/toniator-patterns/src/lib.rs#L8640), [intersections](../crates/toniator-patterns/src/lib.rs#L8336), [current motif controls](../.codex-work/evidence/ui-run-20261118-124607-485541/03-motif-drawing.png).

## Wizard and print workflow

Shape selection replaces the mark resource, preserving placement settings and family-compatible orientation/response intent. A simple unchanged grid should retain its centers, while footprint-dependent visible-margin exclusion can change generated sites. Use existing recalibration when required, without promising identical tone. Built-ins are immutable. Edit a document-local copy by default, optionally **Save to My Shapes**, and explicitly update a personal master separately. Applying and saving remain distinct. Embed geometry in projects and reusable Patterns for portability and independence from master changes. Follow accepted resource ownership semantics. [Stage 21B resource semantics](STAGE_21B_PATTERN_WIZARD_AND_PERSONAL_LIBRARY_PLAN.md#gate-21b-1--storage-and-registry-foundation).

Guides, Scatter and Spiral already accept authored closed marks. Guides can orient a mark to a contributing guide where supported. Scatter and Spiral use fixed mark orientation and still offer Shape rotation. The anticipated implementation is the picker, bundled shape data and personal library actions. Verify selection, orientation and tone in all three families through their normal Wizard routes. [Spiral mark construction](../crates/toniator-domain/src/lib.rs#L5632) · [Spiral mark validation](../crates/toniator-domain/src/lib.rs#L16703)

Current insertion makes document copies and IO supports writing resources. Direct shape browsing and GUI save actions remain proposed, alongside separate **Save Motif…** for open motifs. [Resource insertion](../crates/toniator-app/src/main.rs#L22062) · [Personal resource writer](../crates/toniator-io/src/personal_library.rs#L293)

Offer applicable quick tweaks such as spacing and angle, point spacing on dotted waves, and mirror/row offset on motifs. An optional artwork crop can accompany the neutral gradient. Keep full construction editing under **Customize** and source weighting, calibration and response detail in Advanced. Use a **Document Preset** for one-ink channels and levels. Settings and exports remain independent, with transparent backing and edge settings chosen at PNG export. A shape or single-ink Preset cannot set export options.

Show LPI only for periodic construction with known print scale. Print preparation continues to own physical dimensions, DPI, binary alpha, corrections and garment preview. [Print preparation](USING_TONIATOR.md#print-preparation).

Garment knockout introduces three decisions.

- Selective knockout needs a way to measure how closely each source color matches the chosen shirt. Preview the affected area and let the artist control how far the fade reaches.
- Solid areas outside that selection should keep their original color and detail. Inside the fade, ink restoration should control the surviving dot color separately from its coverage. Decide how preserved source artwork is represented for SVG, or explicitly scope the first version to prepared PNG.
- An eraser would need an editable, undoable local mask. The print workflow can be tested before adding that tool.

The shared document model should own reusable knockout intent. Existing canonical geometry can continue to drive ordinary Pattern preview and export.

## What to test first

Test one shape across grid, scatter and spiral, then different shapes in the same arrangement. Verify immutable built-ins, New, local-copy edits, explicit personal save/update, independent Apply/Cancel, undo, preserved compatible intent, recalibration and portable reopen after library changes.

Then test a dotted wave, a curved mesh sampled at crossings and a mirrored scallop at two scales. Rotation and coarseness stay adjustments on the same construction. Failing edge continuity or prematurely closed shadow holes should drive a specific engine decision.

Use both immutable raster and vector inputs, tone ramps and colored-garment witnesses. Test periodic screens at 300 DPI and 25/35/45 LPI. Compare highlights, shadows, holes, edges, tiny marks and wave/mesh continuity. Include equal-luminance different hues, near-key/far-key colors and fade restoration.

Inspect native RGBA with RGB and alpha separately, editable SVG geometry and production PNG. Verify reconstruction, save/reopen, undo, keyboard and accessible readback. Existing private Sway evidence supports current UI observations. Proposed visual quality, GNOME/Mutter, human workflow and physical printing still require acceptance.
