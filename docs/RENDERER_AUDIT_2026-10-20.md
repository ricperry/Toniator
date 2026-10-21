# Renderer audit — 2026-10-20

The renderer bughunt repairs reproduced preview, raster, SVG and export defects
and adds whole-artwork background rendering at visited zooms for the current
accepted artwork/frame.
Implementation, scoped tests, independent source review and parent visual/runtime
review pass, and the user accepts the changes for 0.4.6 release preparation.
The engineering evidence was collected on inherited dirty HEAD
`12e1d54ff707e65022daaf2d349b08b3feb270b3`, with version 0.4.5 metadata. Those
historical pins do not identify the 0.4.6 packages; release provenance records
their source checkpoint and binaries. The five protected specifications retain
normative authority, with the Addendum taking precedence.

The frozen [background-cache report](../target/validation/preview-background-cache-20261020/report.md)
is the current preview source/runtime entry point, with
[final pins](../target/validation/preview-background-cache-20261020/FINAL-source-binary-pins.json).
The earlier [C2 report](../target/validation/renderer-bughunt-20261020/cache-c2/report.md)
retains its consumer/export checks and historical source/control pins. The user
rejected its demand-only cache usability result: separated settled checks do not
prove continuous-panning behavior. Its 4 GiB policy is superseded. Earlier phase
reports document their own gates. In particular, the paused
`final/` bundle predates cache integration and is partial evidence for older
sources; it is not final cache verification.

## Repaired defects

| Area | Verified resulting behavior | Evidence |
| --- | --- | --- |
| High-zoom precision | Normalization split debt is waived only when both original and accepted normalized whole fills are proved invisible under outward bounds for actual adapter readbacks. Geometry, final/global debt, cumulative `1e-6` authority, work and cancellation remain unchanged. Retained roots replay one ordered whole-view ledger, without tile-local allowances. | [400% source proof](../target/validation/zoom400-preview-20261020/source-proof.md), [C1](../target/validation/renderer-bughunt-20261020/cache-c1/report.md) |
| Raster consumer contracts | AA-off full/stream paths honor NonZero as well as EvenOdd. Raster transforms require finite positive scales and finite offsets. SVG ordered outputs have unique deterministic IDs. | [B1](../target/validation/renderer-bughunt-20261020/b1/report.md) |
| Sampled numeric boundaries | Mapped nonfinite arithmetic fails closed. Reproduced huge finite orientation/scanline and circle-support/derived-zero failures are repaired within an explicit supported fallback domain, with healthy output preserved. The direct `robust` 1.2.0 dependency was already resolved; no new package version was downloaded. | [B1b repair](../target/validation/renderer-bughunt-20261020/b1b/repair/report.md), [B1c](../target/validation/renderer-bughunt-20261020/b1c/report.md) |
| SVG model composition | RGB uses premultiplied linear additive RGBA instead of screen. RGB/CMYK filters reference live same-document original channel groups, removing the reproduced atlas capture defect while retaining editable geometry and no embedded raster. | [B2](../target/validation/renderer-bughunt-20261020/b2/report.md), [B2c](../target/validation/renderer-bughunt-20261020/b2c/report.md) |
| SVG publication | App and CLI stage output atomically; prepublication failure or cancellation preserves an existing destination. Successful publication remains distinct from later durability warnings. | [B2](../target/validation/renderer-bughunt-20261020/b2/report.md) |
| Temporal publication/recovery | Final-callback cancellation is checked before finish. Published video/complete sequences report saved truth, with separate durability/cleanup warnings. A complete recovery copy remains Complete when later cleanup refuses. | [B4](../target/validation/renderer-bughunt-20261020/b4/report.md), [B4b](../target/validation/renderer-bughunt-20261020/b4b/report.md) |

These are bounded reproductions and repairs, not proof of a bug-free renderer.
No fitting is requested by the saved-authority checks. A historical B1c observer
field reporting zero fits was a constant, not a measured fit counter; its
provenance erratum remains with that bundle.

## Current preview cache

Each visited eligible exact physical zoom now autonomously fills the whole
artwork on a fixed 1536-pixel canvas-origin grid. Bounded visible priority does
not starve other visited zooms, and panning preserves their background demand.
The closest certified already-cached scales provide disjoint initial coverage
while requested-scale pixels stream in. The engine owns one shared scene `Arc`,
prepared roots, coverage, in-flight work, certificates and LRU accounting.
One dedicated worker performs preparation, certificate scans/restoration,
painting and upload-data assembly; GTK owns widgets and textures. Root generation
and independent scene-epoch guards reject stale work. A successful explicit domain
`None` transition can rebind unchanged
ownership, including in-flight work and the source key; genuine artwork, frame,
workspace, model, canvas or paint changes invalidate it. Source-view toggles
retain data within the same frame while visited-zoom background work continues.
A real settings-GUI `None` command was not forced: that boundary is covered
by source review and in-flight engine tests.

