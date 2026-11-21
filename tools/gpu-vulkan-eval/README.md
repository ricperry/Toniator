# Isolated Vulkan compatibility pilot

This is research tooling, not a production backend. It evaluates two distinct
renderer routes: hybrid `vello_gpu` 0.3.0 and research compute `vello` 0.11.0.
Both use the same upstream commit; compute Area and MSAA16 are AA modes of the
same second renderer route, not additional backend candidates. It is an independent Cargo
workspace and lockfile; it never changes Toniator's workspace dependencies.
Vello GPU is pinned to upstream commit
`72c2fb0628dc6f4bbc7eaf3375284be89be03a06` (package `vello_gpu` 0.3.0).
The upstream 0.1.0 crates.io package is a placeholder, not this renderer.

The fixed 64x64 matrix takes no arbitrary artwork or geometry input. It creates
small public canonical scenes, asks production `write_svg` to apply per-primitive
fill semantics and form eligible opaque compound paths, then submits those exported paths
to native Vello. Certified positive simple contours are concatenated directly; unsafe primitives
retain CPU normalization. This does not perform a global cross-primitive Boolean
union. It directly tests whether native renderer fill/AA gives true geometric
union coverage for the compound path. A failure cannot be repaired by alpha
composition. Requiring explicit pre-union would retain the profiled CPU cost. It does not substitute primitive
AA, max-alpha or source-over alpha for union. The source SVG stays editable.

White witnesses isolate coverage. Native Vello colors are sRGB, supplied through
an explicit linear-to-sRGB conversion. Hybrid `Pixmap::take_rgba8(ImageAlphaType::Alpha)` performs upstream-supported
unassociation for PNG comparison. Compute `fine.wgsl` already unassociates
before `textureStore`, so compute readback passes through unchanged. Raw
unmodified bytes are retained as `.gpu-native.rgba` with per-route alpha
metadata in `results.json`. Do not unassociate compute pixels again. Native source-over color output
is a diagnostic: RGB additive, CMYK transmittance and source-color **linear-light**
composition require integration not present here. Their discrepancies do not
alone establish that such integration is impossible. No custom GPU engine or
model compensation is built. The source/API certificate audit is independent
of byte comparisons: matching pixels cannot prove Toniator's cumulative
error/work/cancellation authority.

The pinned renderer uses 4x4 geometry tiles (`vello_common/src/tile.rs`), so the
seam path crosses many actual x/y tile boundaries. Its fixed quarter-pixel curve
flattening tolerance and f32 coverage data need separate compatibility review.

From the repository root:

```sh
cargo build --manifest-path tools/gpu-vulkan-eval/Cargo.toml --locked -j 2
cargo test --manifest-path tools/gpu-vulkan-eval/Cargo.toml --locked tests::canonical_union_controls -- --exact
cargo test --manifest-path tools/gpu-vulkan-eval/Cargo.toml --locked tests::alpha_discrepancy_is_reported -- --exact
cargo fmt --manifest-path tools/gpu-vulkan-eval/Cargo.toml --check
cargo clippy --manifest-path tools/gpu-vulkan-eval/Cargo.toml --locked --no-deps -- -D warnings
systemd-run --user --scope --unit=toniator-gpu-eval-tiny-20261031 -p MemoryMax=2G -p MemorySwapMax=0 python3 tools/gpu-vulkan-eval/monitor.py --unit toniator-gpu-eval-tiny-20261031 --report target/validation/gpu-vulkan-20261031/b/tiny-guard.json -- tools/gpu-vulkan-eval/target/debug/toniator-gpu-vulkan-eval target/validation/gpu-vulkan-20261031/b/tiny
```

Choose a fresh unit/output path when repeating a run, preserving earlier
artifacts. The guard deliberately binds this host's discrete AMD PCI address
`0000:0b:00.0`; another host must pass its verified `--gpu-pci` explicitly and
review admission limits. The binary rejects software, non-Vulkan and every adapter except the pinned
AMD discrete PCI address. To move hosts, review both binary adapter selection
and guard binding before rerunning. Do not launch multiple timing/profiling workloads. The monitor lock
serializes this tool, but does not discover every unrelated system workload;
review processes and device usage before admission.

