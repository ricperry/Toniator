# Toniator Greenfield Rewrite Plan

Status: approved execution roadmap and stage contract (2026-08-05)

This plan is subordinate to the normative files in `Project Specification/`.
It records the approved order, scope, and gates for the rewrite; it does not
replace product architecture or authorize work outside a named stage.

## Authority and document roles

- `Project Specification/ArchitectureSchema.md`, `PatternSchema.md`,
  `ChannelSchema.md`, `ModuleStructure.md`, and `Addendum.md` are normative
  product architecture. `Addendum.md` supersedes conflicts.
- `docs/GREENFIELD_REWRITE_PLAN.md` is the approved execution roadmap and
  stage contract.
- `ProgressTracker.md` is the current status and checkpoint ledger.
- `.codex-work/` contains local, checkout-aware evidence only; it is never
  durable authority and cannot advance a stage.

## Non-negotiable constraints

- Keep one authoritative `Document`/`DocumentSession`. Commands mutate that
  authority; widgets, evaluators, caches, previews, and exporters are derived.
- Keep GTK/libadwaita in `toniator-app` only. `toniator-cli` is a peer,
  headless frontend using `toniator-engine`; no headless crate depends on a
  frontend or GTK.
- Preview, PNG, and SVG consume one canonical geometry/render-scene path.
- Pattern families own guide and site generation. Voronoi only constructs
  ordinary cells from family-generated sites.
- Generate complete off-canvas structure, including guard coverage, then clip
  final canonical geometry to the canvas. The canvas never creates topology.
- Presets are ordinary exposed-control configurations, never preset-name
  renderer branches. Use `f64` for continuous authored values and discrete
  integer/ID types for seeds, counts, indices, and topology identifiers.
- Commands report the correct invalidation level; evaluators carry revisions
  and reject stale results.
- Animation is limited to simple start/end transitions with one interpolation
  mode. Do not introduce runtime plugins or broad legacy compatibility.

## Workspace responsibilities and dependency direction

The nine crates are one downward core plus a shared orchestration boundary and
two frontends:

| Crate | Responsibility | Dependency rule |
| --- | --- | --- |
| `toniator-domain` | Authoritative document, channels, pattern references, commands, validation, IDs, revisions/contracts. | Lowest layer; no GTK, renderer, persistence, or geometry algorithms. |
| `toniator-geometry` | Primitives, transforms, bounds, guides, intersections, sites/provenance, topology, clipping, canonical marks/paths/regions, ordinary Voronoi, reusable region offset. | May use domain types where unavoidable; no frontend, persistence, or exporter. |
| `toniator-sampling` | Source decoding boundary and deterministic fields, interpolation, point/path/region sampling, response curves, polarity, weighted distributions. | May use domain and low-level math/image libraries; no GTK, pattern registry, or output encoding. |
| `toniator-patterns` | Pattern schema, family generation, density interpretation, guide/site output, mark/connected/region realization, modulation and coverage coordination, validation, presets as schema data. | Uses domain, geometry, and sampling; never GTK, file dialogs, SVG/PNG, or writable document state. |
| `toniator-render` | `RenderScene` and layer consumption, preview, raster/PNG, SVG, diagnostics/debug overlays. | Uses domain and geometry; never generates sites, reads widgets, or owns state. |
| `toniator-io` | Document/preset serialization, migrations, recovery/recent metadata, source references, export coordination. | Uses domain (and render for export coordination); no pattern mathematics or UI binding. |
| `toniator-engine` | Shared headless orchestration: immutable snapshots, evaluation requests, invalidation/revision scheduling, source-to-pattern-to-render pipeline, cancellation/cache boundaries. | Depends on domain, sampling, patterns, render, and IO; no GTK. This is the common boundary consumed by both frontends. |
| `toniator-cli` | Deterministic command-line frontend (`validate`, then inspect/render and later commands), exit codes, arguments, artifact inspection. | Peer frontend; depends on engine/domain as needed; never GTK. |
| `toniator-app` | GTK/libadwaita application, Blueprint/GResource resources, controllers, view models, command bindings, preview presentation, task coordination. | Peer frontend; consumes engine/domain/IO and owns all GTK concerns. |