The root grid has a fixed canvas origin. GTK contain/ceil layout, ancestor
placement, device scales 1/2, actual sampling support, neighbor gutters and
disjoint physical draw clips are covered. Each displayed source scale needs a
current whole-source-view certificate under the existing precision/work/cancellation
authority. Read-only containing-envelope queries can reuse permission during
motion; refusal cannot consume restoration or change required-region authority.
Storage alone cannot authorize display. Transparent pixels count as coverage;
alpha is not a cache-validity mask. Fractional
presentation resamples this grid, so arbitrary old per-pan sampling-phase byte
identity is not promised. Unsupported optimizations fall back to the ordinary
renderer and a separate exact-target memo.

Whole-root retention has an 8 GiB soft target, with active/projected/running roots
protected and older unpinned zoom roots coherently evicted. It charges CPU pixels,
prepared geometry/debt, the shared scene, additional owned materialization,
gutters, conservative GTK/GPU copies
and estimated in-flight storage. It neither preallocates that amount nor caps
process RSS; source decode, allocator/backend overhead and other app allocations
remain outside that guarantee. Oversized roots use ordinary viewport rendering.
App restarts and earlier canonical frames are outside retention.
[Performance](PERFORMANCE.md#046-retained-preview-regions)
separates paired measurements from diagnostic timings and warm metadata queries.

Tiled paintables now declare dynamic contents, invalidate on scroll/layout, return
an immutable `current_image`, and check actual PAINT-phase translation against
existing whole-view permission. Full fallback strips cover newly exposed canvas
outside the projected patch without underpainting alpha-zero tiles. Ordinary and
Source paintables stay static. A current-frame full Fit bootstrap precedes detail
after edits at retained non-Fit zoom, preserving zoom/scroll; ordinary/memo handoffs
retire stale crop observations. Actual Picture/ScrolledWindow checks cover
invalidation, immutable snapshots, transparent coverage and newly exposed fallback.
The reported full blackout was not reproduced in the frozen 32 held samples;
these repairs address verified source-contract defects, not a reproduced
full-blackout before/after claim.

## Verified consumers and interaction

The current resident proof has 32 actual surface captures during uninterrupted
primary-button motion: 359 distinct origins/crop readbacks and 892 draws at
physical scale 8, with zero new preparation, painting, pixels, certificate queries
or upload-data copies and no foreground pending. Separate current-frame cold
motion has 32 held captures, 358 crop readbacks and 306 draw origins as 23 new
fixed tiles stream; all captures retain visible artwork with disjoint requested
tiles/full fallback. Initial repaired `cold-*` captures were not sustained pan
and are excluded; `edit-cold-*` is the valid cold proof. First warm traversal
constructs uploads and is not mislabeled zero-work resident reuse.

Device-1 natural raster/vector controls each have eight held captures, 366
distinct crops and all five visited zooms complete. Vector's live-text fallback
notice is not a Preview error. Its immediate `vector-01-150.png` still shows Fit
and is not a settled 150% witness. Source/runtime summaries are linked from the
[current report](../target/validation/preview-background-cache-20261020/report.md).
These samples do not prove every frame, an FPS target or human desktop acceptance.

Earlier C2's nine Spectrum Main checks match frozen Canvas/Complete RGBA, authority,
scene facts and logical work across the tested origins and zooms. All seven
cache-case hashes match. C2's normal Main warm trace has 31 events
covering reverse pan, 400→300→200→150→200→300→400 and cached pan, with zero new
source/evaluation submissions, preparation, paints, pixels or texture-data copies
and no pending/error state. These are historical settled observations, not
continuous-motion proof. The texture counter does not measure driver GPU
traffic; no app FPS or end-to-end microsecond claim follows.

The remaining consumer/export evidence below is C2-scoped. Both immutable inputs
retain their documented hashes. Fresh natural projects
match frozen saved-companion bytes. Four native/streamed PNG AA-on/off pairs at
1024×1024 and 900×620 match complete one-frame RGBA, including hidden RGB. Raster
AA-on also matches the old native output. Vector AA-on differs in 1,230 live-text
pixels; old 512/new 1536 CLI outputs are exact in the same isolated environment.
`fc-match` observes Liberation Sans there versus Arial with original font data;
this is not instrumented fontdb-selection proof. No old AA-off comparison was
performed. Both SVGs have unique IDs, editable geometry and no embedded raster,
and render in resvg/Inkscape; visible coverage, gray and edge differences remain.
Inkscape white hidden RGB at alpha zero is transparent, not an opaque background.

C2's 32 structured GL/Cairo raw/decoded-PNG pairs are exact within that gate's
unchanged physical draw-node dependency cone. Those earlier pinned binaries do
not prove the new paintable contract. High-entropy observations reach one code
in premultiplied RGB/alpha, with straight RGB reaching 16 at alpha 16. Neither
result proves universal GPU/backend byte parity.

Actual private GTK evidence covers device-2 Spectrum zoom/warm reuse and device-1
natural four-corner pan, zero-work resize/restore, Feature-size edit/Undo/Redo,
workspace replacement, vector Fit/150% negative letterbox, and Source End 149 /
Start 0 / model refresh. SVG chooser cancellation and successful app save are
observed; the 237,966,931-byte app SVG equals CLI bytes. Healthy 8×8 FFV1 decodes
exactly to the initial PNG sequence. Actual Retry uses a fresh destination;
Save PNG produces a complete exact copy and truthful Complete/path/Close with a
controlled cleanup-refusal warning. Original frames remain exact for that
sentinel refusal. Live Cancel reaps the encoder and removes generated temporary
files without a destination. Discard is separately exercised after controlled
encoding failure. Fault shims are confined to owned PATH, with real healthy
FFmpeg controls. This is not a whole-GUI late-directory-sync fault injection.

No accessibility schema or new setting is introduced. The new passive native
progress bar is at the viewport bottom, three logical pixels high, named
**Background rendering progress**, with range/value 0..1, no actions and no focus.
It reports unique stored pixels across admitted visited zooms, independently of
alpha, and hides on completion or terminal stop. Device-1/2 semantic values,
completion absence and native pixels are recorded. Tooltip property/AT-SPI
description pass; successful hover display is not claimed. Existing GTK
and descriptor vocabulary remains authoritative: stable names/hierarchy, real
roles/actions/state/value/selection, label relations, truthful enabled state,
semantic actions/readbacks and native screenshots/logs are recorded in
[C2 controls](../target/validation/renderer-bughunt-20261020/cache-c2/controls.md)
and the [current control evidence](../target/validation/preview-background-cache-20261020/report.md#focused-checks-and-controls).
Keyboard paths remain unchanged. Ctrl+O/Ctrl+L native Open passes. Automated Close
focus was not verified; Space opened an owned chooser, which was cancelled,
and semantic Close succeeds. No full keyboard, human, GNOME/Mutter or
production-portal acceptance is claimed by automated private Sway evidence.

## Remaining findings and limits

- **Source color + alpha SVG local colored overlap:** native `[156,213,0,191]`
  differs from resvg/Inkscape `[85,170,0,192]` in the reproduced case. Per-primitive
  full-canvas filters were not implemented because of their cost; that design
  requires a separate approval gate.
- **Spectrum editable SVG precision refusal:** `svg.precision.contour` remains
  a conservative prepublication refusal and preserves an existing sentinel.
  The evidence does not establish that refusal as avoidable.
- **Ordinary SVG/vector Boolean protection:** existing ordinary operations have
  work/cancellation gaps. The checked solver changes numerical policy and is
  not a semantics-preserving drop-in replacement. A separate future gate is
  required; no output-policy change or new roadmap is approved here.
- **General numerical/consumer coverage:** arbitrary finite conics/cubics,
  pathological inputs, all resource/cancellation latencies and every SVG engine
  are not universally proved. External AA/quantization remains consumer-specific.
- **Architecture guard:** the unchanged IO→patterns development dependency still
  triggers the shell guard; production headless-boundary metadata checks remain
  separate. This false positive is not fixed by the milestone.

## Evidence preservation

Current [preservation](../target/validation/preview-background-cache-20261020/FINAL-preservation.json)
records 851 unchanged paths of 858 admitted, exactly seven allowed source changes
and all three original input hashes exact. Final normal app pins and independent
source review pass. The frozen followup manifest covers 1,497 files; its
[shutdown proof](../target/validation/preview-background-cache-20261020/FINAL-process-stop-proof.json)
records all 63 PIDs absent and 62 owned units inactive. No runtime remains.

The parent previously verified all 1,737 frozen C2 manifest entries and nine
source/three executable pins at that historical gate; its manifest SHA-256 is
`b498055686eb55e2d6272a12cd4bba01ac695d5a768a578e4d228ce6533dbc04`.
The [preservation record](../target/validation/renderer-bughunt-20261020/cache-c2/preservation.json)
checks 848 unchanged nonowned admission paths. Of 4,430 prior artifact hashes,
4,428 are exact; two inherited B1c proof/report manifest exceptions are approved
observer-provenance errata predating C2. Earlier reports and pins are retained.
The [stop proof](../target/validation/renderer-bughunt-20261020/cache-c2/all-process-stop-proof.json)
records all 58 PIDs absent and 31 owned units inactive. Controlled cleanup-refusal
files are retained evidence, with no running process. These older pins do not
identify the new preview implementation. Acceptance and checkpoint
transitions remain parent-owned.

2026-10-20 followup: the user confirms the rebuilt cache works on their desktop without blackouts.
The subsequent canvas-status-tooltip-only adjustment has fresh [verification and binary pins](../target/validation/canvas-status-tooltip-20261020/report.md); earlier frozen bundles remain historical and unchanged.
