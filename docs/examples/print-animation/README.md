# Alpha, garment preparation, channels, and animation

These are genuine Toniator 0.4.5 outputs and actual application captures. Open
the linked `.toniator` files to continue editing. They embed their sources and
use document schema 17/container 2. The matching source checkpoint is
`b49470bcbbfed5cd7aa218e21acf1028220eb8dc`.

The [manifest](manifest.json) records every delivered asset's SHA-256, dimensions,
alpha statistics, source/project association, and animation metadata.

## Alpha and garment print preparation

Source: [transparent portrait cut-out](source-cutout.png), 1254×1254 RGBA.
It is an AI background-extraction edit of the
[original painted portrait](../showcase/source-portrait.png). The generated
source's fractional alpha is preserved, including hair edges and nearly opaque
interior pixels. Toniator generates the pattern, not the source generator.

[portrait-alpha.toniator](portrait-alpha.toniator) uses **Source color + alpha**,
Poisson-disk circular marks, base density 90 (Feature size approximately 1.111),
and Variable marks with maximum fill 0.5. This deliberately leaves visible gaps
and retains the cut-out silhouette. The raw exports have no blue background.

### Reproduce the three PNGs

1. Open the project and choose **Main Menu → Export image…**, PNG, and a destination.
2. For [ordinary alpha](portrait-alpha.png), leave **Prepare for garment printing**
   off. Set **Output size: Custom**, dimensions **2000×2000**, **Antialiasing: On**,
   and **Background: Automatic**. This model's automatic backing is transparent.
3. For [binary alpha only](portrait-binary.png), enable **Print preparation →
   Prepare for garment printing**. Choose **Millimetres (mm)**, maximum width
   **200**, maximum height **200**, and **254 DPI**. Set all three corrections to
   **0**. The calculated result is also 2000×2000.
4. For the [corrected preparation](portrait-prepared.png), retain that physical
   size/DPI. Set **Minimum feature width (px): 4**, **Remove isolated below (px):
   3**, **Minimum gap width (px): 2**, and **Gap strategy: Fill (average surrounding
   edges)**. Export to a separate destination.

These thresholds illustrate this artwork, not a recommended printing minimum.
Choose them for your printer, transfer/ink, garment, and production process.
Preparation alone does not certify physical print safety. Correction controls
accept whole pixels; zero disables a correction, and isolated-mark removal must
not exceed minimum feature width. At 254 DPI each pixel represents 0.1 mm.

### Inspect the changes

Use **Preview backdrop: Garment**, color **#253C6D**, to inspect the design on a
blue garment swatch. This affects only the viewer. The [ordinary fit capture](screenshots/alpha-preview.png)
and [prepared fit capture](screenshots/prepared-preview.png) show the actual app.
Wait for **Current PNG preview** before comparing.

The matched close-ups use **Zoom in** once (200% of export pixels), then horizontal
scroll value **1500** and vertical **1300**. Both preview pictures are 4000×4000,
inside the same 645×525 viewport, at the same location. Full captures:
[ordinary](screenshots/alpha-zoom-full.png), [prepared](screenshots/prepared-zoom-full.png).
Their [ordinary region](screenshots/alpha-zoom.png) and
[prepared region](screenshots/prepared-zoom.png) are direct `grim` captures of
that native viewport, located through private AT-SPI bounds plus the private
Sway window position. No resizing, flattening, or repainting is applied.

Exact alpha checks on the original 2000×2000 files:

| Output | Alpha range | Fractional-alpha pixels | Operation |
| --- | --- | ---: | --- |
| Ordinary | 0–255 | 402,529 | Native antialiased RGBA |
| Binary only | 0 or 255 | 0 | Ordinary alpha ≥128 becomes opaque, exactly |
| Corrected | 0 or 255 | 0 | Same threshold, then 4/3/2 corrections |

Correction changes 11,030 support pixels relative to binary-only: 10,070 become
opaque and 960 become transparent. Both prepared PNGs carry exactly 254 DPI
metadata. All three remain native RGBA files with transparent surroundings;
viewer color is not exported. Matching output dimensions ensure the comparison
measures preparation rather than a change in resolution.