The intended flow is `domain → geometry/sampling → patterns → render/io →
engine → app or cli`; engine is the shared orchestration boundary, not a
second state authority.

## Baseline test artwork

The tracked files `assets/raster-sample.png` and `assets/vector-sample.svg`
are the project-wide source-artwork baselines. Relevant source loading,
sampling, rendering, preview, and export stages must exercise both files in
addition to any smaller synthetic fixtures. The PNG is a 1024×1024 RGBA image
with nontrivial alpha. The 900×620 SVG contains gradients, transparency, a
stroked path, and a live `<text>` element.

Keep these inputs byte-stable and write derived output under
`target/validation/`. Their verified properties and SHA-256 values are
recorded in `assets/README.md`. Replacing either baseline requires explicit
approval and synchronized plan, fixture-integrity, and test updates. SVG tests
must verify live-text handling, but must not use font-dependent exact raster
pixels as portable goldens until the test provides a deterministic font.

## Working method and Git gates

Work one short stage at a time. The parent names the exact allowed files and
non-goals. The writer runs focused tests, CLI checks, and artifact inspection
appropriate to that stage, then reports. The parent reviews implementation and
evidence; the user provides acceptance; only then may the parent make an
explicit checkpoint commit. Stop before the next stage. Never push in this
workflow.

Use these statuses exactly:

- **Planned** — scoped but not authorized or started.
- **In progress** — parent has authorized the bounded stage.
- **Implemented awaiting review** — writer finished; parent review remains.
- **Accepted awaiting checkpoint** — parent review and user acceptance are
  complete, but the checkpoint commit is pending.
- **Complete at commit `<hash>`** — the accepted work is present at the named
  local checkpoint.

Tracker transitions must never claim accepted or complete before actual user
acceptance and (for complete) the checkpoint hash. Baseline is `11c2c8e`;
Stage 1 is committed at `567d307`; Stage 2 is complete at `e842a8a`.

## Completed stages

### Stage 0 — baseline relocation/spec checkpoint

**Complete at `11c2c8e`.** Established the greenfield rewrite baseline and
protected normative specification inputs.

### Stage 1 — nine-crate foundation

**Complete at `567d307`.** Created the nine-crate workspace and dependency
guard, headless CLI and app shells, project guidance, and architecture checks.
No document model, geometry, sampling, render, persistence, GTK resources, or
exports were shipped in this checkpoint.

### Stage 2 — authoritative document and invalidation boundary

**Complete at `e842a8a`.** Added validated authoritative in-memory domain
state, stable IDs, continuous `f64` layout/appearance values, validated commands and invalidation
levels (`Presentation`, `Realization`, `Family`, `Source`), immutable
`Document::apply_command`, `DocumentSession` revision ownership, and stale
evaluation-token rejection. Added headless `toniator validate` and nine
integration tests (four domain, three engine, two CLI). Verified workspace
format/check/clippy/tests, architecture validation, valid and invalid CLI
paths, and protected-spec/Legacy diffs. Stage 2 intentionally has no geometry,
rendering, persistence, source decoding, async evaluation, or GTK.

## Stage 3 — straight-guide family output

**Status: Complete at `f60eb65`.** The accepted bounded family-output slice is
implemented and checkpointed. It provides deterministic headless output for
two rotated/translated straight-guide dimensions, analytical off-canvas guard
coverage, intersection-site provenance/fingerprint, and the `inspect grid`
JSON path. Verification passed the focused and workspace tests, strict
Clippy/checks, architecture validation, and canonical JSON comparison. User
acceptance is recorded, but point-site correctness was not visually confirmed
on a plotted canvas because no visible-output stage exists yet. Marks,
rendering, and Stage 4 remain unimplemented. The historical contract below
remains the bounded scope; do not begin marks or rendering from it.

