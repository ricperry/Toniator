# Stages 22–23: media, animation, and frame export

Date: 2026-09-05. Accepted: 2026-09-06. Status: **Complete at commit `0f862512a86b7bb0eb25b915a496a7b68018b333`.**
The user accepts this combined implementation and its subsequent desktop layout
corrections. This document remains the implementation contract. Evidence is tracked in
[`STAGE_22_23_IMPLEMENTATION.md`](STAGE_22_23_IMPLEMENTATION.md).
Baseline: accepted Stage 21B at `8deb02d`, acceptance documentation at `c54ad44d`.
The protected Addendum §§5–6 remains normative.

## Intended outcome

A creative user can open moving artwork, apply a Pattern, optionally animate
supported settings between a start and an end value, preview either endpoint, and
export the selected interval as numbered PNGs or a finished video. A still
image can also supply the artwork for an animated Pattern. CLI and GTK use the
same frame selection, transition evaluation, and canonical geometry.

Stages 22 and 23 form one implementation effort and one final end-to-end
acceptance gate. The dependency steps below are internal milestones, not a
reason to finish headless work and defer its user interface to another effort.
The user provided combined acceptance on 2026-09-06; no later stage is authorized.

## Output decisions

- **Default video: lossless FFV1 version 3 in Matroska (`.mkv`).** Encode the
  rendered 8-bit RGBA PNG frames as BGRA, retaining RGB and alpha without a
  subsampling or RGB-to-YUV conversion. Compression effort may affect speed and
  size, never image quality. This is a preservation/editing output; universal
  browser playback is not promised.
- **PNG sequence:** write `frame-000000.png` onward and a manifest containing
  frame rate, selected source range, dimensions, and completion state.
- **Optional sharing output: AV1/WebM**, explicitly described as lossy and
  smaller. Proposed software encoder: SVT-AV1, 10-bit 4:2:0, CRF 18, preset 6;
  these speed/size defaults require artifact review. Transparent content needs
  an explicit export matte for this option. Do not silently flatten it.
- Headless numbered SVG sequences remain in scope because Addendum §5.1
  requires PNG or SVG sequences. Existing single-frame PNG/SVG export remains.
- The user's temporary lossless-AV1 preference was withdrawn. It is not a
  requirement or the selected default.

