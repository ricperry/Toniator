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
embedded PNG/SVG source bytes. Canonical v1 saves contain exactly
`document.json` and one embedded source entry in normalized, uncompressed
Stored form; the reader also tolerates Deflated required files and one exact
empty `sources/` directory marker from a benign manual repack. Other topology
or compression remains invalid. The CLI supports `document create`, container
`validate -i`, and container `render -i`; direct-source behavior remains
available. Loading reconstructs a fresh document/history at revision zero,
and history, dirty state, and filesystem source paths are not serialized.

Stage 13A GTK document lifecycle is complete at checkpoint `36c7b44`.
`toniator-app [PATH]` accepts zero or one local PNG, SVG, or `.toniator` path at
startup. New creates an untitled, unsourced document; Open accepts direct
PNG/SVG artwork or a `.toniator` container; Save and Save As write `.toniator`
documents (direct artwork uses Save As); and Close plus window close share a
Cancel/Discard/Save confirmation when work is unsaved. The app-owned workspace
keeps the headless history and immutable source bundle, while dirty state
compares the exact current document plus source-bundle content and identity
with the accepted savepoint rather than revision numbers, so undoing to saved
content and semantic no-ops are clean. Atomic save failures preserve the
current content, history, location, title, and dirty state; successful saves update the
location, title, and savepoint only after IO succeeds. Load/save errors and
generic migration information are reported in-window. GTK delegates default
document construction to the headless factory and remains ignorant of pattern
internals; channel/pattern controls and GTK undo controls remain out of scope.
The separately accepted app-only reentrancy correction at `02bc2c9` preserves
this lifecycle behavior while preventing nested model-selector and window-close
callbacks; it is not part of the Stage 14 schema checkpoint.

Stage 13B is complete at checkpoint `2a773a3`. Direct-source `toniator render`
uses decoded PNG dimensions or resolved SVG intrinsic/`viewBox` dimensions by
default; an explicit `--canvas` remains a direct-source-only override, while
containers keep their stored canvas. PNG output accepts `--antialiasing
on|off` (default `on`); `off` is intentionally hard-edged, and SVG output is
unaffected.

The GTK Export action writes native PNG or editable SVG. PNG options are
consumer-only transparent/black/white backing, antialiasing, and an optional
`WIDTHxHEIGHT` output target; SVG remains transparent semantic vector output.
Output targets are checked against the renderer's allocation safety limit.
Exports rerasterize the unchanged canonical scene and do not resize the
authoritative document or preview canvas, or mutate document, source,
history, revision, savepoint, location, title, dirty state, or preview state.

The accepted native app-test outputs use source-native aspect and resolution
(1024×1024 raster, 900×620 vector). This corrected the earlier tiny-output
moire diagnostic; AA-off edge stepping remains the intentional hard-edge
consumer policy. The checkpoint records automated GTK snapshot coverage and
native artifact inspection; it does not claim exhaustive manual dialog,
accessibility, or interactive GTK acceptance.

Stage 14 is complete at implementation checkpoint `88fc6dd`. The headless
domain now owns one-root typed pattern definitions with document-wide stable
definition, mechanism, and output-layer IDs and deterministic ordering.
`DocumentHistory` is the sole authority for atomic definition add, duplicate,
retarget, unreferenced removal, selected-channel copy-on-edit, and explicit
shared-definition edits, including exact stale-base, invalidation,
affected-channel, undo, and redo behavior. The immutable private v1 parser is
unchanged: container layout remains v1, loading dispatches through the v1 DTO
and deterministic v1-to-v2 migration, and current or migrated documents save
as schema v2 only. Embedded source bytes remain exact, and the supported typed
configuration preserves accepted RGB, CMYK, and SourceColorAlpha geometry,
raster, PNG, and editable SVG parity across both frozen v1 containers and
equivalent v2 documents. Native artifacts were inspected as raw RGBA and
editable SVG; this does not claim exhaustive manual GTK dialog,
accessibility, or interactive acceptance. Stage 15 generalized evaluation and
GTK pattern-editor architecture remain planned.

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