### Objective and invariant

Produce deterministic output for two straight guide dimensions and their
intersection sites over analytical, off-canvas coverage. Families own guides
and sites; the canvas only plans extent and performs final clipping, never
edges or topology.

### Allowed scope

The explicit path allowlist is:

- `crates/toniator-domain/Cargo.toml`, `src/**`, and `tests/**` for the
  domain-side pattern-schema additions.
- `crates/toniator-geometry/Cargo.toml`, `src/**`, and `tests/**` for
  primitives, transforms, bounds, straight guides, intersections, sites, and
  coverage.
- `crates/toniator-patterns/Cargo.toml`, `src/**`, and `tests/**` for grid,
  density, and family output.
- `crates/toniator-engine/Cargo.toml`, `src/**`, and `tests/**` for inspect
  orchestration.
- `crates/toniator-cli/Cargo.toml`, `src/**`, and `tests/**` for `inspect`.
- `Cargo.toml` and `Cargo.lock` only when a dependency-edge update is required
  by the bounded implementation.
- `fixtures/canonical/**` for the Stage 3 sorted-sites golden.
- `ProgressTracker.md` and `.codex-work/agents/**` evidence for the proposed
  transition and checkout-aware findings.

Explicitly exclude `Project Specification/**`, `ToniatorLegacy/**`,
`assets/**`, `README.md`, `AGENTS.md`, all other crate directories, and all
agent/skill governance files. No path outside this allowlist is in scope.

### Forbidden scope

No marks, source sampling, PNG/SVG/rendering, GTK, curves, random processes,
graphs, regions/offset, Voronoi, animation, plugins, persistence, migration,
or broad legacy compatibility.

### Settled Stage 3 decisions

- Use a 900×600 document with 90×60 density as the fixed planning reference:
  nominal spacing is 10 document units in both axes.
- Generate two stable-ID straight dimensions. Rotate about the canvas center,
  then apply document-axis X/Y translation.
- Derive periodic phase modulo spacing while retaining authored translation in
  state. Plan with `guard_steps = 2`, support radius `4.5`, and any additional
  antialiasing/support margin required by validation.
- Inverse-transform the padded canvas; project its corners onto each guide
  normal; enumerate inclusive floor/ceil guide indices; extend each line across
  the local domain. Never create segments from canvas edges.
- Emit deterministic guide/intersection sites with provenance and a stable
  family fingerprint. A comparison against an independently generated broader
  lattice must show every support-intersecting site is present.

### Required CLI acceptance

```bash
cargo run -p toniator-cli -- inspect grid --canvas 900x600 --density-x 90.0 --density-y 60.0 --rotation 17.0 --offset-x 3.25 --offset-y -4.5 --guard-steps 2 --support-radius 4.5 --format json
```

Write the command's JSON to `target/validation` and compare sorted output to
`fixtures/canonical/stage-3-sites.sorted.json`.

### Stage 3 tests and verification

Test spacing and directional frequency; inverse transform and coverage;
deterministic guides/intersections/provenance/family fingerprint; independent
broader-lattice completeness; rotations `0`, `17`, `45`, `89.5`, `137`; zero,
positive, negative, and multi-period translations; rejection of nonfinite or
invalid inputs at the boundary; and absence of nonfinite geometry. From the
repository root, use this validation block (it verifies that the repository's
Python provides the preferred deterministic JSON sorter before using it):

