# Gallery projects and reproduction

The [repository README](../../README.md) shows 20 actual Toniator 0.4.5 PNG
gallery exports and five original workflow screenshots, plus
[alpha, print preparation, mixed channels, and two animations](print-animation/README.md). The files in
[showcase/](showcase/) are the full-size originals, not thumbnail exports.

## Open an example

Download or clone this repository and open a file from
[showcase/recipes/](showcase/recipes/) in Toniator 0.4.5. Each `.toniator` project
embeds its source artwork and complete editable configuration, including saved
calibration. You can inspect it, change its settings, or load its configuration
onto your own artwork with **New options → Load preset…**.

These projects use current document schema 17/container 2. Use the matching
[0.4.5 release](https://github.com/ricperry/Toniator/releases/tag/v0.4.5) or its
source checkpoint `b49470bcbbfed5cd7aa218e21acf1028220eb8dc` for exact reproduction.
The standalone Pattern library and source-free Document Presets are different
formats; the supplied examples are ordinary source-backed projects.

## Sources and resolution

- [Raster sample](../../assets/raster-sample.png): immutable 1024×1024 RGBA input;
  seven PNG outputs are 2048×2048.
- [Vector sample](../../assets/vector-sample.svg): immutable 900×620 SVG input;
  six PNG outputs are 1800×1240. Its live text uses installed fonts, so exact
  glyph pixels can vary between systems.
- [Painted portrait](showcase/source-portrait.png): original AI-generated
  1254×1254 RGB source; seven PNG outputs are 2508×2508.
- [Neutral tone study](../../assets/stage20s-preset-icon-source.svg): the app's
  actual built-in 100×100 gradient source for Wizard studies/gallery thumbnails.

All final PNGs are rendered directly at **2× the project's canvas size** through
Toniator's canonical scene and ordinary streaming PNG exporter. No PNG was
upscaled, flattened, recolored, sharpened, or otherwise edited afterward. The RGB
examples use automatic black backing, CMYK uses white, and Source color + alpha
uses transparent backing while preserving native alpha. The opaque portrait and
its covering marks produce an opaque example here, as the manifest records.
All use ordinary antialiasing, with print preparation off.

## Settings and editable recipes

Each link opens the exact project used for its PNG. Base density is the stored
CLI/domain value; the app shows its inverse as Feature size. Named-channel
rotation/density deltas and all other settings are preserved inside the project;
the [manifest](showcase/manifest.json) records models, canvas/output dimensions,
channel layouts, provenance, SHA-256 values, and alpha statistics. These are
purposeful artistic examples, not color-fidelity or print-safety benchmarks.

| Example / full PNG | Editable project | Model | Base density | Output |
| --- | --- | --- | ---: | --- |
| [Fine CMYK waves](showcase/raster-curves.png) | [raster-curves.toniator](showcase/recipes/raster-curves.toniator) | CMYK | 65 | 2048×2048 |
| [Source-Weighted Voronoi](showcase/raster-voronoi.png) | [raster-voronoi.toniator](showcase/recipes/raster-voronoi.toniator) | RGB | 60 | 2048×2048 |
| [Two-Guide Maze](showcase/raster-maze.png) | [raster-maze.toniator](showcase/recipes/raster-maze.toniator) | RGB | 65 | 2048×2048 |
| [Poisson Disk Marks](showcase/raster-scatter.png) | [raster-scatter.toniator](showcase/recipes/raster-scatter.toniator) | RGB | 60 | 2048×2048 |
| [Custom curved guides](showcase/vector-curves.png) | [vector-curves.toniator](showcase/recipes/vector-curves.toniator) | RGB | 65 | 1800×1240 |
| [Magazine CMYK dots](showcase/vector-dots.png) | [vector-dots.toniator](showcase/recipes/vector-dots.toniator) | CMYK | 65 | 1800×1240 |
| [Round Spiral Line](showcase/raster-round-spiral-line.png) | [raster-round-spiral-line.toniator](showcase/recipes/raster-round-spiral-line.toniator) | RGB | 65 | 2048×2048 |
| [Round Spiral Marks](showcase/raster-round-spiral-marks.png) | [raster-round-spiral-marks.toniator](showcase/recipes/raster-round-spiral-marks.toniator) | RGB | 60 | 2048×2048 |
| [Square Spiral](showcase/raster-square-spiral.png) | [raster-square-spiral.toniator](showcase/recipes/raster-square-spiral.toniator) | RGB | 65 | 2048×2048 |
| [Three-Guide Cells](showcase/vector-three-guide-cells.png) | [vector-three-guide-cells.toniator](showcase/recipes/vector-three-guide-cells.toniator) | RGB | 55 | 1800×1240 |
| [Triagrid Diamond Marks](showcase/vector-diamonds.png) | [vector-diamonds.toniator](showcase/recipes/vector-diamonds.toniator) | RGB | 50 | 1800×1240 |
| [Triagrid Spanning Tree](showcase/vector-spanning-tree.png) | [vector-spanning-tree.toniator](showcase/recipes/vector-spanning-tree.toniator) | RGB | 45 | 1800×1240 |
| [Two-Guide Cells](showcase/vector-two-guide-cells.png) | [vector-two-guide-cells.toniator](showcase/recipes/vector-two-guide-cells.toniator) | RGB | 55 | 1800×1240 |
| [Curve Motif](showcase/portrait-motif.png) | [portrait-motif.toniator](showcase/recipes/portrait-motif.toniator) | RGB | 75 | 2508×2508 |
| [Clustered Connections](showcase/portrait-clustered-links.png) | [portrait-clustered-links.toniator](showcase/recipes/portrait-clustered-links.toniator) | RGB | 90 | 2508×2508 |
| [One Guide Lines](showcase/portrait-lines.png) | [portrait-lines.toniator](showcase/recipes/portrait-lines.toniator) | RGB | 80 | 2508×2508 |
| [Grid Voronoi](showcase/portrait-grid-voronoi.png) | [portrait-grid-voronoi.toniator](showcase/recipes/portrait-grid-voronoi.toniator) | RGB | 70 | 2508×2508 |
| [Three-Guide Maze](showcase/portrait-three-guide-maze.png) | [portrait-three-guide-maze.toniator](showcase/recipes/portrait-three-guide-maze.toniator) | RGB | 75 | 2508×2508 |
| [CMYK custom waves](showcase/portrait-waves.png) | [portrait-waves.toniator](showcase/recipes/portrait-waves.toniator) | CMYK | 50 | 2508×2508 |
| [Source color Poisson marks](showcase/portrait-source-color.png) | [portrait-source-color.toniator](showcase/recipes/portrait-source-color.toniator) | Source color + alpha | 90 | 2508×2508 |

The collection covers fifteen built-in Pattern constructions plus a custom
continuous square spiral and curved-guide document looks: circular marks,
straight lines, custom curve motifs, Poisson-disk scatter, clustered random
connections, grid and source-weighted Voronoi, round spiral lines, round
spiral marks, triangular diamond marks, two-/three-guide cells, two-/three-guide
mazes, and triangular spanning trees. Custom curved guides and CMYK waves come
from the shipped source-free document configuration collection. RGB portrait
channel rotations are 30°/60°/0°; their separated-color appearance is intentional.
The source-color portrait uses a single sampled-color channel.

## Reproduce in the GUI

1. Open the linked `.toniator` project.
2. Choose **Main Menu → Export image…**, select PNG, and choose a new destination.
3. In **PNG export options**, keep **Background: Automatic**, **Antialiasing: On**,
   and print preparation off. Select **Output size: 2×**.
4. Choose **Export PNG…**. The canvas remains unchanged; final geometry is
   rasterized directly at the requested output dimensions.

The [export screenshot](showcase/screenshots/png-export.png) demonstrates this
exact operation for `vector-curves.toniator`. Its GUI PNG and the helper's PNG
match byte-for-byte at 1800×1240 in this run.

## Reproduce from source

The normal CLI renders a project's stored native canvas dimensions:

```bash
mkdir -p target/validation/readme-reproduction
./target/release/toniator render \
  -i docs/examples/showcase/recipes/vector-curves.toniator \
  -o target/validation/readme-reproduction/vector-curves-native.png
```

To reproduce the gallery's 2× output exactly, this small documentation helper
uses the same shared engine/streaming PNG API as the GUI. It never changes the
project, source, calibration, or geometry and refuses an existing destination.
Build requirements are those of the [headless CLI](../BUILDING.md); GTK is not
required. From the matching source checkout:

```bash
cargo build --release --locked \
  --manifest-path docs/examples/renderer/Cargo.toml --target-dir target
mkdir -p target/validation/readme-reproduction
./target/release/toniator-showcase-renderer \
  docs/examples/showcase/recipes/vector-curves.toniator \
  target/validation/readme-reproduction/vector-curves.png
```

Reproduce all twenty exports into an empty output directory:

```bash
for name in $(python3 -c 'import json; print(" ".join(r["name"] for r in json.load(open("docs/examples/showcase/manifest.json"))["renders"]))'); do
  project="docs/examples/showcase/recipes/$name.toniator"
  ./target/release/toniator-showcase-renderer "$project" \
    "target/validation/readme-reproduction/$name.png"
done
```

The helper preserves the repository's patched geometry dependencies through its
own manifest, avoiding a different polygon renderer. It was built successfully
and all twenty final projects were evaluated through it. To export editable
geometry instead, use the ordinary CLI:

```bash
./target/release/toniator render \
  -i docs/examples/showcase/recipes/vector-curves.toniator \
  -o target/validation/readme-reproduction/vector-curves.svg
```

[The corresponding native editable SVG](showcase/vector-curves.svg) is included.
It contains vector paths, without an embedded raster image.

## Screenshot and output provenance

Captured 2026-10-18 from the actual Linux 0.4.5 release app at source checkpoint
`b49470bcbbfed5cd7aa218e21acf1028220eb8dc`:

| Screenshot | Running state | Source |
| --- | --- | --- |
| [Editor](showcase/screenshots/editor.png) | Source-colored Poisson portrait, inspector visible | Painted portrait |
| [Pattern Gallery](showcase/screenshots/pattern-gallery.png) | Poisson Disk Marks selected | Portrait behind dialog; neutral gradient study |
| [Distribution Wizard](showcase/screenshots/distribution-wizard.png) | Scatter family options | Portrait behind dialog; neutral gradient study |
| [Source comparison](showcase/screenshots/source-comparison.png) | Source view of curved-guide vector project | Immutable SVG sample |
| [PNG export](showcase/screenshots/png-export.png) | Automatic backing, AA On, 2× output | Immutable SVG sample |

Screenshots are unedited 1440×1000 captures from an isolated headless Sway
Wayland session, using GTK OpenGL presentation. Actual runtime manifests verify
four disposable XDG profile roots, private Wayland/D-Bus/AT-SPI buses, and socket
identity. No personal desktop/browser session is controlled. These are real app
screenshots; this automated environment is not GNOME/Mutter, portal, human
keyboard/screen-reader, or physical-print acceptance.

App SHA-256: `50de886d571ec193fab057b80750dc5fd81db50ffb512cb4ab0ba60dcac242a8`.
CLI SHA-256: `c5bba9171b8d4341901e0f21f4a84f0adfb1fbd121c508420be68f99ea81e0fd`.
2× helper SHA-256: `4dd6aa1066bfcb84007d6f35f59cebe3f92687db641e863486fff108e17134a5`.

All final exports use the pinned 0.4.5 binaries identified above.
Local raw logs, runtime manifests, preliminary studies, GUI/helper PNG
comparison, and shutdown evidence are retained under
`target/validation/readme-showcase-20261018/` (not required repository assets).
The older 0.2.0 `rgb-dots.png`, `cmyk-dots.png`, and `vector-dots.svg` remain as
historical files and are not used by the current README. The superseded
`raster-square-spiral-marks` project and PNG are also retained; the selected
gallery now uses a continuous square spiral.

## Portrait source provenance

The fictional portrait was generated specifically as continuous-tone source art
using OpenAI ImageGen, then copied byte-for-byte into this repository. The image
generator produced only the source painting. Toniator produced every effect;
no UI image was generated. Source SHA-256 is recorded in the manifest.

<details>
<summary>Exact source generation prompt</summary>

```text
Use case: illustration-story
Asset type: original source artwork for a creative desktop application's image-to-pattern README demonstrations.
Primary request: A beautiful original square continuous-tone painted portrait of a fictional adult woman in three-quarter view, with expressive dark curly hair and a calm face, warmly lit against a softly graded botanical backdrop.
Style/medium: polished editorial digital painting with natural facial proportions and flowing sculptural forms; rich smooth tonal modeling rather than flat poster colors.
Composition/framing: square chest-up portrait, face large and clearly readable at thumbnail size, dark curls and broad leaf shapes framing the head, visually balanced.
Lighting/mood: luminous warm side light and cool shaded contours, deep darks through clear middle values to cream highlights.
Color palette: warm copper and terracotta skin highlights, deep indigo shadows, teal and sage foliage, restrained apricot background.
Constraints: opaque full-bleed artwork; no text, logo, border, signature, watermark or interface; no halftone dots, line-screen, mosaic, stippling or printed texture because Toniator will produce these effects from this source. Make this a clean original image, not a screenshot. Render with abundant fine detail and smooth gradients at the highest available square resolution.
```

</details>