FFV1 is specified as a lossless intra-frame codec in
[RFC 9043](https://www.rfc-editor.org/rfc/rfc9043.html). Required encoder and
pixel-format support must be probed, not inferred from the presence of an
`ffmpeg` executable. Use a redistributable FFmpeg build with reviewed dependency
licenses; follow [FFmpeg's license guidance](https://ffmpeg.org/legal.html).
The host's enabled optional libraries are not a packaging recipe. AV1 is an
optional distribution format, consistent with
[AOMedia's open codec work](https://aomedia.org/about/story/).

**Audio is deferred pending scope confirmation.** Initial outputs are silent
and the export sheet says so when a source contains audio. The optional audio
question has not been answered; elapsed time does not authorize adding audio
processing. A later bounded addition could retain decoded integer audio with
FLAC in Matroska and use Opus for WebM. Audio editing, mixing, effects, and
speed changes are outside this combined stage.

## Destination and job lifecycle

The export sheet shows format, interval, frame rate, size, background/alpha,
audio availability, destination directory, and job name before starting.
Prompt for a directory when no default exists. A user can set or change a
personal default directory in settings; it is not stored in a Preset or project.
Persist portal grants where supported and prompt again when a grant expires.

PNG/SVG export creates a new, exclusively owned child directory. Existing
files are never overwritten implicitly. Video export creates a private
`/tmp/toniator-render-<random>` directory for numbered PNG intermediates, then
encodes to a temporary sibling of the destination file and publishes by atomic
rename after successful encoding and validation. Offer an advanced temporary
directory setting because `/tmp` may use RAM-backed storage.

Preflight validates the complete job, encoder capability, dimensions, range,
output ownership, and estimated storage requirement before rendering. Base
the conservative storage estimate on uncompressed RGBA size plus overhead,
not the unusually small compressed test clip. Check available space during
the job and stop cleanly on exhaustion. Never remove files outside this job's
owned paths. On successful publication remove owned intermediate frames. On
encoding failure retain them while the user chooses Retry encoding, Save PNG
sequence, or Discard; cancellation cleans temporary video output. Completed
sequence frames remain identifiable as incomplete until the manifest is finalized.

An immutable export snapshot isolates the job from later document edits.
Allow one export job at a time; starting a job does not save the project or
add history entries. Stop/close handling cancels and reaps the subprocess and
workers before releasing owned files. Arguments use process argument arrays,
never a shell. Consume progress and bounded stderr concurrently.

Report rendering as completed frames plus current-frame work, and encoding
as a separate phase with encoded frames/time. Do not reserve only the last 5%
of a single progress bar for encoding or fabricate an ETA. Publish completion
only after the file is finalized. FFmpeg's
[`-progress` output](https://ffmpeg.org/ffmpeg.html) supplies encoding progress;
coalesce updates to roughly 10 Hz and retain cancellation responsiveness.

## One headless temporal authority

1. **Domain:** typed `Constant`/`Transition` values, stable target/field IDs,
   interpolation, validation, commands, history, and effective channel
   resolution. Extend current property descriptors with temporal capability;
   GTK and CLI must not maintain separate lists of animatable properties.
2. **Media:** implement the Addendum's `SourceMedia`/`FrameSource` boundary in
   `toniator-sampling/src/media/`, following its module outline. The frame
   wraps existing `SourceField` authority with media identity, index, PTS and
   timebase. FFmpeg/ffprobe subprocesses handle moving-media probe/decode;
   the headless export runner owns encoding. Existing still decoders supply
   the same decoded-frame abstraction. Network media is excluded.
3. **Sampling:** consume decoded pixel buffers directly. Do not encode and
   decode temporary PNGs merely to pass a source frame into the evaluator.
   Existing source color and SVG sampling authority remains canonical.
4. **Engine:** evaluate a frame-specific immutable effective snapshot into the
   existing canonical scene. Frame identity and animated values participate
   in relevant cache keys. No container-specific Pattern or renderer branch.
5. **I/O:** persist media identity, project timing, and authored transitions;
   provide bounded archive/media access and sequence publication. CLI and GTK
   invoke the same render-job runner; neither owns animation mathematics.
6. **GTK:** project descriptors and domain commands, schedule cancellable
   previews, and reject stale publication by document revision and frame ticket.

Keep decoded-frame memory bounded: sequential export holds the current frame
and a small capped cache, not an entire decoded clip. Endpoint requests select
by presentation timestamp; verify that repeated requests and switching back to
Start agree with sequential decode. Scrubbing remains deferred until render
pipeline hardware acceleration is available. Initial compressed source/archive and decoded
pixel limits remain explicit, including today's 128 MiB source, 256 MiB archive,
and 64-megapixel decoded-source limits. Reject oversized input before expensive
work. Raising these limits or introducing streaming large-project containers
is a separate sizing decision, not an implicit consequence of video support.

Frame identity includes the source fingerprint, stream, presentation timestamp,
and decode/color settings. Video conversion honors declared range, primaries
and transfer before entering existing working-color sampling authority; add
color-reference checks rather than assuming raw video samples are sRGB.

Support still images, animated images, explicitly ordered image sequences,
and video. Select the first video stream initially; handle orientation and
sample aspect ratio consistently. Reject unsupported HDR rather than silently
applying an unspecified tone map. Animated images play one source pass;
looping forever is not inherited from an image container. Image sequences
have an explicit ordered manifest and assigned frame rate.

Package the required FFmpeg tools for AppImage and supply them inside the
Flatpak runtime/application sandbox. Do not depend on host execution or widen
filesystem/network permissions. CPU software decoding/encoding is the initial
authority; hardware-specific acceleration and GPU-only dependencies are deferred.

## Timing and transition semantics

- Store frame rate as a positive rational. Output is constant-frame-rate;
  preserve a CFR input's rate by default. For variable-rate input, show the
  proposed normalized rate and select the latest source frame whose PTS is
  at or before each requested source time. No blending or optical flow.
- Source time intervals are half-open `[start, end)`. CLI frame bounds are
  zero-based and inclusive; convert once into the canonical job range.
  Reject conflicting frame/time selectors. `--frame` selects one frame.
- For a time-selected duration `D`, emit `ceil(D * fps)` frames, with source
  timestamps strictly before the selected end. Pad no extra source frame;
  the last selected frame occupies the final output frame interval.
- For `N` output frames, frame `i` has output time `i / fps`; total duration
  is `N / fps`. Animation progress is `i / (N - 1)`, so the last frame reaches
  the end value. A one-frame job uses the start value. Trimming establishes
  the transition range; it does not change playback speed.
- A still source repeats over an explicit animation duration; propose 5 seconds
  at 30 fps in the initial UI. A moving source defaults to its full duration.
- Implement Hold, Linear, QuadraticIn, QuadraticOut, SmoothStep, SmoothInOut.
  Let `u` be clamped progress: Hold is start until `u=1`; Linear is `u`;
  QuadraticIn is `u²`; QuadraticOut is `1-(1-u)²`; SmoothStep is `3u²-2u³`;
  proposed SmoothInOut is piecewise quadratic (`2u²` below 0.5, otherwise
  `1-2(1-u)²`). Store the selected enum, not frontend-specific formulas.
- Angles interpolate as authored, unwrapped degrees: 0→360 is a full turn.
  Component and start/end-color transitions interpolate in the existing linear
  working-color authority. HEX is an endpoint entry/display format, not a packed
  integer to interpolate. Hue rotation is a separate explicitly selected color
  path, resolved into the same canonical paint before composition (see below).
- Evaluate base values and compatible named-channel deltas using the existing
  domain resolver. Do not flatten inherited settings into independent channel
  tracks. Edits to effective endpoints use the same domain conversion as static
  edits. Clear a transition by retaining the displayed evaluated value as a
  constant, through one undoable command.
- Validate finite values, field bounds, and coupled minimum/maximum ordering
  at endpoints and at every requested frame before output publication. Reject
  invalid combinations with target/field/frame diagnostics; never clamp silently.
  Seeds remain constant. First End initialization is undoable; subsequent frame selection
  adds no undo step unless a newly invalidated portion needs initialization.

Later user clarification controls storage and UI: the ordinary document is
the Start state. Store only End overrides and easing for supported properties;
first selecting End captures the active keyframable values, including equal values.
Those End values remain independent of later Start edits. Omission represents an
uninitialized value. A pattern replacement clears only its affected pattern-relative
End settings; other channels and independent paint, mapping, translation and opacity
are preserved. The next End selection initializes the missing settings from Start.
Do not store a second Start snapshot. The UI uses Start frame / End frame only.
Scrubbing is explicitly deferred until suitable render-pipeline hardware
acceleration exists; arbitrary frame evaluation remains headless export authority.

## Animatable property inventory

The existing-field allowlist has **20 scalar field IDs**, supplemented by the
color endpoint and hue-rotation authoring modes below. This is a count of
field kinds, not a count of channels or all proposed UI controls. Eligibility also depends
on the selected target, output capability, paint model, and current descriptor
applicability. A number being editable does not make it animatable.

| User setting | Current `PropertyFieldId` | Bounds and applicability |
| --- | --- | --- |
| Density | `Density` | Positive; effective channel layout |
| Density aspect | `DensityAspect` | Positive; authored density/aspect authority, not separate stored X/Y densities |
| Pattern rotation | `RotationDegrees` | Finite degrees; channel layout |
| X and Y translation | `TranslationX`, `TranslationY` | Finite document distance; channel-specific |
| Minimum and maximum mark size | `MarkMinimumFill`, `MarkMaximumFill` | Each 0–2, minimum ≤ maximum; applicable mark output |
| Minimum and maximum path thickness | `ConnectedMinimumThickness`, `ConnectedMaximumThickness` | Each 0–2, minimum ≤ maximum; applicable connected/curve output |
| Curve stroke alignment | `CurveResponseBias` | −1–1; curve-response capability; distinct from sampling bias |
| Shape rotation | `ShapeRotationDegrees` | Finite degrees; applicable shape output |
| Sampling gain | `ModeledMappingGain` | Nonnegative; modeled channels only |
| Sampling bias | `ModeledMappingBias` | Finite; modeled channels only |
| Paint components, on every RGB or CMYK ink channel | `ColorRed`, `ColorGreen`, `ColorBlue`, `ColorAlpha` | Each 0–1; internal linear RGBA representation of solid paint, including Cyan/Magenta/Yellow/Black channel paints; not source-sampled paint |
| Channel opacity | `Opacity` | 0–1; channel appearance |
| Minimum and maximum region fill | `RegionMinimumFill`, `RegionMaximumFill` | Each 0–2, minimum ≤ maximum; supported region resize output |

“Region scale/gap” maps to the existing region response fields and selected
static resize algorithm; it does not introduce an independent gap parameter.
Minimum/maximum pairs are coupled responses, not two unrelated validity checks.
Response tracks bind only to `ChannelOutput` with stable channel and output
IDs. `OutputLayer` response values remain static definition defaults. ALL
response animation edits are domain-owned batches to compatible channel outputs,
not animation of a shared definition. Density/aspect and both rotations support
document base/channel delta bindings; translation, mapping, color and opacity
are channel-specific. ALL edits of these channel-specific fields likewise use
one validated domain batch. Preserve compatible existing overrides.

Two current descriptor gaps are prerequisites: expose the positive density-aspect
bound already required by its resolver, and distinguish static `OutputLayer`
response authority from `ChannelOutput` delta authority. The current small
`active_controls` projection is not the complete animation allowlist.

**Static throughout a job:** Pattern/definition selection; source references;
channel model, visibility and paint kind; mapping component/placement/inversion;
seeds; counts and work budgets; guide prototypes, paths, centers, spacing,
phases and repetition; random distributions, clustering, exclusion and artwork
weight fields; parametric construction; connections/maze topology and limits;
output kind, prototype, orientation policy and site-use references; region
algorithm/sampling mode; Curve Motif alternating-row configuration. Numeric
definition settings in those groups remain static, including artwork-weight
gain/bias. No discrete Hold tracks in this first effort.

Add descriptor tests covering every field ID: each must be explicitly allowed
or static, with scope/capability reasons. Reuse descriptor bounds in commands,
persistence validation, CLI, and UI. This table documents the proposed contract;
the implementation's domain descriptors become its executable authority.

### Color animation and CMYK channel coverage

The user requested explicit color animation on 2026-09-05. Document channel
model, paint representation, and export encoding are separate concepts:

- An RGB document has Red, Green and Blue semantic channels. A CMYK document
  has Cyan, Magenta, Yellow and Black semantic channels.
- Each of those channels has a solid paint stored as canonical linear RGBA
  (`ColorValue`), regardless of channel role. A Cyan channel can therefore
  transition from cyan to purple without changing which source component it
  samples. RGBA field names do not restrict animation to RGB documents.
- CMYK source separation and the fixed subtractive compositor remain unchanged.
  PNG and composed video encode the resulting image; their pixel format does
  not determine which document channels can be animated.

Expose the following capabilities for each named channel, and as explicit
domain batches for ALL while retaining each channel's separate identity:

| Document channel | Paint color animation | Presentation strength | Source-driven response |
| --- | --- | --- | --- |
| Red, Green, Blue (RGB document) | Start/end HEX colors, hue rotation, or component editing | Paint alpha | Sampling gain/bias and applicable geometry responses |
| Cyan (CMYK document) | Start/end HEX colors, hue rotation, or component editing | Cyan paint alpha | Cyan mapping gain/bias and applicable geometry responses |
| Magenta (CMYK document) | Start/end HEX colors, hue rotation, or component editing | Magenta paint alpha | Magenta mapping gain/bias and applicable geometry responses |
| Yellow (CMYK document) | Start/end HEX colors, hue rotation, or component editing | Yellow paint alpha | Yellow mapping gain/bias and applicable geometry responses |
| Black (CMYK document) | Start/end HEX colors, hue rotation, or component editing | Black paint alpha | Black mapping gain/bias and applicable geometry responses |

**Color assignment:** replace Advanced's raw RGB assignment with one color picker,
swatch, `#RRGGBB` / `#RRGGBBAA` entry and alpha control for the selected frame.
Do not add Start color / End color labels or paired endpoint controls. The existing
Start frame / End frame toggle is the keyframe control for all ordinary editing.
Parse sRGB endpoints into canonical linear RGB; interpolate
straight RGB components and alpha independently, as for explicit component
tracks. Store full-precision values, not rounded display text. This is a color
transition over time, not a spatial gradient. A grouped color edit is one domain
command and one undo step over the same component authority.

The user clarified that starting/ending colors are the artistic controls and
animating alpha supplies strength. Make those the primary color-animation UI:
the selected frame's ordinary Color controls. HEX alpha and an alpha control
edit the same value; never multiply two copies of it. The Color interpolation
choice updates paint and its alpha transition together, including hue mode. Channel opacity
remains separately available with its current compositing semantics, but do not
introduce another strength parameter or duplicate CMYK percentage controls.

**Hue rotation:** use the ordinary Start paint plus a stored End angular offset
and easing. Use HSL hue in encoded sRGB, keeping the base saturation, lightness
and alpha fixed; convert the result to canonical linear RGBA before composition.
Use [CSS Color 4's HSL conversions](https://www.w3.org/TR/css-color-4/#the-hsl-notation)
as the conversion reference. Angles are unwrapped: 0→360 traverses a full color
wheel, negative offsets reverse direction, and 0→720 makes two turns. Wrap only
for conversion, not before interpolation. Gray, white and black retain their
color under pure hue rotation; use start/end colors to colorize them. HSL
lightness is not a promise of constant perceived brightness.

Component/color-endpoint mode and hue mode are mutually exclusive RGB bindings
for a channel, chosen explicitly; do not apply competing RGB writers or silently
bake a hue path into linear endpoints. Hue's alpha remains constant unless an
explicit alpha transition is authored; channel opacity remains independently
animatable. Domain descriptors, commands, persistence and CLI carry the typed
mode and its parameters. GTK only projects that authority. All modes remain
presentation changes and must not regenerate sites or mark geometry.

These additions cover solid channel paints in both RGB and CMYK documents.
`SourceColorAlpha` keeps sampled source paint; applying a hue filter to every
sampled source color is a different capability and is not implicitly added.
The user's start/end-color and alpha clarification settles the intended workflow;
four-percentage CMYK color entry is not part of this plan. Do not invent
`ColorCyan`/`ColorMagenta`/`ColorYellow`/`ColorBlack` paint fields as aliases for
source separation or channel opacity.

## Project, Preset, and UI behavior

Projects retain source media, selected interval, rational frame rate, duration,
and authored transitions. Keep portable embedded source ownership and existing
bounded I/O; no implicit loose external video reference. Image sequences embed
their ordered source entries and manifest within the aggregate source limit.
Propose shared configuration schema 8 for transitions and project container
version 2 for the media manifest; the source-free Preset envelope remains
version 1 with shared configuration schema 8. Current-only
loading rejects superseded configuration schemas; do not add migration adapters
or rewrite user-owned projects/Presets. This incompatibility needs explicit
acknowledgment in implementation review.

In the container, project-only timing/range and the source manifest live in
the document envelope, outside `DocumentConfiguration`. Media entries use
generated archive names (`sources/000000`, etc.); the manifest records kind,
ordered entry references, hashes and stream selection. Reject unsafe paths,
missing/duplicate entries and aggregate-size violations. Configuration stores
only normalized transition bindings and reusable settings. The standalone
structural Pattern format (currently version 4) remains distinct and unchanged.

Source-free `.toniator-preset` stores normalized transitions, including the
selected color mode and full-precision endpoints/hue parameters, alongside reusable
settings. Loading retains the destination project's source, canvas, timing,
and selected interval; transitions stretch across its render range. A valid
current `.toniator` remains accepted by `Load preset...` through the same
configuration extraction. Loading is one undoable transition. Personal
structural Patterns remain static recipes, distinct from document Presets.

Show a Start frame / End frame toggle below the canvas when moving media or
animation is active. It selects the endpoint shown and edited by the inspector;
selection itself is transient and never adds history or marks the project dirty.
Place the linked endpoint pair in the same canvas toolbar as Preview / Source,
per the user's 2026-09-06 layout correction. Keep Preview / Source in its existing location. No scrubber, drag-to-seek,
real-time playback, or additional transport is included. Intermediate frames
are evaluated for export. Timing/range settings remain available for the job.

Eligible inspector rows edit the selected endpoint, with easing progressively
disclosed. Start edits change ordinary document values; End edits store only
supported values. Static settings remain shared and use the normal controls at
either endpoint. Changing a pattern invalidates only the affected End dependencies.
ALL response edits preserve compatible outputs individually
in one domain batch. Never collapse differing channel values. Keep inheritance and
reset semantics visible. Do not expose arbitrary keyframes, multiple segments,
timeline lanes, curve editors, or a dope sheet. Account for every interactive
control's product name, role, value, enabled state, keyboard path, and semantic
action/readback; use the private GTK harness for implementation verification.

## Internal implementation order and final acceptance

1. Domain timing, typed transitions, descriptor allowlist, effective resolution,
   and current-schema persistence, including source-free Preset behavior.
2. Bounded frame providers and frame-aware engine/cache identity; deterministic
   still, animated-image, sequence, CFR and VFR selection.
3. Shared export runner, CLI ranges, PNG/SVG sequences, FFV1, optional AV1,
   cancellation, progress, temporary storage and atomic publication.
4. GTK frame selection, endpoint editing, export/destination settings, accessibility,
   packaging, and end-to-end workflow verification.

One writer owns implementation at a time. Independent read-only reviews cover
temporal/cache/persistence correctness and the complete creative workflow.
Use focused new/current foundational tests, frontend-target compilation, strict
Clippy, architecture validation and diff checks. Do not sweep obsolete historical
test suites or regenerate old validation directories.

Acceptance must demonstrate:

- The existing `assets/video-sample0001-0010.mp4`: all 10 frames, 1080×1920,
  6 fps, correct order/duration, preview/export frame agreement; a real Toniator
  Pattern render, not merely codec transcoding.
- Both immutable still fixtures animated through the same frame pipeline;
  directly inspect native PNG 1024×1024 and SVG 900×620 outputs and representative
  first/middle/last frames. Preserve native RGBA; inspect alpha separately.
- All 20 fields' endpoints, midpoint/easing, applicability, inheritance,
  min/max constraints, source-frame cache invalidation, cancellation and stale
  publication. Include a one-frame range, 30000/1001 timing, VFR seeking,
  transparent animated input and an explicit sequence manifest.
- Color endpoints and hue rotation on RGB and each C/M/Y/K channel; exact
  endpoints, linear-light midpoint, alpha/hidden RGB, positive/negative/full-turn
  hue paths, gray/black no-op hue, mode-conflict rejection, independent ink
  strength, ALL batches with differing colors, persistence and Preset round trips.
  Verify preview/PNG/SVG/FFV1 agreement and presentation-only cache invalidation.
- Save/reopen project and Preset round trips; loading a valid current project
  as a Preset retains source/timing; obsolete and malformed files fail cleanly.
- Decode FFV1 back to RGBA and compare every pixel to the rendered PNG inputs,
  including fractional alpha and hidden RGB. Inspect AV1 output separately;
  do not claim it is pixel-identical. Verify truthful silent-output messaging
  with a generated audio-bearing fixture; the bundled video has no audio.
- Missing codec, insufficient disk, existing destination, encoder failure,
  cancel during decode/render/encode, retry, cleanup, and paths with spaces;
  no damaged destination or unrelated file removal.
- Private GTK semantic, keyboard, screenshot and log evidence for the complete
  workflow; stop its private session. AppImage/Flatpak tools must work inside
  their packaging boundaries. Automated Sway evidence is not human
  GNOME/Mutter/portal acceptance.

## Planning evidence and limits

`target/validation/stage22-23-planning/codec-proof.json` records a codec-only
probe: the sample's ten decoded RGB frames were encoded with FFV1 v3/BGRA and
decoded again; all RGBA pixels matched. The 10 PNGs total 2,047,764 bytes and
the Matroska file is 1,482,682 bytes. This small opaque clip proves neither
general compression ratios, alpha edge cases, encoding speed, nor Toniator
media implementation. FFmpeg 8.1.2 was used locally. Sample SHA-256:
`c84d4a42cf62803d41ac35152fd3fea1719a664c633900cb946b9b5a6d6bef81`.

No production implementation, schema change, protected specification edit,
stage acceptance, commit, push or release is performed by this planning work.
