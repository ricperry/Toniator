# Measured performance

The shared preview, accepted-scene reuse and Random/Dispersion optimizations
measured on 2026-10-16 are included in Toniator 0.4.4. The staged comparisons
below were collected during development from GUI source checkpoint
`8ee739375313e11c07a325f297a9e91a257ac963` with the respective pilot changes;
they are not new benchmarks of the packaged release. Implementation and scoped
verification are complete. These measurements establish specific workload
improvements, not universal speedups or user acceptance.

## Shared implementation

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

## Observed gains

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

## Memory, exactness and cancellation

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

## Reproduce current configurations

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

## Hardware, choices and remaining limits

The measured host is Fedora 44, Ryzen 9 5900X (12 cores/24 threads), 32 GiB RAM,
RX 6900 XT with amdgpu/RADV Mesa 26.2.4, and rustc 1.94.1. Vulkan 1.4 was detected;
performance work executes on the CPU, with OpenGL used for GTK presentation.
No compute GPU kernel is tested on AMD, Intel, NVIDIA or integrated graphics.
No new backend, driver setup, environment switch or manual backend choice is
introduced. Ordinary CPU execution and automatic dense/sparse selection remain
available without hardware-specific setup.

GPU and sorting options were researched, not implemented.
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

## Credits

The clipped-union change reuses existing iOverlay 9.0.0 checked NonZero
intersection, by Nail Sharipov/iShape-Rust, under MIT OR Apache-2.0. Existing
licenses and pinned commit `26782e3b93f9b7d2516e7653f3d56eebc31af798` are retained
in [vendored provenance](../vendor/i_overlay/PROVENANCE.md). Toniator's existing
`vector_occupancy` positive-intersection approach informs this reuse. P3 copies
no external code and adds no dependency. AGG, tiny-skia, ImageMagick and optional
sorting libraries were research references only; they are not adopted code.