```bash
set -euo pipefail

cargo fmt --all -- --check
cargo test -p toniator-domain
cargo test -p toniator-geometry
cargo test -p toniator-patterns
cargo test -p toniator-engine
cargo test -p toniator-cli
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
bash scripts/validate_architecture.sh

python3 -m json.tool --help | grep -q -- '--sort-keys'
mkdir -p target/validation
cargo run -p toniator-cli -- inspect grid --canvas 900x600 --density-x 90.0 --density-y 60.0 --rotation 17.0 --offset-x 3.25 --offset-y -4.5 --guard-steps 2 --support-radius 4.5 --format json \
  > target/validation/stage-3-sites.json
python3 -m json.tool --sort-keys fixtures/canonical/stage-3-sites.sorted.json \
  > target/validation/stage-3-sites.fixture.sorted.json
python3 -m json.tool --sort-keys target/validation/stage-3-sites.json \
  > target/validation/stage-3-sites.sorted.json
diff -u target/validation/stage-3-sites.fixture.sorted.json \
  target/validation/stage-3-sites.sorted.json

git diff --check
git diff --exit-code -- ToniatorLegacy 'Project Specification'
git status --short -- ToniatorLegacy 'Project Specification'
git status --short
```

Report the implementation and evidence, propose the tracker transition
**Implemented awaiting review**, leave all work uncommitted, and stop. Do not
start marks, rendering, or Stage 4.

## First complete vertical slice (later short stages)

### Stage 4 — source sampling and circular mark realization

Add deterministic source sampling and circular mark realization plus canonical
mark geometry; do not add a renderer. Ink amount is `1 - luminance`; authored
diameter is linear `2.0..9.0`; store canonical radius. Color and opacity remain
presentation. Prove shape-size changes reuse the same Stage 3 sites. Exercise
both tracked baseline inputs through the shared source-field boundary: verify
PNG alpha semantics and SVG live-text handling under an explicit font policy.

### Stage 5 — RenderScene and preview/export consumers

Consume the same `RenderScene` from a shared headless `RasterSurface` (for
future preview and PNG) and SVG writer. Add CLI render with output-extension
selection, RGB PNG black background, CMYK PNG white background, and an explicit
transparent option. Clip only at final output. Use both baseline inputs for
cross-consumer parity checks and keep generated artifacts outside `assets/`.

Use this fixed reference: 900×600, 90×60, rotation 17°, offsets 3.25/−4.5,
channel color `#00b7ff`, opacity `0.72`. Inspect artifacts with `identify`,
`xmllint`, Inkscape SVG rasterization, ImageMagick RMSE `<= 0.02`, and visual
side-by-side inspection before accepting goldens.

Historical note: Stages 4 and 5 share implementation checkpoint `31f4cc9`.
Stage 5 visual review prompted the accepted Stage 4 alpha-associated luminance
correction. The accepted outputs remain derived validation artifacts under
`target/validation/`, not committed binary goldens.

The slice excludes curves, random, maze, Voronoi, regions/offset, video,
animation UI, plugins, and legacy import. GTK editing remains a later stage.

## Stage 6 — authoritative document evaluation

**Status: Planned.** Replace the Stage 5 CLI's ad hoc render DTO as the primary
pipeline entry with synchronous evaluation derived from one immutable
authoritative document snapshot.

### Stage 6 public contracts

- Add an assigned `SourceReferenceId`, retaining `Unassigned`.
- Extend `PatternDefinition` with the supported structural straight-grid
  definition and circular-mark output declaration.
- Add channel source mapping for `Luminance` or `Alpha` with
  `StretchToCanvas`.
- Constrain the supported circular-mark diameter to `2.0..=9.0` at the domain
  boundary.
- Add `DocumentSession::evaluation_snapshot(channel_id)`, atomically pairing a
  document clone with its revision token.
- Add an engine request containing that snapshot and matching resolved source
  bytes.
- Derive guard depth from the pattern definition and support radius from the
  channel's maximum mark diameter.
- Return the token, decoded source identity, immutable `RenderScene`, and
  raster preview.
- Route CLI `render` through this boundary. Add `-i`/`--input`, retaining
  `--source` as an alias.

### Stage 6 scope

