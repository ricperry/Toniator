# Measured performance

The shared preview, accepted-scene reuse and Random/Dispersion optimizations
measured on 2026-10-16 are included in Toniator 0.4.4. The staged comparisons
below were collected during development from GUI source checkpoint
`8ee739375313e11c07a325f297a9e91a257ac963` with the respective pilot changes;
they are not new benchmarks of the packaged release. Implementation and scoped
verification are complete. These measurements establish specific workload
improvements, not universal speedups or user acceptance. The later recovery and
capacity work below is user-accepted for the 0.4.5 release preparation.

## 2026-11-18 development exact Pattern construction

The current development app enables exact Vulkan geometry assistance independently
of the Fast/Accurate canvas switch. It constructs the same canonical family and
scene used by preview, PNG and SVG. Existing settings, calibration and UI controls
continue to apply. The CLI remains headless and CPU-only by default, with
an explicit `geometry-compute` build feature. See [build options](BUILDING.md).

| Work | Current route and preserved authority |
| --- | --- |
| Tangent ribbons Spacing, Size and Blend | Automatic Vulkan certification of independent nearest-guide ownership queries. CPU source sampling, mass accumulation, tracing and final placement retain their original order. |
| Fixed and rotated rectangular grids and generalized straight guides, including Triagrid | CPU construction uses guide/orientation indexing, uniform tangent-pair basis and diameter reuse, borrowed merge buckets, stable raw sorting and bounded row rejection. Rectangular Vulkan assembly is forced-diagnostic only. |
| Bayer and Stochastic cluster screen | Independent source samples use bounded indexed Rayon blocks at 262,144 or more candidates. Results are consumed in the original serpentine order. Cluster nucleus rows are reused. |
| Floyd–Steinberg diffusion, quadtree debt, seeded Poisson, weighted Lloyd, curve tracing and greedy merging | CPU topology and order-dependent decisions remain authoritative. Shared sampling/indexing improvements can reduce surrounding work. |

Intrinsic placement and ordinary mark realization reuse exact bounded prepared
source mappings and avoid duplicate density samples. RGB/Alpha component sampling
avoids unused channel conversions. These changes retain numeric results, site
identity, provenance, ties, stable ordering and authored density.

Automatic ribbon admission starts at **65,536 ownership queries per window**.
Row/column stripes retain at most **1,048,576 owners** at once. Individual GPU
buffers and point/index prefixes are capped at **64 MiB**. This permits meshes
larger than three million queries without retaining the whole ownership mesh.
Only physical discrete/integrated Vulkan adapters are admitted. Integer interval
certificates require a strictly separated winner. Overlaps, ties and unsupported
numeric ranges use the original CPU nearest-guide routine. Unavailable hardware,
allocation/device failures and contention also fall back to CPU. Cancellation
discards private pending work and returns the ordinary cancellation result
before publishing a canonical family. The shaders use integer arithmetic
without optional FP64.

`TONIATOR_GEOMETRY_COMPUTE=cpu` disables geometry dispatch before initialization.
Unset or `auto` uses the admission policy above. `=gpu` forces supported diagnostic
jobs, including the unrotated rectangular-grid kernel, and bypasses the minimum
query threshold.
`TONIATOR_GEOMETRY_COMPUTE_TRACE=1` reports adapter, kernel, certificate/correction
counts and timings to stderr. These are developer diagnostics, separate from
Fast preview.

Measurements use RX 6900 XT/RADV Vulkan, four Rayon workers and genuine calibrated
fixtures derived from both immutable sample artworks. The
[verification method](../target/validation/pattern-compute-20261118/README.md) and
[raw summary](../target/validation/pattern-compute-20261118/summary.json) retain
binary/source pins, complete family/scene hashes and guarded resource records.
Timed engine calls include upload, dispatch, synchronization, readback and CPU
fallback, plus the ordinary one-pixel AA-off raster. Native export timings are
separate. Setup/loading/pool creation are reported separately. These local
development measurements establish engine-call latency for the named workloads.

| Matched workload | Pre-change CPU | Optimized CPU | GPU-assisted | Measurement |
| --- | ---: | ---: | ---: | --- |
| Dense raster Tangent ribbons Spacing 512, 653,436 RGB primitives | 6.011 s | 3.707 s | 2.255 s forced | Medians of 3 baseline and 4 final calls per route. Both ownership windows meet automatic admission. |
| Fresh CMYK raster Tangent ribbons Blend 192, 134,511 primitives | 3.089 s | 2.847 s | 2.038 s automatic | Medians of 2 baseline and 4 final calls per route. |
| Wizard CMYK Star vector 192, 46,218 primitives | 1.364 s | 1.286 s | 0.979 s automatic | Medians of 2 baseline and 4 final calls per route. |
| Dense raster Tangent ribbons Spacing 704, 1,230,243 RGB primitives | — | 7.346 s | 5.046 s automatic | One call per route. Over 3 million ownership queries in four stripes. |
| Raster rectangular grid 1024, 3,213,555 RGB primitives | 4.326 s | 2.966 s | 3.067 s forced | Medians of 3 baseline and 4 optimized calls per route. Automatic mode uses CPU. |
| Vector rectangular grid 1024, 2,219,991 RGB primitives | 4.395 s | 2.318 s | 2.449 s forced | Medians of 3 baseline and 4 optimized calls per route. Automatic mode uses CPU. |

Complete family/scene hashes match across the compared routes. For both fresh
colored CMYK cases, native PNG bytes, raw straight-sRGBA pixels and SVG bytes also
match the pre-change output exactly. Native rasterization and export are measured
separately from the engine-call results above. The paired grid measurements favor
the optimized CPU route on both artworks. In a separate run of the final app
policy, the two rectangular grid cases took 3.364 s and 2.581 s respectively,
both on CPU. The [complete benchmark report](../target/validation/pattern-compute-20261118/report.md)
includes the Bayer, Stochastic cluster screen and Triagrid measurements.

With the Vulkan driver unavailable, the raster Spacing 100 project completed
on the CPU. Its geometry, native PNG, RGBA pixels and SVG matched the baseline.