The monitor requires an exact per-run owned scope, no cgroup swap, finite hard
backup, sufficient host headroom and dedicated VRAM reserve of max(25%,4GiB).
It samples cgroup/RSS/host memory and dedicated VRAM/GTT. Tiny GPU incremental
VRAM/GTT budgets are 512MiB; host reserve is 10GiB. Polling is a graceful backup,
not a proof against instantaneous allocations; fixed tiny dimensions/path count
and a finite cgroup provide additional admission bounds. GPU driver allocation
is not capped by the host cgroup. Stop on device loss, driver error or reserve
failure; never increase caps automatically or use OOM as a test strategy.

Stage A/B reports and raw evidence are under
`target/validation/gpu-vulkan-20261031/{a,b}/`. Denser artwork workloads and the
conditional Lloyd experiment require the parent technical gate.

To exercise the second route, append `compute-area` or `compute-msaa16` after
the output directory and choose a fresh unit/report/output name. The default
is `hybrid`. Compute uses GPU preprocessing (`use_cpu=false`), one shader-init
thread, only the selected AA mode, and native `render_to_texture`. Readback and
PNG conversion are shared. Fixed sample MSAA can diagnose winding conflation
but cannot establish Toniator's analytic coverage/error guarantee.

Only `final-v3-*` runs in the dated evidence directory use the final corrected
route-specific readback and preserved final binary. `tiny*`, `verified-*` and earlier `final-*` are
retained development diagnostics: early attempts contain documented parser,
source-paint-alpha or compute-unassociation harness errors. Their alpha evidence
remains useful where stated, but their RGB comparisons are not final evidence.

Validate actual readback conventions and cross-route scene/SVG identities with
Python3 plus the already installed Pillow package:

```sh
python3 tools/gpu-vulkan-eval/validate_results.py target/validation/gpu-vulkan-20261031/b/final-v3-hybrid target/validation/gpu-vulkan-20261031/b/final-v3-compute-area target/validation/gpu-vulkan-20261031/b/final-v3-compute-msaa16
```

This checks the exact white quarter-pixel readback control (alpha64), native
alpha metadata, PNG/straight byte consistency and canonical scenes across
routes. It does not turn a visually similar or byte-equal case into a general
area certificate. The final matrix deliberately retains incompatibilities.

## Followup: native blend alternatives

Append `blend` after the route to run the separately authorized fixed blend probe
in a fresh evidence directory. It tests8 geometric controls as compound and
primitive-separated submissions against native SrcOver, Copy, PlusLighter,
Lighten/SrcOver and a global channel/group opacity0.9 contrast. The first4 use
native layer opacity1. Destructive Copy uses supported push_layer/pop_layer,
not hybrid set_blend_mode (which forbids destructive modes).

Color probes separately compare the native sRGB paint carrier and a numerical
linear flat-color carrier, with native RGB PlusLighter, CMYK Multiply/SrcOver and
SourceColorAlpha SrcOver. Numerical-linear mode sends canonical linear numbers
through the nominal-sRGB color container, retains all raw bytes, and applies the
final transfer only after native blending and straight readback. It changes no
coverage, mask or channel equations and is not a new composition engine. This
checks an integration possibility; packed8-bit paint/intermediate precision,
text/images/gradients and exact native model parity need separate validation.

```sh
# Build and freeze a fresh producing binary/source manifest first, then use
# the same guard pattern and a fresh exact unit/report/output directory:
systemd-run --user --scope --unit=toniator-gpu-eval-blend-hybrid-20261031 -p MemoryMax=2G -p MemorySwapMax=0 python3 tools/gpu-vulkan-eval/monitor.py --unit toniator-gpu-eval-blend-hybrid-20261031 --report target/validation/gpu-vulkan-20261031/b/blend-hybrid-guard.json -- tools/gpu-vulkan-eval/target/debug/toniator-gpu-vulkan-eval target/validation/gpu-vulkan-20261031/b/blend-hybrid hybrid blend
cargo test --manifest-path tools/gpu-vulkan-eval/Cargo.toml --locked blend_probe::tests::geometry_authority_controls -- --exact
```

Frozen original final-v3 evidence remains a valid record of the initial route
comparison; the blend followup has its own pins and evidence and may refine the
interpretation of color integration. No isolated per-fixture opacity value grants
compatibility: every mode must also preserve full coverage, duplicates and the
remaining controls under the unchanged authority.