Allowed: domain, sampling type relocation/re-export, patterns, engine, CLI,
focused tests, Cargo manifests, architecture validation, future plan/tracker
text, and derived Stage 6 artifacts.

Forbidden: async work, caches, GTK, persistence, undo, multiple-channel
composition, new families or outputs, presets, Legacy, or specification edits.

### Stage 6 tests and acceptance

- Snapshot and token cannot be mismatched.
- Missing channel/definition, unassigned or mismatched source, invalid mapping,
  invalid size, and unsupported structure fail at stable boundary paths.
- Commands continue to report the correct invalidation layer.
- Both baseline assets preserve accepted Stage 5 geometry and visually
  equivalent PNG/SVG output.
- No accepted renderer semantics regress.

Focused commands:

```bash
cargo test -p toniator-domain -p toniator-sampling -p toniator-patterns -p toniator-engine -p toniator-cli

cargo run --bin toniator -- render \
  -i assets/raster-sample.png \
  -o target/validation/stage-6/raster.png \
  --mode rgb --transparent \
  --canvas 900x600 \
  --density-x 90.0 --density-y 60.0 \
  --rotation 17.0 --offset-x 3.25 --offset-y -4.5 \
  --guard-steps 2 --source-component luminance \
  --size-min 2.0 --size-max 9.0 \
  --color '#00b7ff' --opacity 0.72

cargo run --bin toniator -- render \
  -i assets/vector-sample.svg \
  -o target/validation/stage-6/vector.svg \
  --mode rgb \
  --canvas 900x600 \
  --density-x 90.0 --density-y 60.0 \
  --rotation 17.0 --offset-x 3.25 --offset-y -4.5 \
  --guard-steps 2 --source-component luminance \
  --size-min 2.0 --size-max 9.0 \
  --color '#00b7ff' --opacity 0.72

identify -verbose target/validation/stage-6/raster.png
xmllint --noout target/validation/stage-6/vector.svg
```

**Stop condition:** Report document-derived identity parity and artifact
inspection; do not start Stage 7.

## Stage 7 — asynchronous scheduling and stale-result safety

**Status: Planned.** Evaluate immutable Stage 6 requests off the frontend
thread while ensuring superseded work cannot replace current output.

### Stage 7 implementation contract

- Use one engine-owned background worker and standard-library channels; no
  async runtime.
- Each submission owns its document snapshot, shares immutable source bytes,
  and receives a monotonically increasing ticket.
- A newer request cancels the active ticket and coalesces queued work to the
  newest request.
- Check cancellation before and after decode, family generation, realization,
  scene construction, and rasterization.
- Canceled work returns no partial output.
- Callers must validate completion tokens against the current
  `DocumentSession`.
- Shutdown and `Drop` terminate and join the worker cleanly.

Allowed: engine and tests, narrowly necessary domain token changes, Cargo
manifests, and plan/tracker status.

Forbidden: caches, GTK, UI callbacks, Tokio, worker pools, quality tiers,
persistence, undo, or algorithm changes.

### Stage 7 tests

- Current completions are accepted; stale revisions are rejected.
- Rapid N/N+1/N+2 submission exposes only N+2 as presentable.
- Cancellation returns no partial geometry.
- Errors retain their ticket and revision.
- Scheduling does not change identities or pixels.
- Shutdown does not leak or hang.
- Both baseline sources complete through the scheduler.

**Stop condition:** Report automated concurrency evidence; do not begin GTK or
Stage 8 work.

## Stage 8 — invalidation-aware derived caches

**Status: Planned.** Reuse the highest valid pipeline layer without allowing
caches to become authority.

### Stage 8 implementation contract

- Keep one bounded last-successful cache slot per active-channel layer: decoded
  source, family output, realization, scene, and raster preview.
- Typed keys include every relevant authoritative input.
- Source keys include content, format, and decoder-contract identity.
- Downstream keys include decoded-pixel identity so SVG font resolution remains
  represented.