## Mix Patterns and transforms

Source: the immutable [900×620 SVG](../../../assets/vector-sample.svg), also
shown in the README's vector gallery. The
[editable mixed-channel project](vector-mixed-channels.toniator) and
[native 1800×1240 PNG](vector-mixed-channels.png) preserve independent channel
Patterns, calibration, Feature size, and transforms.

| Channel | Pattern | Feature size | Rotation | X offset | Y offset |
| --- | --- | ---: | ---: | ---: | ---: |
| R | One Guide Lines | 1.5 | 15° | 12 | −8 |
| G | Poisson Disk Marks | 0.95 | −10° | −16 | 10 |
| B | Triagrid Diamond Marks | 1.25 | 30° | 8 | 18 |

Select R, G, or B before **Change… → Apply Pattern** and before editing its
Feature size/transform. The [actual G inspector](screenshots/mixed-channel-G.png)
shows its generic **Random** family label: the saved typed construction is
Poisson-disk circular marks, with Even placement, minimum-center separation 8,
and seed 19. Pattern names and generic family labels describe different levels.
Offsets are the app's stored layout units, not print millimetres.

The document base density is 65. Effective channel densities are approximately
66.6667 / 105.2632 / 80, corresponding to the Feature sizes above. For the still
PNG use ordinary **2×** export, Automatic backing, and antialiasing On.

## Animate scale and rotation

Source: the [original opaque painted portrait](../showcase/source-portrait.png),
not the cut-out. [portrait-motion.toniator](portrait-motion.toniator) uses
source-colored Poisson-disk marks with maximum fill 0.5. Its authored document
Start/End tracks change density **65 → 160** (Feature size approximately
**1.53846 → 0.625**) and Rotation **0° → 60°**, with **Smooth in/out** interpolation.
The actual native frames are the endpoint/motion evidence.

- [Native AV1/WebM video](portrait-motion.webm): 1254×1254, 16 fps, 48 frames,
  approximately three seconds, native opaque black video backing.
- [Sharing GIF](portrait-motion.gif): 720×720, 24 sampled frames, nominal 8 fps,
  64-color palette, explicit black sharing backdrop.
- Native PNG frames [0](native-frames/portrait-000000.png),
  [24](native-frames/portrait-000024.png), [47](native-frames/portrait-000047.png)
  retain the original 1254×1254 RGBA pixels.

Frame 47 is the last sampled frame before the exclusive end at 3 seconds; it
approaches the authored End rather than adding a duplicate End frame.

## Animate independent channels

[vector-channel-motion.toniator](vector-channel-motion.toniator) uses the same
source and heterogeneous Patterns as the mixed-channel still. Base density,
every channel's density/Feature size, and combined scale stay fixed. Only these
nine independent channel tracks animate, all with **Smooth in/out**:

| Channel | Rotation Start → End | X Start → End | Y Start → End |
| --- | --- | --- | --- |
| R | 15° → −30° | 12 → −18 | −8 → 12 |
| G | −10° → 35° | −16 → 16 | 10 → −14 |
| B | 30° → −20° | 8 → −12 | 18 → −18 |

The [real End/B inspector](screenshots/channel-animation-End-B.png) shows the
last row's endpoint while Feature size remains 1.25.

- [Native AV1/WebM video](vector-channel-motion.webm): 900×620, 16 fps, 48 frames,
  approximately three seconds, native opaque black backing.
- [Sharing GIF](vector-channel-motion.gif): 720×496, 24 sampled frames,
  nominal 8 fps, 128-color palette.
- Native PNG frames [0](native-frames/channel-000000.png),
  [24](native-frames/channel-000024.png), [47](native-frames/channel-000047.png)
  retain the original 900×620 pixels.

### Timing and export

