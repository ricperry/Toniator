# Toniator

Toniator is a GPL-3.0-only native Linux creative tool in a greenfield rewrite.
The accepted Stage 9 headless authoritative multi-channel render path is
checkpointed at `67e831a`, after the Stage 1 foundation, authoritative document
boundary, deterministic guide family, source sampling, rendering, scheduling,
cache, channel-model, compositor, and CLI integration stages. It provides
authoritative RGB, CMYK, and SourceColorAlpha PNG/SVG rendering over both
baseline sources, with native review artifacts under `target/validation/`.

Stage 10's accepted view-only GTK/libadwaita preview is checkpointed at
`980af50` and opens with
`toniator-app [PATH]` (zero or one local PNG/SVG path) or its Open action. PNG
decoded dimensions and SVG intrinsic/`viewBox` dimensions define the
authoritative canvas and aspect; the app has no canvas override. A visible
selector switches among RGB, CMYK, and SourceColorAlpha evaluation. Evaluation
is asynchronous, and only source loads and completions accepted for the
current document revision are presented, so stale work cannot replace the
preview.

The preview rerasterizes the unchanged canonical scene to the fitted viewport,
clips to the transformed intrinsic canvas so guard geometry cannot leak into
letterbox margins, and presents the exact straight raw RGBA raster without
PNG encoding, premultiplication, flattening, checkerboarding, or channel
recomposition. Persistence, command-bound editing, and later app/CLI export
controls remain planned.

Stage 11 headless undo and redo are complete at checkpoint `341ad8e` through
`DocumentHistory`, an accepted wrapper around `DocumentSession` that stores
exact validated document snapshots and preserves monotonic revision authority
with stale-result rejection after apply, undo, and redo. GTK undo controls and
history persistence remain planned.

Stage 12 portable `.toniator` persistence is complete at checkpoint `dd7ca56`.
The headless `toniator-io` boundary writes and loads deterministic version-1
ZIP containers containing the complete supported document and the exact
embedded PNG/SVG source bytes. The CLI supports `document create`, container
`validate -i`, and container `render -i`; direct-source behavior remains
available. Loading reconstructs a fresh document/history at revision zero,
and history, dirty state, and filesystem source paths are not serialized.
The GTK app remains view-only and its Open action accepts direct PNG/SVG
sources; GTK container New/Open/Save/Save As lifecycle remains planned for
Stage 13A.

The headless Stage 9E direct-source CLI still requires an explicit
`--canvas`; source-native direct-still sizing and PNG antialiasing controls
remain planned.

## Build and run

On Fedora, install the GTK4 development files (GTK 4.10 or newer), libadwaita
development files (libadwaita 1.4 or newer), and `blueprint-compiler`; these
are required by the `toniator-app` dependencies and build script. Then launch
the preview with:

```bash
cargo run --bin toniator-app -- assets/raster-sample.png
```

The approved execution roadmap is [GREENFIELD_REWRITE_PLAN.md](docs/GREENFIELD_REWRITE_PLAN.md),
and the current checkpoint ledger is [ProgressTracker.md](ProgressTracker.md).
The normative design is in [Architecture Schema](Project%20Specification/ArchitectureSchema.md),
[Pattern Schema](Project%20Specification/PatternSchema.md),
[Channel Schema](Project%20Specification/ChannelSchema.md),
[Module Structure](Project%20Specification/ModuleStructure.md), and the
precedence-setting [Addendum](Project%20Specification/Addendum.md).

The headless CLI and GTK app are separate peer frontends over the shared
`toniator-engine` boundary; neither frontend owns document or pattern state.