- Cache entries are committed only after successful, uncanceled, current
  evaluation.
- Return read-only hit/miss diagnostics.
- Never persist caches or expose mutable cached values.

### Stage 8 tests

- Exact repeat reuses every layer.
- Presentation edits reuse source, family, and realization.
- Mark-size edits reuse source and family.
- Density, rotation, and translation reuse decoded source only.
- Source edits reuse no downstream layer.
- Failed, canceled, and stale evaluations do not replace cache entries.
- Cached and uncached results are identical for both baseline assets.

**Stop condition:** Report the reuse matrix and unchanged outputs; do not begin
GTK.

## Stage 9 — view-only GTK preview

**Status: Planned.** Provide the first native GTK/libadwaita frontend over the
Stage 6–8 engine pipeline.

### Stage 9 implementation contract

- Add GTK4/libadwaita dependencies only to `toniator-app`.
- Use tracked Blueprint sources and GResource; generated `.ui` files remain in
  Cargo `OUT_DIR`.
- Create an `AdwApplicationWindow` with header bar, Open action, empty state,
  loading state, error display, and fit-to-window canvas.
- Support a normal file chooser and `toniator-app [PATH]`.
- Opening artwork commits a new authoritative source reference and schedules
  evaluation.
- Display only a completion accepted by the current document revision.
- Wrap the exact straight-sRGBA `RasterSurface` in a GDK memory texture.
- Do not PNG-encode, flatten, checkerboard, or alter pixels for preview.
- Treat the widget background explicitly as viewer presentation, not file
  content.
- Surface SVG live-text/system-font diagnostics.

Allowed: `toniator-app`, Blueprint/GResource/build files, workspace dependency
declarations, architecture validation, future plan/tracker text, and Stage 9
validation artifacts.

Forbidden: pattern/channel editing, undo, save, export UI, recent files,
drag-and-drop, zoom tools, GTK geometry, or alternate rendering.

### Stage 9 validation

```bash
cargo test -p toniator-app
cargo check -p toniator-app --all-targets
GDK_BACKEND=wayland cargo run --bin toniator-app -- assets/raster-sample.png
GDK_BACKEND=wayland cargo run --bin toniator-app -- assets/vector-sample.svg
```

Manually inspect both sources for rotation, translation, mark response, edge
clipping, alpha behavior, resize fitting, SVG text diagnostics, and
stale-preview rejection during rapid source changes.

**Stop condition:** User visual acceptance is required. Do not begin Stage 10
automatically.

## Stage 10 — headless undo and redo

**Status: Planned.** Make authoritative commands reversible independently of
GTK widget state.

### Stage 10 public contract

- Add `DocumentHistory` around `DocumentSession`.
- Successful commands record authoritative before/after states and invalidation
  results.
- `undo()` and `redo()` each advance the current revision exactly once; old
  revision numbers are never restored.
- Undo/redo report the same affected channels and invalidation level as the
  original transition.
- Failed commands create no history.
- A new successful command after undo clears redo.
- Command coalescing is deferred.

### Stage 10 tests

- Round-trip every supported command, including source assignment and mapping.
- Restore exact values and pattern-definition references.
- Verify monotonic revisions and stale-token rejection.
- Verify empty undo/redo and failed commands are no-ops.
- Verify branching clears redo.
- Render both baselines after state restoration.

Forbidden: GTK bindings, persistence, serialized history, coalescing, or editor
controls.

**Stop condition:** Accept the headless history contract before persistence
begins.

## Stage 11 — portable `.toniator` container

**Status: Planned.** Save the complete supported document and its exact source
artwork in one portable file, then load and render it through the shared engine.

### Stage 11 container format

`.toniator` is a deterministic ZIP container, not plain JSON. Required entries
are:

```text
document.json
sources/<source-id>.png
```

or:

```text
document.json
sources/<source-id>.svg
```