The color followup includes a minimal factorial on identical non-primary paints,
fractional opacity and fractional geometry: default SrcOver versus model-specific
blend, each with native-sRGB versus numeric-linear carrier. SourceColorAlpha
already uses SrcOver. This distinguishes blend-choice benefit from linear
transport benefit. The resulting90 cases per route use separate blend-v2
producing pins; blend-v1 is retained, not overwritten.

```sh
python3 tools/gpu-vulkan-eval/validate_blends.py target/validation/gpu-vulkan-20261031/b/blend-v2-hybrid target/validation/gpu-vulkan-20261031/b/blend-v2-compute-area target/validation/gpu-vulkan-20261031/b/blend-v2-compute-msaa16
```

## Diagnostic performance/full artwork (explicit user exception)

The `perf PROJECT [all|PREFIX] [validate|cold-only]` experiment uses fixed4 Rayon
workers (set `RAYON_NUM_THREADS=4`) and genuine existing saved authority. CPU uses
the actual transparent native AA evaluator; GPU uses public `for_scene` with its
extra1x1 AA-off derived consumer, then the same canonical scene. The direct bridge
rejects noncircle/per-primitive-paint/multiple-output cases and retains all circle
IDs/coordinate/radius bits, including zero and off-canvas inputs. Circle-to-native
Bezier tolerance is1/64 document unit, not coarsened for speed. Native output is
900x620 or1024x1024, with256-byte row padding stripped before PNG encoding.

Without `cold-only`, CPU separately measures actual completed native cache hit,
fresh canonical reraster and public PreviewRoot prepared/certified repaint. GPU
retains the actual device, renderer/pipelines/resources, encoded scene, target
and readback buffer across its warm reraster. Include final transfer and PNG/write
cost in resident comparisons. `cold-only` skips warm controls to provide matched
fresh-process totals and memory footprints. These are fresh process/device runs
with existing driver/file caches; no global cache purge or governor change.

The second renderer's original fixed2^21 line buffer cannot hold the full vector
scene (needs2174102 lines). ONE isolated `vello_encoding` capacity configuration
uses2^23 lines/segment counts/segments (+336MiB allocation), with exact patch,
upstream commit and source worktree under the dated C evidence. No shader, quality,
AA, blend or other buffer changes. The standalone Git patch selects that copied
crate only; production Cargo files remain untouched. The device explicitly
requests supported256MiB storage binding and records actual limits. External
resource budgets do not increase. A separate `validate` process uses upstream's
experimental async counter seam, with no visual overlays, to require failed0
and every counter within actual capacities before standard timed full rendering.
Validation's extra line-buffer download is excluded from performance runs.

The monitor now accepts explicit predeclared `--vram-increment-mib` and
`--gtt-increment-mib`; defaults remain512. All cases preserve4GiB/min25% dedicated
reserve and10GiB host reserve. Phase files distinguish sampled cold/warm memory
from whole-process kernel peak. `/usr/bin/time -v` records whole-process CPU work,
which includes warm controls unless `cold-only` is selected; GPUbusy is global
shared-device diagnostic, not power/energy evidence. No automatic budget rise,
capacity growth after failure, production integration or acceptance follows.

### Reproducing the isolated capacity source after a clean checkout

The Git patch depends on an explicitly prepared ignored source tree. From the
Toniator root, **only when that destination does not exist**:

```sh
(
set -euo pipefail
test ! -e target/validation/gpu-vulkan-20261031/c/capacity-v1-source
git clone --filter=blob:none --no-checkout https://github.com/linebender/vello.git target/validation/gpu-vulkan-20261031/c/capacity-v1-source
git -C target/validation/gpu-vulkan-20261031/c/capacity-v1-source fetch --depth=1 origin 72c2fb0628dc6f4bbc7eaf3375284be89be03a06
git -C target/validation/gpu-vulkan-20261031/c/capacity-v1-source checkout --detach 72c2fb0628dc6f4bbc7eaf3375284be89be03a06
git -C target/validation/gpu-vulkan-20261031/c/capacity-v1-source apply --check ../../../../../tools/gpu-vulkan-eval/patches/vello-encoding-capacity-v1.patch
git -C target/validation/gpu-vulkan-20261031/c/capacity-v1-source apply ../../../../../tools/gpu-vulkan-eval/patches/vello-encoding-capacity-v1.patch
cargo build --manifest-path tools/gpu-vulkan-eval/Cargo.toml --release --locked -j2
)
```

Stop if the destination exists or any command fails. For an existing source,
verify exact base HEAD, exactly the two expected modified files/no unexpected
untracked files, and equality of these patch hashes before building:

