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
internals; at this checkpoint, channel/pattern controls and GTK undo controls
remained out of scope.
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
accessibility, or interactive acceptance.

Stage 15 generic pattern evaluation is complete at implementation checkpoint
`711058b`. Typed capability validation now resolves the supported family,
explicit modulation, ordered output realization, canonical geometry, and
final-consumer clipping without renderer-owned pattern dispatch. Stable
definition, mechanism, and output-layer provenance is retained; family,
realization, source/decode, scene, and raster identities now invalidate and
reuse at their respective boundaries. Structural planning is bounded by
candidate limits and cancellation, unsupported combinations fail before
partial output, and scheduler cache publication remains transactional for
accepted current work. The supported straight-guide/intersection/circular
configuration preserves exact geometry and RGB/CMYK/SourceColorAlpha output
parity. Frozen v1 containers migrated, saved as v2, reopened, and rendered
with byte-identical PNG/SVG results across all three models; v1/v2 persistence,
`DocumentHistory`, renderer, app, and GTK lifecycle behavior remain unchanged.
Native artifacts and bounded app launches are liveness/review evidence, not
exhaustive manual visual, interactive, or accessibility acceptance. Stage 16A
was provisionally accepted on 2026-08-09 and is implemented at checkpoint
`ccec466`; Stage 16B is complete at checkpoint `77bad7c`, and Stage 17 is
complete at checkpoint `e777270`.

Stage 16A generalizes the typed straight-guide family through the same generic
headless pipeline. A definition may contain one to four ordered straight-guide
dimensions with document-wide stable IDs, independent finite baseline angles,
phase and repetition state, and the shared channel transform. Explicit
intersection products evaluate selected dimensions, deterministically merge
coincident multiway intersections, and retain every contributing guide. Along-
guide products use regular arc-length sampling over an explicit dimension
selection and retain stable guide, sequence, absolute/local arc-position, and
Canvas/Guard provenance. Both products are typed, addressable mechanisms; no
consumer reconstructs guides or sites.

Typed circle mark prototypes and fixed or contributing-guide orientation rules
are validated as part of realization identity without renderer pattern dispatch.
Analytical transformed coverage, guard support, checked candidate limits,
cancellation, transactional scheduling, and family/realization/scene/raster
cache boundaries remain generic. The accepted two-guide/intersection/circle
configuration retains its Stage 15 geometry and RGB/CMYK/SourceColorAlpha
parity. The private immutable v1 parser and migration remain unchanged; new
definitions use only additive current-v2 DTO variants and deterministically
save and reopen without evaluator or cache state.

Stage 16B adds a typed `RandomSites` family through the same headless path. Its
ordered chain combines raw-uniform, genuinely even, or clustered base
processes; uniform or artwork-weighted density modulation (Linear or
Smoothstep); and minimum-center or visible-mark exclusion before circular mark
realization. A deterministic xorshift32 stream consumes each authored `u32`
seed, and accepted sites retain stable candidate/accepted ordinals with
Canvas/Guard provenance. Diagnostics report requested versus achieved sites,
candidate/rejection counts, and scope counts under bounded candidate,
neighbor-work, and cancellation policies.

Source identity is intentionally narrow: only artwork-weighted structure uses
decoded content and pixel identity; source-independent random families remain
source-free, while logical source references stay at decoder lookup. New
families use additive current-v2 persistence variants while the immutable v1
parser/migration and existing v2 forms remain preserved. Random variants reuse
the shared canonical geometry, clipping, preview, PNG, and SVG output pipeline.
High-density natural-resolution validation covers the 1024×1024 PNG and
900×620 SVG baselines, including raw/native artifact and save/reopen parity.
Automated checks, raw artifact inspection, CLI parity, and bounded app launches
are evidence only; no separate manual visual, interactive, or accessibility
acceptance is claimed.

Stage 17 headless editing authority is complete at checkpoint `e777270`.
`DocumentHistory` accepts typed commands for supported channel properties and
structural edits, validates each transition atomically, reports deterministic
affected channels, and returns the earliest applicable invalidation level:
Presentation, Realization, Family, Source, or ChannelTopology. Schema-derived
read-only descriptors expose stable typed field IDs, value kinds, choices,
bounds, units, dependencies, support, and invalidation metadata without owning
values, validation, serialization, or UI behavior. The generic
`toniator capabilities [--input PATH]` command emits those descriptors in
deterministic order. Stable-ID allocation, copy-on-edit and explicit shared
editing, undo/redo, persistence/CLI parity, cache reuse, and restored canonical
render output are covered by the accepted headless tests. GTK channel controls
are implemented in the descriptor-driven Stage 18 inspector below.

Stage 17A and Stage 18 are complete at checkpoint `2a85252`. Stage 17A adds
immutable transient compound-variant drafts for random character,
density-modulation, exclusion-policy, and guided-orientation changes. Drafts
derive complete typed payloads from domain contracts, require visible explicit
confirmation, finalize only existing typed edits, and remain outside descriptors,
document/history state, persistence, cache/evaluator inputs, and frontend state.

Stage 18 adds a descriptor-driven selected-channel GTK inspector. It reads
current values through a separate immutable typed reader, selects and falls back
by stable channel ID, renders generic controls with progressive disclosure, and
routes selected copy-on-edit or deliberate shared editing through
`DocumentHistory`. Compound transitions show their domain draft before
confirmation; invalid, rejected, and semantic no-op drafts preserve the draft,
status, document/history state, and last-successful preview. Selection, drafts,
disclosure, status, and focus remain runtime-only. Existing lifecycle,
v1/current-v2 persistence, and canonical preview/PNG/SVG output behavior are
preserved, including guarded asynchronous preview updates.

Automated and static checks, raw native output inspection, and bounded app
evidence do not claim actual GNOME/Wayland manual visual, keyboard/focus, or
assistive-technology acceptance; that review remains outstanding.

Stage 19A is complete at implementation checkpoint `9919d85`. The headless
preset registry is version 1, with standalone serialized records using
`preset_format_version: 1`. Its stable bundled order is
`even-random-circles` followed by `straight-grid-circles`; IDs, names,
categories, descriptions, and thumbnails are metadata only and never enter
document evaluation, cache identity, or renderer dispatch. Applying a record
reconstructs an ordinary typed pattern definition through the existing typed
command and Stage 17A draft authority. Selected-channel application creates an
independent document-owned definition; explicit shared replacement first
discloses the ordered affected channels and then requires confirmation against
that scope. The registry does not add GTK preset controls.

Every bundled record is serialized and reloaded, reconstructed independently,
and checked for canonical PNG/SVG parity at the natural 1024×1024 raster and
900×620 SVG dimensions. The strengthened RGB-independence evidence applies
different preset definitions to independent channels, verifies stable channel
and definition identities, proves an isolated red edit leaves green/blue
definitions, identities, isolated PNG bytes, and visible geometry unchanged,
and records the modeled SVG document-identity metadata caveat. Document schema v2,
`.toniator` container v1, and the immutable v1 parser/migration remain
unchanged; current and migrated documents continue to save as schema v2.
The accepted channel-selector correction is included in `9919d85`: it uses a
persistent `StringList`, `splice`, deferred rebuild, and invalid-position
rejection. This is a bounded GTK selector correction, not GTK preset UI.

Low-resolution fixtures and outputs are supplementary only. Every future stage
that exercises source loading, sampling, rendering, preview, or export must
also test the immutable PNG at its natural 1024×1024 dimensions and the SVG at
its natural 900×620 dimensions through the applicable canonical consumer
boundary.

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