Rules:

- `document.json` is versioned UTF-8 JSON with discrete `u32` schema versions.
- Source entries contain the exact original PNG or SVG bytes without decoding
  or recompression.
- The manifest records source ID, entry name, format, byte length, SHA-256, and
  optional non-authoritative display name.
- Source paths on the original filesystem are not persisted or used during
  loading.
- Entries use a stable order and normalized timestamps.
- Store entries without archive compression in version 1 for deterministic,
  lossless byte preservation.
- Reject duplicate required entries, missing entries, unsupported formats,
  invalid paths, hash/length mismatches, oversized entries, and malformed
  archives.
- Read named entries directly; never extract archive paths to the filesystem.
- Limit version-1 source and archive sizes to a documented safe boundary.
- Unknown container or document versions fail clearly; migrations are not
  implemented yet.

### Stage 11 IO and CLI behavior

- `toniator-io` owns ZIP layout, JSON DTO conversion, validation, and atomic
  saving.
- Loading returns a validated `Document` plus immutable embedded source bytes
  matched to its `SourceReferenceId`.
- Saving writes to a same-directory temporary file, flushes it, and atomically
  renames it.
- Add `toniator document create`.
- Add `toniator validate -i file.toniator`.
- Add `toniator render -i file.toniator -o output.png` and
  `toniator render -i file.toniator -o output.svg`.
- Document rendering uses saved state. CLI document overrides remain deferred.

### Stage 11 tests

- Exact round-trip of IDs, canvas, pattern definition, density, transform,
  source mapping, size response, appearance, and source bytes.
- Embedded PNG bytes exactly match `assets/raster-sample.png`.
- Embedded SVG bytes exactly match `assets/vector-sample.svg`, including live
  text.
- Move the `.toniator` file to another directory and render successfully.
- Create a document from a temporary source, remove that temporary external
  source, and prove the container still renders.
- Detect missing source entry, duplicate entry, changed bytes, hash mismatch,
  malformed JSON, malformed ZIP, unsafe entry name, oversized entry, and
  unknown version.
- Save failure leaves an existing document intact.
- Container-based and direct-source evaluation produce identical canonical
  identities and rendered output.
- SVG font-dependent decoded pixels remain environment-sensitive, but the
  embedded original SVG bytes remain exact.

Forbidden: external source references, GTK Open/Save integration, presets,
migrations, recovery/recent files, legacy import, arbitrary attachments, or
broad compatibility.

**Stop condition:** User inspects both `.toniator` containers and their
rendered PNG/SVG outputs. GTK document actions and editors require a separately
approved Stage 12 plan.

## Stage 12+ — GTK document actions and command-bound editors

Stage 12 and later work is deliberately deferred. GTK Open/Save integration,
document actions, command-bound pattern and channel editors, generalized
families, connected and region output, multiframe evaluation, and simple
transitions require separately scoped and approved short-stage contracts.

## Common validation and Git gates

Every stage ends with:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
bash scripts/validate_architecture.sh
git diff --check
git status --short --branch
sha256sum assets/raster-sample.png assets/vector-sample.svg
```

At each transition:

1. Mark only the approved stage **In progress**.
2. One writer implements only its allowlist.
3. Record **Implemented awaiting review**.
4. Obtain automated and, where applicable, visual acceptance.
5. Mark **Accepted awaiting checkpoint**.
6. Commit only after explicit authorization.
7. Record the implementation SHA in a documentation closeout commit.
8. Push only after explicit authorization and verify the upstream commit.
9. Stop before the next stage.

## Legacy quarry procedure

Assign one named responsibility per quarry. Record hidden dependencies and
characterization tests, adapt the algorithm to the receiving greenfield
interfaces, remove GTK/renderer/global/persistence assumptions, and perform an
isolated architecture review. Never copy a whole legacy module or treat its
structure as authority. Legacy remains read-only.
