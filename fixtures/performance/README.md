# Current random/dispersion performance configurations

These12 source-free current-format Document Presets retain the Even circles,
Clustered random-links, and source-weighted Voronoi validation configurations at
Feature size1 and0.25 for the immutable PNG/SVG intrinsic canvases. They derive
from the current retained TON-018 native validation projects, not an obsolete
project adapter or a claim of user acceptance. PNG fixtures preserve3 channels;
SVG fixtures preserve4 channels, along with their authored rotations/seeds and
saved calibration authority. The two source families are distinct real workloads.

Creation uses current `load`, `DocumentConfiguration::capture`,
`save_document_preset` and `load_document_preset`; every configuration compares
exactly after roundtrip. Archives contain only `preset.json`, with no artwork.
The source reference/canvas are rebound by the opt-in engine example to fresh
immutable `assets/raster-sample.png`1024×1024 or `assets/vector-sample.svg`900×620.
The SVG live-text/font caveat in `assets/README.md` applies.

Exact originating project and Preset hashes, current compiler/build/hardware
provenance, release binaries, raw measurements and restoration guards are retained
under `target/validation/performance-20261016/p3/`. The prepare command refuses
existing Presets; it is only a provenance derivation path. Normal reproduction
loads these retained Presets directly and does not need the ignored native projects.

Build only the opt-in examples:

```sh
cargo build --release -p toniator-engine --example random_dispersion_probe --example random_dispersion_alloc
```

A fresh evidence directory must be beneath
`target/validation/performance-20261016/p3/`. Invoke the ordinary binary as:

```text
random_dispersion_probe family png even-random-circles 025 FRESH_P3_DIRECTORY 3
random_dispersion_probe engine svg source-weighted-dispersion-voronoi 1 FRESH_P3_DIRECTORY 1
```

Use a separate fresh directory for every invocation. Family bytes and their SHA
are produced outside timing; `P3_SKIP_SVG=1` skips only later SVG serialization
and logs the omission. The allocation example is diagnostic-only and must never
supply a speedup claim. `p3_support/run.py` supplies bounded serialized process
supervision and the original fixed matrix; its named output directories refuse
reuse, and its referenced frozen binaries live under the P3 evidence bundle.
No runtime dependency/driver installation, GUI session or model download is needed.