```sh
git -C target/validation/gpu-vulkan-20261031/c/capacity-v1-source rev-parse HEAD
git -C target/validation/gpu-vulkan-20261031/c/capacity-v1-source status --short
git -C target/validation/gpu-vulkan-20261031/c/capacity-v1-source diff --binary | sha256sum
sha256sum tools/gpu-vulkan-eval/patches/vello-encoding-capacity-v1.patch
```

Unexpected edits require a fresh separately named source and an explicit updated
manifest/path decision; never overwrite, reset, clean or auto-repair that source.
This source preparation does not install system dependencies or change production
Cargo files. Actual run commands, fixed budgets and producing pins are retained
in the dated C evidence. `validate` uses upstream's experimental allocation API;
its counter/whole-line downloads are separate from ordinary timed rendering.

### Real production-cell panning and radius/phase diagnostics

`OUTPUT cpu|hybrid pan PROJECT` executes the fixed genuine vector0.2 trace at4x
(3600x2480), viewport640x480. It mirrors the pinned private1536 grid enumeration
and reuses public `gutter`, `missing`, intersection and certified CPU root/paint
APIs. Initial cell is cached; four miss events paint five new cells; true hits
perform no renderer calls. Three rounds clear pixel coverage only, retaining
geometry, request debt, GPU curves/strips/device/renderer. CPU whole storage-cell
and whole-view certification are included. GPU native integer camera offset
charges all-scene alpha upload/scheduling, variable target/depth/readback allocation,
synchronization/unassociation/final color transfer. Both charge viewport assembly
and RGBA ownership-copy as an upload proxy. Actual GTK/GDK upload/compositor,
continuous/fractional pan and app background prefill are not simulated. No GTK or
product source changes are made.

Saved native cell/gutter/viewport PNGs remain RGBA unchanged. Fullzoom reference
renders occur outside event timing. Round0 saved images and round2 runtime cells
are compared with each backend's own fullzoom crop; this is camera validation,
not CPU/GPU quality equality. CPU root generation0 remains0. Round1 pixels are
not independently retained. The external evidence audit asserts exact alpha,
visible and hidden RGB crop equality and guard/canonical invariants. Compute pan
is deferred: public camera offset is unavailable and O(N) translated append is
not represented as a resident pan.

`OUTPUT hybrid|compute-area circles` renders18 isolated canonical radius/phase
cases at1x/4x (36 records/route), retaining analyticπr², native Bezier area, CPU/GPU
alpha sums and native sheets. Circle coordinates/radii stay authored document units;
scale changes only the display consumer. Native fixedquarterpixel curve flattening
can lose significant small-circle area despite correct native Bezier area.

The ONE optional `hybrid-refined` circle-consumer variant uses existing
`kurbo::flatten` to convert the same Bezier path into line contours once before
native Scene filling. Bezier conversion target remains1/64 output pixel and
flatten Hausdorff target1/128 output pixel, divided by zoom in document units.
These targets add and are **not** a guaranteed combined1/128 analytic area bound.
It changes no canonical radius/ID/calibration, shaders, backend or blend rules;
opaque overlapping contours still have the native winding/union incompatibility.
An exact counting pass records per-channel path elements/bytes and preallocates
one compound channel at a time. Counting/preprocessing are charged once outside
pan clocks; native hotpath upload/scheduling remains included.

Use `hybrid-refined circles` first, then `hybrid-refined pan PROJECT projection`
for full input path-count/byte admission without native submission, then
`hybrid-refined pan PROJECT 10000` for a prefix resource probe. Prefix output is
explicitly diagnostic partial geometry, never full quality or performance evidence.
Full `hybrid-refined pan PROJECT` requires the recorded parent resource gate;
no automatic budget raise follows failure. All execute under the same external
resource supervisor and fresh output requirement.

`analyze_render_pairs.py CPU.png GPU.png FRESH_OUTPUT` retains exact native100%
RGBA crops plus separately labeled alpha/associated-linear error diagnostics and
signed mean tone bias. It never flattens/composites RGBA or declares perceptual/JND
acceptance. Raw full artwork, alpha, visible color and hidden RGB remain distinct.
The dated `c/report.md`, producing manifests, exact commands and `analyze_pan.py`
record the bounded research findings; the tool does not enable a production GPU
backend or approximate-preview UI.