The final ordinary app check uses all four CMYK channels with Accurate preview
and Fast off. Across four dispatches, 342,531 ownership queries are GPU-certified
and 6,233 are resolved on CPU. The
[GUI record](../target/validation/pattern-compute-20261118/gui-verification.json)
retains the executable pin, semantic readback, screenshot and clean session
shutdown. This is automated private Sway evidence. Human and GNOME Shell/Mutter
acceptance remain unverified. The 65,536-query policy is measured on this adapter.
Other hardware needs paired workload measurements.

## 2026-11-02 development: Fast / Accurate preview

Toniator 0.4.7 includes optional approximate GPU rendering for the main
canvas; Accurate CPU rendering remains each new window's default. Parent technical
review passes the bounded implementation. The user accepts its visual quality
for now on 2026-11-02, finding GPU/CPU differences exceedingly difficult to discern.
[TON-025](../ISSUES.md#ton-025--reevaluate-cpugpu-preview-defaults-at-a-later-stage) defers default reevaluation; Accurate remains the default. Broader
GNOME/Mutter qualification remains unproven. Fast consumes the same canonical geometry, calibration
and density through a genuine scene-only evaluation path, without first paying
for a CPU raster. Eligibility is whole-scene supported circles in RGB/CMYK;
unsupported content, device or target selects whole CPU fallback. Independent
exports, print, temporal output, gallery and Wizard consumers retain CPU authority.

The historical development [qualification report](../target/validation/fast-accurate-preview-20261102/stage4-report.md)
and [parent review](../target/validation/fast-accurate-preview-20261102/stage4-parent-review.json)
retain source/binary pins, images, individual timings, preparation and resource
records. Five alternating pairs per artwork/policy use the same release Main
integration-test binary on RX 6900 XT/RADV, genuine Feature size 0.2 inputs,
physical 640×480 viewports, vector 4× and raster 2×. Each endpoint requires current
complete primary Canvas coverage and same-frame nonzero GDK presentation feedback,
including viewport assembly, copies and texture upload. Ordinary native input
and rendered frames corroborate the integration tests; these are not FPS or
physical scanout measurements.

| Presented pan latency | Accurate median | Fast median | Scope |
| --- | ---: | ---: | --- |
| Ordinary vector 4× | 62.3 ms | 73.3 ms | 40 pans per route; background prefetch ON |
| Ordinary raster 2× | 57.6 ms | 60.3 ms | 40 pans per route; background prefetch ON |
| Conditioned vector 4× matched misses | 1,118.6 ms | 105.8 ms | 15 matched observations; 10.57× ratio of medians |
| Conditioned raster 2× matched misses | 1,510.2 ms | 92.9 ms | 10 matched observations; 16.25× ratio of medians |

Ordinary CPU caching remains competitive; its five initial lazy-root stalls per
artwork are retained. The separate conditioned diagnostic disables autonomous
prefetch and waits for ready roots before timing misses. Preparation is excluded
from those miss intervals and reported separately: CPU root-ready medians are
13.3 s vector / 20.3 s raster. Fast mode cycles include outliers of 15–22 s and are
not pure GPU initialization timings. Five conditioned-raster CPU hit pans have
exact Canvas/after-paint but unavailable GDK presentation feedback at two seconds;
they remain disclosed without imputed presentation. No universal pan speedup is
claimed.

Fresh-process initial-Fit observations, N=1 per route/artwork with warm OS caches,
start **after UI construction but before document loading** and include decode,
canonical construction, backend initialization/preparation and first complete
Canvas/GDK presentation. Vector wall time is 17.370 s Accurate / 5.796 s preselected
Fast; raster is 27.228 / 9.634 s. These are not full app-startup or repeated cold
guarantees. Process high-water memory includes startup/UI baseline. Mixed-route
group kernel peaks are 2.424–2.447 GiB vector and 3.788–3.915 GiB raster, including
file cache; they are not isolated per-mode pan peaks. Device-wide VRAM/GTT samples
and conservative reservations are separate. The combined preview payload target
remains a soft 8 GiB, including the full 2 GiB GPU reservation charged once; it is
neither a hard process-RSS limit nor an available-VRAM guarantee.

Approximation still fails exact coverage: a duplicate-circle witness has alpha
CPU 199 / Fast 255; a native 128² raster crop loses 313 CPU-positive pixels and
has 735 hidden-RGB differences where both alphas are zero. Opaque screen crops
have different sampling and cannot replace native alpha evidence. Fast has no
Accurate error certificate. Genuine raster 4× exceeds a native strip-buffer
limit; higher physical virtual roots can exceed the configured 4096 blend-root
limit. These are distinct pilot/resource limits, not universal hardware maxima.

Both artworks' native/prepared PNG, semantic SVG and two-frame PNG sequence bytes
match with Fast requested OFF/ON. Vector print rasters match; raster print returns
the same current record-limit refusal in both modes, with no accepted print pixels.
An offline SDK 50 build and static Accurate/Fast preview in an uninstalled
Platform 50 tree pass; Platform memory/guard counters are excluded because its
process moved outside the outer monitor scope. No new installable-package,
bundled-media, other-hardware or energy claim follows from these checks.

## 2026-10-31: personal gallery and dense Pattern previews

The user provisionally accepts the verified TON-002/TON-003 CPU improvements on
2026-10-31, with the documented limits retained. These
checkout-local measurements compare retained pre-change/current release binaries
with matching optimized/debug/link flags, inputs and genuine production
calibration. They do not benchmark a packaged release. The
[scoped report](../target/validation/ton002-ton003-20261031/report.md) retains
commands, source/executable hashes, profiler evidence and raw artifacts.

Personal thumbnail request preparation and evaluation now run entirely off GTK
through one dedicated worker with one queued job. GTK admission is nonblocking;
close/cancellation, catalog generation, fingerprint and weak-card checks reject
obsolete work/results. Warm fingerprint cache reuse remains. Failure to start
this optional worker logs the error and leaves cards/actions usable.

The matched ordinary two-record cold gallery has synchronous append **89.57 →
75.13 ms**, maximum 5-ms heartbeat gap **89.73 → 75.18 ms**. Warm synchronous
append remains **76.99 / 77.40 ms**. Cold image completion is **222.78 →
323.54 ms**: bounded serial work trades throughput for controlled background
resource use. Built-in card/widget work remains on GTK. The historical
multi-second first-open observation is not reproduced by these ordinary current
fixtures. Actual normal-app private checks cover pending Cancel, reopen,
external edits/Refresh, obsolete title rejection and current card selection.

Dense evaluation uses an optional exact mapped scalar plane, local to one
operation and bounded to 16 MiB, with fallible allocation, cancellation and direct
sampling fallback. Lloyd retains global triangulation/neighbor topology and
stable site order, streams one cell through two reused polygon buffers, and
publishes positions only after a complete iteration. Raster normalization frees
consumed raw shapes and inserts normalized contours in the same order into one
fixed-adapter global overlay; temporary raw/positive containers are released
before global graph work. No parallel cell execution or spatial partition is
introduced. Authored density, saturated Poisson population, seed, four weighted
iterations, numerical/certificate/work policy and canonical output remain intact.

| Matched workload | Baseline | Current | Boundary |
| --- | ---: | ---: | --- |
| Vector Feature 0.2, 900×620, complete-command wall | 22.533 s ±0.052 | 17.128 s ±0.030 | Three serial fresh-process Hyperfine runs each; 24.0% lower mean |
| Same repeated runs, mean peak RSS | 2.265 GB | 2.086 GB | 7.9% lower; baseline RSS variation about 73.6 MB |
| Vector 0.2 cold evaluator | 21.654 s | 16.220 s | One pair; warm 1.697 / 1.699 ms |
| Raster 0.2, 1024×1024, cold evaluator | 35.604 s | 26.714 s | One pair; warm 3.218 / 2.804 ms |
| Same raster pair, peak cgroup memory | 3.646 GiB | 3.312 GiB | Single observation, distinct from RSS |
| Vector 0.1, 900×620, cold evaluator | 97.577 s | 76.525 s | One pair; warm 5.997 / 5.825 ms |
| Same finer vector pair, peak cgroup memory | 8.096 GiB | 7.114 GiB | Single observation, 12.1% lower |
| Actual Wizard projection, Feature 0.1, 128×128 | 322.14 ms ±5.63 | 287.08 ms ±1.30 | Three fresh evaluator runs each; 10.9% lower mean |

The small Wizard projection uses the existing production neutral presentation
policy; its timing/pixels are separate from calibrated native Main/Review
consumers. Native per-channel site counts remain exactly 137,419 for vector 0.2,
255,876 for raster 0.2 and 542,680 for vector 0.1. All baseline/current cold/warm
native PNG bytes match in each case, and all six Wizard PNGs match. Independent
pre-change oracles compare complete seeded Lloyd positions/IDs/fingerprints,
geometry/pixels/work/precision and cancellation. Prepared/direct sampler tests
compare exact float bits. Focused tests and strict app Clippy pass.

Both immutable artworks have intrinsic transparent PNG/SVG exports through the
supported current CLI; SVG retains three canonical channel paths and existing
filter composition. Both SVGs are rasterized at natural dimensions for visual
review. Raster CLI PNG equals native Preview RGBA. Vector live text initially
differs under different XDG/font environments; a controlled Preview run under
the identical four disposable CLI roots equals CLI RGBA exactly. This confirms
the documented source-font caveat rather than a general consumer difference.
Private GTK covers the actual personal gallery lifecycle and completed raster 0.2
preview at fitted target 1140×889, with Feature size 0.2 and seed 19 readback.
Parent review inspects raw native PNGs, SVG rasterizations and GTK screenshots.
Requested-card keyboard targeting remains explicitly unverified; existing
keyboard setup is unchanged. These automated Sway checks do not establish human,
screen-reader-user, GNOME Shell or Mutter acceptance.

One early owned vector 0.1 benchmark was OOM-killed by our original **6 GiB test
cgroup**, not a system-wide memory limit. Invalid/partial diagnostics are retained
and excluded from completed benchmark claims. Subsequent workloads use graceful
software memory/headroom/watchdog stops before finite hard cgroup backups,
`MemorySwapMax=0`, and serial execution. Completed vector 0.2 uses soft 3/hard 4 GiB;
raster 0.2 soft 5/hard 6 GiB with at least 16 GiB starting available memory; vector 0.1
soft 10/hard 12 GiB with at least 20 GiB starting available memory. The latter cases
retain a 10 GiB host reserve. All completed pairs record zero OOM/max events and
zero cgroup swap. These are benchmark guards, not new application settings.

The finer native case still takes **76.5 seconds cold and about 7.1 GiB peak
cgroup memory**. These gains preserve quality; they do not establish interactive
latency for every dense native artwork. The user prioritizes the largest safe
gains first. Further changes require measured scope rather than a speculative
renderer redesign. Reproduction semantics are in
[Pattern Wizard checks](PATTERN_WIZARD_TESTING.md).

The authorized [GPU/Vulkan research](GPU_VULKAN_EVALUATION_PLAN.md) now retains
[matched diagnostic timings, full natural RGBA images and zoomed pan regions](../target/validation/gpu-vulkan-20261031/report.md).
Three same-binary0.2 runs give CPU/hybrid/compute medians15.669/4.072/3.983s vector
and26.510/7.700/7.635s raster, using exact canonical scenes/calibration/density.
A separate4x retained-geometry trace paints five new production1536px cache cells:
CPU3.373s versus native hybrid0.177s or refined-circle hybrid0.190s median across
three pixel-cache-cold rounds (best refined17.8× ratio). Refined alpha MAE is
0.259–0.758 bytes across the tested4x views; local errors/union authority remain.
One-time preparation is3.139s refined versus11.269s CPU; native cold comparisons
above do not include this separate refinement. This headless research did not
measure actual GTK upload/continuous pan, and app background caching can hide misses.
Native GPU coverage/tone differs materially; correct model blends/linear transport
and the bounded finer-circle consumer correction distinguish repairable integration
from remaining union/certificate limits. These gains support researching an
explicit approximate live-preview option with precise CPU exports; that research
alone established no product backend, normative accuracy change or perceptual
acceptance. The 2026-11-02 development qualification above covers the subsequent
bounded main-preview implementation.
Authored inputs/calibration and resource reserves remain protected.

## 0.4.6: retained preview regions

The 2026-10-20 followup renders the **whole artwork at each visited eligible
zoom** in the background, using fixed 1536-pixel cells. Visible work receives
bounded priority while other visited zooms continue; panning does not cancel
their demand. The closest already-cached certified scales provide provisional
coverage while requested-scale detail streams in. Retention applies to one
accepted artwork/frame, in memory only. The implementation is user-accepted for
0.4.6. These development checks retain their original source and executable pins;
they do not benchmark the 0.4.6 packages. The frozen
[background-cache report](../target/validation/preview-background-cache-20261020/report.md)
is the current source/runtime entry point; the
[renderer audit](RENDERER_AUDIT_2026-10-20.md) retains earlier repairs and limits.

The engine owns one shared accepted scene, exact-scale roots, sparse pixel
coverage, in-flight deduplication and LRU accounting. One dedicated worker handles
preparation, certificate scans/restoration, painting and upload-data assembly;
GTK owns widgets and textures. Stored pixels alone grant no display permission:
each displayed source scale needs a current whole-source-view certificate under
the unchanged cumulative `1e-6` precision authority, work limits and cancellation.
There is no separate allowance per tile. Fixed canvas-origin grids include
actual GTK sampling support and gutters, with disjoint coverage between scales
and the current-frame full Fit fallback. After an edit at a retained zoom, that
new full fallback is accepted before detail resumes without changing zoom/scroll.
Fractional presentation intentionally resamples this fixed grid; arbitrary old
per-pan sampling-phase byte identity is not promised. Unsupported optimizations
use the ordinary renderer with a separate exact-target memo.

Actual sustained resident motion has 32 held surface captures, 359 distinct
origins and 892 GTK draws at requested physical scale 8, with no new preparation,
painting, pixel work, certificate queries or copied upload bytes. A separate
current-frame cold motion has 32 held captures, 358 crop readbacks and 306 draw
origins while 23 more fixed tiles stream in; visible artwork remains in every
capture. Natural device-1 PNG/SVG controls each have eight held captures, 366
distinct crops and all five visited zooms complete. These establish the tested
motion/reuse behavior, not all-frame correctness, app FPS or an end-to-end
microsecond response. First-traversal texture construction is legitimate upload
work. `copied_upload_bytes` measures CPU texture-data copies, not driver GPU traffic.

Whole-root retention has a generous **8 GiB soft target**.
Accounting includes CPU pixels, prepared geometry/debt, the shared scene and
additional owned materialization, gutters, conservative GTK/GPU copies and
estimated in-flight payload. Active/projected/running roots remain protected;
older unpinned zoom roots evict coherently instead of discarding individual cells.
Oversized roots decline to ordinary viewport rendering. Nothing allocates 8 GiB
upfront, and this is not a process RSS cap: source decode, allocator/backend
overhead and other app allocations remain outside that accounting guarantee.
App restarts and earlier canonical frames are outside retention.

One earlier autonomous headless Spectrum observation fills all 10032² pixels
with one preparation and 49 tiles in **47.225 s**, at **1,675,040 KiB** process
peak RSS. Seven ROI hashes and both natural controls match prior painter output.
This is a single diagnostic observation, not a clean benchmark or UI memory cap.
Its historical probe binary remains separately pinned; final app presentation
repairs do not rerun that painter proof.

### Earlier scoped measurements

The user rejected C2's demand-only cache usability result: its separated,
settled checks do not establish continuous-panning behavior. Its **4 GiB** policy,
31 zero-work settled events and the cache-tuning timings below are historical,
superseded evidence, not acceptance of the current interaction. The reported full
blackout was not reproduced in the frozen 32 held samples; subsequent source
review established and repaired paintable/placement/invalidation defects.
The first repaired 32 `cold-*` captures did not demonstrate sustained panning
and are excluded; the valid current cold proof is `edit-cold-*`.

The following observations have different boundaries and must not be combined:

| Observation | Result | Measurement boundary |
| --- | --- | --- |
| Fresh four-worker XY pilot, Spectrum | 76.7% slower than paired native Direct | Corrected P2 paired raster measurements; the diagnostic pilot remains default-off. This does not measure persistent reuse. |
| Opaque-layer allocation deferral, Spectrum | About 183 MiB lower peak RSS; no material time gain | P3, three fresh-process pairs, before retaining the new cache; not a cached-app RSS saving. |
| Cache tile size 512→1536, Spectrum interior | 28 jobs / 7.63 s → 6 jobs / 2.85 s | Single diagnostic observations; all seven cache-case hashes stay exact. |
| Same tile-size comparison, cold view | 28 jobs / 18.97 s → 6 jobs / 13.93 s | Same diagnostic boundary; excludes source evaluation, copies, encoding, I/O and GTK. |
| Retained same-scale headless lookup | About 4–5 µs | Metadata only; excludes pixel assembly/copy, encoding and GTK. |

Overlap reuse paints exactly 532,288 missing pixels in the recorded case.
Single tile-size observations are not clean paired benchmarks. The
[P2 report](../target/validation/renderer-bughunt-20261020/p2/report.md),
[P3 report](../target/validation/renderer-bughunt-20261020/p3/report.md) and
[C2 tuning observations](../target/validation/renderer-bughunt-20261020/cache-c2/tuning-observations.json)
retain their exact inputs and bounds. No universal speedup, memory saving or
backend superiority follows.

## 0.4.6: certified construction

The 2026-10-19 artistic geometry implementation compares **Complete** construction
with certified **Canvas** materialization of the same new recipe. These are
development measurements of the implementation accepted for 0.4.6, not packaged
benchmarks or a comparison with the pre-refactor implementation. The later historical
sections retain their published 0.4.4/0.4.5 provenance.

Semantic `SiteUsageSet` stays independent of materialized geometry. Authored closed
marks can omit certified off-canvas construction; the fixed straight-grid route
prunes rejected candidate work only. Curves and connections prepare complete
profiles and globally compacted spines before conservative outline pruning. Full
sites, topology, tone lattices, logical work budgets, cancellation and numerical
authority remain. Immutable preparation supports source-free materialization;
cache identity includes coverage and numeric contracts, with a frozen full fallback
when certificates do not apply. SVG restores full pruned **stroke outlines** for
exact external normalization; authored-mark certificates can remain. The fallback
can temporarily retain both restricted and full geometry.

The later precision correction also permits one complete-stroke retry when
antialiased raster preparation of a restricted layer exceeds its checked Boolean
split-certificate budget. It reuses frozen geometry and the same remaining work
allowances, without relaxing precision. A failing case pays for both attempts and
can use more temporary memory; the measurements below do not benchmark that retry.
See the [correction report](../target/validation/preview-area-precision-20261019/fix/report.md).

Fine concentric hatching exposed a second case during final canvas clipping.
When the final operation alone exceeds the layer's area-error limit after the
ordinary recovery path, Accurate rendering can retry that operation with one
quarter of the original grid spacing. It lifts the original integer
coordinates exactly and checks that the result fits the existing error and work
allowances. The reported 3118 × 1808 Fit preview took about two minutes from a
cold launch. A separate headless run spent about 99 seconds generating the scene.
[Fine concentric preview verification](../target/validation/fine-concentric-20261119/ui/report.md).

One warmup and five measured cold/warm timings per case use the same final binary,
counters off. Timed end-to-end work includes native evaluation, scaled PNG, preview,
337×219 streamed PNG and SVG; artifact I/O is outside timing. Separate diagnostics
count actual constructors and reserved storage capacities, which are work/storage
sums rather than live peak memory. Process peak RSS is measured separately.

| Authored-mark workload | Complete peak RSS | Canvas peak RSS | Reduction |
| --- | ---: | ---: | ---: |
| Immutable raster source | 376.0 MiB | 238.8 MiB | about 37% |
| Immutable vector source | 289.7 MiB | 211.8 MiB | about 27% |

Grid results are neutral or slightly slower; other cases are mixed. A bounded
vector-motif repeat increases Canvas peak RSS by about 11%. These data establish
neither universal memory savings nor net pre-refactor performance gains. There
is no GPU acceleration claim. Synthetic transfer fixtures isolate geometry in
these measurements; Phase 5 separately uses genuine strict preparation.

All ten Complete/Canvas pairs have exact native RGBA, scaled PNG, preview, streamed
PNG and independently decoded SVG output; two Inkscape Complete/Canvas pairs are
also exact. The 337×219 stream is not the intrinsic native size. Phase 3 separately
checks its own intrinsic native/stream pairs. Native versus external antialiasing
is not claimed equal. See the [Phase 4 report](../target/validation/artistic-geometry-20261019/phase4/report.md)
for per-case timings, repeats, commands, source identities and retained failures,
and the [Phase 5 report](../target/validation/artistic-geometry-20261019/phase5/report.md)
for genuine preparation and external-consumer isolation/warning limits. Both
immutable artwork inputs retain the SVG live-text/font caveat.

The generic vector-occupancy correction retains one canvas-origin integer frame
through normalization and clipping, each primitive's fill rule, existing scales,
flatten tolerance and strict solver criteria. Checked integer double-area uses the
original physical-canvas denominator. No fit criterion or epsilon is relaxed.
Scoped producer identities `anchored-tone-v1` and `connection-selection-v2` prevent
reuse of affected old fits; current exact fits remain reusable. Tone-spacing
edits retain conservative calibration binding and Realization invalidation, with
copy-on-edit escalation where required. Connection selection changes Family
authority; geometric point spacing remains separate.

The architecture shell guard still flags the unchanged io-to-patterns development
dependency; separate production metadata checks confirm headless boundaries.
No validator or dependency adjustment is part of these measurements.

### Placement-aware coverage follow-up

The coverage-intent fix accepted for 0.4.6 retains certified Canvas construction and
complete off-canvas topology. B1 grid/dead-corner checks prove actual rejected
construction is omitted. Positive-epsilon Maze/straight-Voronoi support uses exact
cache keys where broader reuse would change topology; exact warm reuse remains.
Placement preserves exact saved calibration and response authority without refits;
new producer bindings are genuinely measured before reuse.

The later production matrix has 15 full Complete/Canvas consumer pairs plus the
3118×1916 Main preview at Feature size 0.25, with 61 independently decoded pair
comparisons across native PNG, streaming PNG, SVG consumers and preview. Eight
fixture groups traverse four poses on the same evolving document, including a
combined X/Y/rotation → zero restoration: all retain exact authority, zero new
fits, exact cold/warm pixels and restored-zero pixels. Both immutable source
assets are exercised. The sequential follow-up supersedes the initial Stage C
probe's independent-pose restoration claim.

These are correctness checks, not a new performance benchmark. Main Canvas incurs
three full rematerializations and 1,607,820 outline constructors versus Complete's
1,043,447; ray cases incur 240/361 rematerializations and random connections 3,129.
Fallback can increase work and memory while retaining the existing precision and
work-budget contracts. No universal memory or speed advantage follows.

Checkout-local evidence, absent from release packages: the
[Stage C report](../target/validation/offset-coverage-20261019/implementation/stage-c/report.md),
[parent gate](../target/validation/offset-coverage-20261019/implementation/stage-c/parent-gate.json)
and superseding [sequential roundtrip report](../target/validation/offset-coverage-20261019/implementation/stage-c-roundtrip/report.md).
Native RGBA and hidden RGB are preserved without flattening; external SVG
antialiasing can differ from native output, and SVG live text remains font-dependent.

## 0.4.5: preview recovery and curve capacity

The 2026-10-18 engineering checks cover the accepted 0.4.5 implementation,
collected before its metadata-only version bump from a working tree based on
`798fb5d86032a53636c09eeda58ee1755cc5a800`. The behavior is new since published
0.4.4. This paragraph records the pre-publication measurement context; the
replacement 0.4.5 publication is recorded in [the tracker](../ProgressTracker.md).
The measured pre-bump app SHA-256 is `981696d9001b33994f40c9f994c082e9bea216b57cc338dd2a917d7e331d0827`;
it identifies the measured binary rather than the published 0.4.5 package. The
replacement `v0.4.5` tag and attached build provenance identify the published
release source and artifacts.
Detailed evidence links in this section refer to checkout-local artifacts under
`target/validation/preview-recovery-20261018/`; those artifacts are not shipped.

### Recovery and ownership

Main-preview recovery is shared across Pattern families. The app coordinator
captures workspace/revision/frame identity and the complete exact raster target,
including crop, scale, and backing. Attempt epochs and tickets do not create new
intent. A matching failed request stays terminal before source cloning,
preparation/decode jobs, or engine submission; unavailable viewport allocation
does not clear that failure. Cancellation and stale completions do not become
terminal errors or replace current ownership.

The error overlay does not participate in canvas measurement. It retains the
last accepted pixels, or the blank canvas before the first success, without
changing the raster target. **Retry preview** admits one current attempt;
**Dismiss preview error** hides presentation while the failure remains latched.
A real edit, frame/view change, or workspace replacement can recover normally.
Operation/Save errors keep independent identity through preview changes.
The engine still owns ticket acceptance and atomic cache publication; the app
coordinator owns no document, pixels, or evaluator cache. Retry/Dismiss are real
GTK buttons with native actions and keyboard paths; sensitivity follows current
failure ownership. Full diagnostics remain in the status tooltip and accessible
description.

### Capacity and numerical boundaries

The proven parametric round/square spiral route no longer has the obsolete
4,096-segment construction ceiling or fixed path/clipping count ceilings.
Geometry-owned `CurveWorkLimits` supplies machine-addressable normal policy and
validated nonzero finite bounds for constrained callers. Construction, copying,
transforms, clipping, profile/outline growth, and fragment publication use checked
counts, fallible incremental storage, and cancellation checks. Addressability
checks do not promise that host RAM is available. Both `FamilyCacheKey` and
`DocumentFamilyCacheKey` include the exact policy, so a permissive cached result
cannot bypass a stricter request. Resource policy stays outside canonical content
identity. Unrelated arc-length/intersection policies and numerical bounds remain
unchanged; this pilot does not establish every family or possible failure cause.

The reached raster numeric failures have a separate, narrow correction. At
terminal flattening failure, repeated endpoint handles or an axis-aligned control
hull between endpoints certify an exact chord. A genuine
`raster.flatten.numeric` failure may instead roll back that original transformed
cubic's partial staging and use its chord only when the outward padded control
box fits the existing cumulative `1e-6` output-pixel-area budget per layer.
Actual incurred debt is accumulated upward and charged once, including retained
shapes and holes; consumed work stays charged. Circle potential-loss accounting
remains separate. Nonfinite or excess debt fails atomically. This is a local,
pre-grid winding-area bound, not a universal post-grid error proof. Successful
flatten branches, shared predicates, tolerance, depth, and authored canonical
geometry are unchanged.

SVG contour flattening now subdivides for curvature accuracy rather than span
alone. Lazy cubic-to-quadratic intervals use 10% of the unchanged `1e-8`
canvas-relative tolerance; convex-hull chord certificates use the remaining
90%. Exact certified straight curves emit endpoints. Checked work/storage,
cancellation, primitive normalization, fill/paint order, clipping, and the fixed
Boolean scale remain authoritative. The shared occupancy flattener is unchanged.
There is no global precision reduction, new backend/dependency, or schema change.

The user-flagged square-crossing SVG falls from **56,493,266 to 473,528 bytes**
(about 119× smaller), with line commands reduced from **1,550,716 to 12,806**.
Original baseline files are retained. Only eight recorded spiral SVG goldens
intentionally change; all 14 baseline cases retain exact canonical/raw-f64,
identity, RGBA, and PNG witnesses across both immutable source types. Old/new
SVG rasterizations preserve visually reviewed topology, corners, crossings,
holes, and endpoints, with small edge differences up to 18/255 in alpha or
premultiplied RGB. SVG pixel identity is not claimed. The final transformed
raster correction preserves all 112 frozen small-case artifacts and six original
native artifacts exactly. The [source/capacity report](../target/validation/preview-recovery-20261018/phase-c/final-report.md)
and [transformed-raster report](../target/validation/preview-recovery-20261018/phase-d/bounded-recovery/final-report.md)
retain the scoped tests and equivalence records.

### Observed completion and remaining costs

The unchanged 5016×5016 GoatLocker document completes actual GTK preview in one
observed **7.624788 s** interval from source-job start to observed current-ticket
acceptance, sampled every 100 ms; peak app RSS is **1,743,832 KiB**. It remains
settled for 60.087 s with one source job, submission, and acceptance. Pending
edit/Undo restores clean current acceptance in 1.927547 s without stale
publication; Zoom/Fit and Close/Recent reopen also pass. The original previously
failed, so this is successful completion evidence, not a before/after speedup.
Three cold/three warm small controls show no material regression within their
scoped checks, not a universal performance guarantee.

Actual app native transparent PNG has decoded RGBA identical to headless output,
including hidden RGB; PNG compression/metadata bytes differ. The native SVG is
**98,454,149 bytes**, with four editable paths and no embedded images, and is
byte-identical to repeated headless output. Export completion is distinct from
an external viewer's cost of rasterizing the existing CMYK filter graph.

Independent Inkscape and pinned resvg inspections at 1500×1500 show the expected
composition. Native 5016×5016 resvg inspection succeeds in **49.856 s** at
**11,040,692 KiB** peak RSS under a separately bounded 14 GiB/120 s observer.
Its earlier 8 GiB run was stopped; native Inkscape was stopped after 602.93 s
with an incomplete image and 10,346,768 KiB peak RSS. Neither stopped attempt
counts as success. Native SVG inspection and PNG agree visually, with rasterizer
antialiasing differences rather than pixel identity. librsvg 2.62 produces cyan
and transparency for this file: an external renderer compatibility limit.
Off-canvas filter-atlas clipping is an inference, not a proven specification
defect. See the [renderer review](../target/validation/preview-recovery-20261018/phase-d/svg-inspection/cmyk-renderer-review.md)
and [final product report](../target/validation/preview-recovery-20261018/phase-d/final-report.md),
which supersedes that review's pending native-resvg status.

Measured in-flight geometry/profile/outline cancellation is below the nominal
250 ms target in the recorded samples. Synchronous SVG Boolean processing reaches
**420.392 ms**, and the existing early source-decode cancellation gap remains.
These are explicit dependency limits; no general latency guarantee is claimed.
Private GTK evidence covers distinct-family 60-second failure stability,
pre-ticket routing, Retry/Dismiss, retained pixels/Save errors, and a 480×360
HighContrast/Sans 20 overlay. Four disposable XDG roots and private sockets were
verified, and all owned private units/processes are stopped. These
automated Sway results do not establish human assistive-technology or physical
keyboard acceptance, GNOME/Mutter, portal, native Windows, or physical-print
acceptance.

## Published 0.4.4 performance measurements

The remaining sections describe the 2026-10-16 comparisons and their original
measurement boundaries. Later completion evidence above does not turn those
historical SVG timeouts into successful baseline runs.

### Shared implementation

- **Clipped raster union:** independently normalized positive subjects are
  intersected with the unchanged canvas rectangle in one checked NonZero
  operation, replacing union-then-intersection. The native/unclipped union path
  is unchanged. Circles, authored marks, strokes and regions share this consumer.
- **Accepted-scene reuse:** a private `SceneReuseGuard` compares exact projected
  inputs and strong immutable output `Arc` owners before layer construction,
  full scene validation and hashing. Unchanged preflight, source checks and
  cached-output resource accounting precede lookup. A miss uses the canonical
  constructors and semantic identity path; failed optional guard reservation
  also uses that path. Accepted publication, cancellation and reset remain
  transactional. This is an in-memory shortcut, not a persistent cache.
- **Random/Dispersion storage:** shared ordered buckets use an 8-byte ordinal
  next-link arena and 24-byte head/tail/count cells on the measured 64-bit host. A dense table is selected
  automatically only when checked finite bounds fit within 16 MiB of metadata.
  It retains the original division/floor cell mapping and signed bounds.
  Ineligible layouts, layout arithmetic overflow, failed dense reservation and
  outside-envelope cells use checked sparse storage. Invalid cell coordinates
  retain their existing errors. The bound selects storage; it never caps
  creative density, candidate count or work allowance.
- **Temporary storage:** fingerprints stream precisely the same FNV bytes;
  ordering stores ordinals instead of a copied `Point2` array. Cancellable
  run sorting, heap merge and breadth-first ordering remain intact. Finished
  frontier/index storage is released before ordering.

These mechanisms sit below named Pattern configurations. Future generators
using the same canonical geometry, accepted output or random-site capabilities
can reuse them without acceleration branches keyed by Pattern name. Domain,
source, calibration, project/Preset schemas and canonical scene authority are
unchanged. PRNG/seed behavior, source thinning, site/bucket traversal order,
square-root distance predicates, upfront whole-bucket work charges, guard
population, saturation and diagnostics retain their existing contracts.
Production changes introduce no new dependency, unsafe code, custom hasher,
parallel acceptance or 160-byte `FamilySite` representation rewrite.

### Observed gains

Each row compares that pilot's baseline and candidate; rows are not additive.
PNG/SVG identify the intrinsic source artwork, not an export format. Release
settings and source bytes match within each comparison. OS caches were not
cleared, and no governor, driver or target-CPU settings were changed.

| Workload and measured boundary | PNG source, before → after | SVG source, before → after |
| --- | ---: | ---: |
| P1 fine preview, engine + accepted cache commit | 20.031 → 13.362 s (33.29% lower) | 16.570 → 10.164 s (38.66% lower) |
| P2 warm accepted Maze, engine + cache commit | 312.463 → 10.777 ms (28.99×) | 269.416 → 8.903 ms (30.26×) |
| P2 warm accepted fine, same boundary | 43.406 → 0.056 ms | 30.811 → 0.040 ms |
| P3b fine Even, family only | 2.308694 → 0.908473 s (2.541×) | 0.791541 → 0.352269 s (2.247×) |
| P3b fine Even, complete engine API | 17.914027 → 14.367283 s (19.80% lower) | 11.277476 → 9.535691 s (15.44% lower) |
| P3b fine Even, private GTK ticket acceptance | 19.224120 → 15.473355 s (19.51% lower) | 11.897449 → 9.737625 s (18.15% lower) |

P1 uses three independent alternating process pairs for each fine source. P2
uses three process pairs, each with one engine-cold acceptance and seven warm
requests; reported warm values are medians of process medians. Engine-cold means
an empty engine cache with normal OS caching. P2 introduces no established cold
speedup. Its serialized Scene timings below 1 microsecond mean below reporting
resolution, not zero time. P3b family values use three repeats in one process per
side; complete API values use three independent alternating pairs. Its 2×/15%
stretch and 25%/7% minimum family/API gates pass. Default and focused control
cases show no material regression beyond their defined thresholds.

Private GTK uses two trials per source/binary with alternating order in an owned
1440×1000, scale-1 Sway/OpenGL session. Matching-ticket submission-to-acceptance
is application latency, not compositor paint latency. P1 separately records
21.033 → 13.785 s median PNG acceptance and 18.983 → 11.530 s single SVG
acceptance. These are separate comparisons against earlier binaries.

### Memory, exactness and cancellation

P3b diagnostic allocation windows exclude setup, source decode, hash/report work,
allocator metadata, stack and RSS. Fine Even requested peak capacity falls
11.68%/17.47%, cumulative requested bytes 33.06%/40.12%, and allocation calls
99.977%/99.955% for PNG/SVG. Fine Clustered requested peak falls 37.40% on both.
The retained fine site capacity remains 40/20 MiB. Total process peak RSS is
2112.85 → 2109.98 MiB for PNG but 1188.05 → 1205.38 MiB for SVG (+1.46%);
this work does not establish an across-the-board memory reduction.

P1 preserves 94 compared files, including unchanged native and 8× streaming
paths. P2 preserves 104 files and 222 evaluation identity pairs. P3b preserves
25 independent full-family byte pairs, 42 engine/cache pairs, 62 channel
witnesses and 120 files (18 PNG, 8 SVG, 94 binary witnesses). Failed or timed-out
exports are excluded. Native RGBA, including hidden RGB, remains exact; parent
review inspects native outputs and alpha separately without flattening or a
checkerboard composite. Both immutable sources are exercised; the
[SVG live-text/font caveat](../assets/README.md) still applies.

All 12 externally signalled P3b engine cancellation cases return `Cancelled`
without output. At a 250 ms signal, single measured signal-to-return points are
9.210 → 2.705 ms PNG and 2.075 → 0.010 ms SVG. These are not latency guarantees.
Early SVG signals still wait for the existing uncancellable decode (~180 ms).
Family elapsed-deadline probes are distinct from actual engine cancellation;
work completed before a deadline is recorded as completed.

Injected dense allocation failure verifies sparse fallback. Actual host OOM and
separate arena/sparse allocation failure were not provoked; reservation ordering
was reviewed statically. Focused current tests, strict affected-library/example
Clippy, formatting checks and release app/CLI builds pass. No historical package
or workspace-wide test sweep is claimed. The final 17.17 s incremental build is
verification, not evidence of faster cold compilation.

Private GTK confirms a real pending Feature size edit followed by Undo restores
0.25, clean revision 2, and current acceptance in 0.453038 s; no superseded
revision-1 acceptance is observed. Settled PNG/SVG captures and all six native
fine family outputs are inspected. One progress-overlay capture remains labeled
transient. An initial launcher failure before product startup is retained and
corrected with fixed wrappers. All 14 owned services are independently inactive,
all recorded application PIDs are absent, and all 48 preservation hashes pass.
No GTK control or accessibility schema is added by the performance changes.

### Reproduce current configurations

Run from the repository root. The 12 source-free current-format
[performance Presets](../fixtures/performance/README.md) contain configurations
for Even circles, Clustered links and source-weighted Voronoi, at Feature size
1 and 0.25, for both immutable sources. Loading uses current APIs and rebinds
`assets/raster-sample.png` or `assets/vector-sample.svg` at intrinsic dimensions.
Do not use `prepare` for normal reproduction: it is fixture-creation provenance
and requires retained local TON-018 projects. The accompanying fixture candidates
remove that requirement from current family/engine probes.

```sh
cargo build --release --locked -p toniator-engine \
  --example random_dispersion_probe --example random_dispersion_alloc
mkdir -p target/validation/performance-20261016/p3
mkdir -p target/validation/performance-20261016/p3b
family_out=$(mktemp -d target/validation/performance-20261016/p3b/reproduce-family-png-XXXXXX)
target/release/examples/random_dispersion_probe \
  family png even-random-circles 025 "$family_out" 3
```

Use `svg` for the vector-source fixture, `1` for default Feature size, or one of
`clustered-dispersion-random-links` / `source-weighted-dispersion-voronoi` for the
other recipes. Every invocation needs its own fresh output directory. Both P3
and P3b roots must exist because the current probe canonicalizes both; admission
is hardcoded beneath these roots and refuses reused evidence directories.
Family clocks exclude setup/decode/output hashing; `full-family.bin` and JSON
records supply independent exactness witnesses. The allocation example accepts
the same arguments but is diagnostic-only and must not establish speed claims.
For a smaller engine check that avoids later SVG serialization:

```sh
engine_out=$(mktemp -d target/validation/performance-20261016/p3b/reproduce-engine-svg-XXXXXX)
P3_SKIP_SVG=1 target/release/examples/random_dispersion_probe \
  engine svg source-weighted-dispersion-voronoi 1 "$engine_out" 1
```

The skip is explicitly logged; it does not count as successful SVG export.
Current probes reproduce candidate behavior, not archived baseline speedups.
Original paired binaries, raw records and reports are checkout-local ignored
files under `target/validation/performance-20261016/{p0,p1,p2,p3,p3b}` and the
companion `*-gui` directories. P0/P1/P2 extended matrices require some retained
current TON-018 fixtures; the entire historical matrix is not reproducible from
an otherwise clean checkout. Retain original evidence and use fresh directories.

### Hardware, choices and remaining limits

For the historical 2026-10-16 measurements, the host is Fedora 44,
Ryzen 9 5900X (12 cores/24 threads), 32 GiB RAM,
RX 6900 XT with amdgpu/RADV Mesa 26.2.4, and rustc 1.94.1. Vulkan 1.4 was detected;
that performance work executes on the CPU, with OpenGL used for GTK presentation.
Those checks tested no compute GPU kernel on AMD, Intel, NVIDIA or integrated graphics.
That milestone introduced no new backend, driver setup, environment switch or manual
backend choice. Ordinary CPU execution and automatic dense/sparse selection remain
available without hardware-specific setup.

GPU and sorting options were researched, not implemented in that milestone.
[wgpu documents Vulkan, D3D12 and Metal backends](https://github.com/gfx-rs/wgpu#supported-platforms)
that are candidates for ordinary OS graphics drivers. This is documented
portability, not tested Toniator compute compatibility or a claim that every
device supports a suitable backend. Portable
[WGSL floating point](https://www.w3.org/TR/WGSL/#floating-point-evaluation)
provides f32 and optional f16 rather than portable f64;
[wgpu SHADER_F64](https://docs.rs/wgpu/latest/wgpu/struct.Features.html#associatedconstant.SHADER_F64)
is a restricted native SPIR-V capability, and Vulkan float64 is optional.
Integer Morton ordering could avoid f64, but its measured ~42 ms is under 2% of
the baseline Even family. Device/shader initialization, upload, synchronization
and readback are not justified by that share. CPU storage has measured higher
return. Existing Rayon remains where valid; parallel seeded acceptance would
change ordering. Standard `HashMap` already has SwissTable lineage, so adding
`hashbrown` alone is not an optimization. No new SIMD or sorting dependency is
adopted.

Checked native union/raster work remains expensive: baseline fine Maze takes
17.7/48.3 s. Fine Curve Motif and Clustered SVG serialization reach bounded
120 s timeouts; no exact large-SVG completion is claimed there. Default explicit
calibration fitting remains about 18.382 s. Decode/source sampling, weighted
Lloyd/topology and retained site records remain separate costs. Baseline work
also profiles animation and large 8× export; it does not establish acceleration
for every workload. Automated Sway evidence does not establish human,
GNOME/Mutter, portal, physical-keyboard or physical-print acceptance.

### Credits

The clipped-union change reuses existing iOverlay 9.0.0 checked NonZero
intersection, by Nail Sharipov/iShape-Rust, under MIT OR Apache-2.0. Existing
licenses and pinned commit `26782e3b93f9b7d2516e7653f3d56eebc31af798` are retained
in [vendored provenance](../vendor/i_overlay/PROVENANCE.md). Toniator's existing
`vector_occupancy` positive-intersection approach informs this reuse. P3 copies
no external code and adds no dependency. AGG, tiny-skia, ImageMagick and optional
sorting libraries were research references only; they are not adopted code.