Open either project. **Main Menu → Animation settings…** shows **16 fps** and
source interval **0–3 seconds**, yielding **48 frames**. The
[actual timing dialog](screenshots/animation-timing.png) records these settings.
Use **Export video…** to choose a PNG sequence, FFV1/Matroska, or AV1/WebM.
The app authors Start/End states; it does not offer real-time playback/scrubbing.

The matching headless CLI also exports the stored animation. Use a new sequence
directory: Toniator creates it atomically, so it must not exist beforehand.
The accepted filename placeholder is exactly `%06d`.

```bash
./target/release/toniator render \
  -i docs/examples/print-animation/portrait-motion.toniator \
  -o target/validation/portrait-sequence/frame-%06d.png
./target/release/toniator render \
  -i docs/examples/print-animation/portrait-motion.toniator \
  -o target/validation/portrait-motion.webm --background black

./target/release/toniator render \
  -i docs/examples/print-animation/vector-channel-motion.toniator \
  -o target/validation/channel-sequence/frame-%06d.png
./target/release/toniator render \
  -i docs/examples/print-animation/vector-channel-motion.toniator \
  -o target/validation/vector-channel-motion.webm --background black
```

All 48 intermediate PNG frames in each sequence are rendered by Toniator. Native
video and sequence exports were independently decoder-checked at 16 fps/48
frames. WebM duration is 3.001 seconds; GIF centisecond timing totals 3.010 seconds.
The GIFs select 24 existing frames, resize for sharing, and convert to a palette;
there is no external interpolation or invented frame.

To reproduce the sharing derivatives with FFmpeg after exporting the sequences:

```bash
ffmpeg -v error -framerate 16 \
  -i target/validation/portrait-sequence/frame-%06d.png \
  -filter_complex 'color=c=black:s=1254x1254:r=16:d=3[bg];[bg][0:v]overlay=shortest=1:format=auto,fps=8,scale=720:720:flags=lanczos,split[a][b];[a]palettegen=max_colors=64:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4' \
  -loop 0 target/validation/portrait-motion.gif
ffmpeg -v error -framerate 16 \
  -i target/validation/channel-sequence/frame-%06d.png \
  -filter_complex 'fps=8,scale=720:496:flags=lanczos,split[a][b];[a]palettegen=max_colors=128:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4' \
  -loop 0 target/validation/vector-channel-motion.gif
```

## Provenance

Native exports use the pinned Toniator 0.4.5 app/CLI and 2× shared-renderer helper
identified in the [gallery provenance](../README.md#screenshot-and-output-provenance).
The print PNGs come from the actual GUI exporter; animations and their full
sequences come from the native CLI; mixed-channel and square-spiral stills use
the same 2× shared-renderer helper already matched against GUI export.

Full screenshots are raw 1440×1000 `grim` captures from a private headless Sway
session. Region screenshots are raw 645×525 viewport captures. Runtime manifests
verify four disposable XDG roots and private Wayland/D-Bus/AT-SPI sockets. This
is actual app evidence, not GNOME/Mutter, portal, human screen-reader, or physical
print acceptance. Native PNGs remain unedited. Local raw sequences, captures,
controls, logs, and shutdown proof are under
`target/validation/readme-alpha-print-animation-20261018/`.

<details>
<summary>Exact transparent-source edit prompt</summary>

```text
Use case: background-extraction
Asset type: transparent source artwork for a README demonstration of source alpha and garment-print preparation.
Input image: the existing fictional painted portrait is the edit target.
Primary request: remove the background and botanical surroundings completely, leaving a clean transparent cut-out of the same woman, dark curly hair, shoulders and white clothing.
Constraints: preserve her exact face, expression, pose, proportions, warm lighting, colors and painting style. Preserve fine hair curls and wisps with natural partial-alpha edges. The background outside the subject must be genuinely transparent, including visible openings between curls; no opaque white, black, gray, checkerboard or colored matte. Keep the original square canvas and subject framing. Do not add any objects, text, border, shadow, glow or texture. No halftone, dots or line screen: Toniator will render the effects from this source.
```

</details>
