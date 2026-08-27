# Toniator Greenfield Rewrite Plan

Status: approved execution roadmap and stage contract (2026-08-21)

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
- Keep GTK concerns in `toniator-app` only. The current app is GTK4-only;
  `toniator-cli` is a peer, headless frontend using `toniator-engine`, and no
  headless crate depends on a frontend or GTK.
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
| `toniator-app` | GTK4 application, Blueprint/GResource resources, controllers, view models, command bindings, preview presentation, task coordination. | Peer frontend; consumes engine/domain/IO and owns all GTK concerns. |

The intended flow is `domain → geometry/sampling → patterns → render/io →
engine → app or cli`; engine is the shared orchestration boundary, not a
second state authority.

### Authoritative CLI hierarchy

- `toniator render` is the product-level integration and acceptance surface.
  It evaluates the complete authoritative document and renders its ordered
  active channel topology.
- `toniator inspect grid` is a structural guide/site diagnostic.
- `toniator inspect marks` is a single-channel sampling/realization diagnostic.
- Inspect commands may expose low-level characterization parameters, but they
  are not document authorities or alternate product-render paths.
- Future CLI and GTK work extends or consumes the authoritative document
  evaluation path used by `toniator render`; it must not add a competing
  render authority.

## Baseline test artwork

The tracked files `assets/raster-sample.png` and `assets/vector-sample.svg`
are the project-wide source-artwork baselines. Relevant source loading,
sampling, rendering, preview, and export stages must exercise both files in
addition to any smaller synthetic fixtures. The PNG is a 1024×1024 RGBA image
with nontrivial alpha. The 900×620 SVG contains gradients, transparency, a
stroked path, and a live `<text>` element.

Low-resolution fixtures and outputs may supplement fast or isolated tests,
but they never satisfy the native-output gate by themselves. Every future
stage that exercises source loading, sampling, rendering, preview, or export
must also test both baselines at their natural source dimensions (1024×1024
for the PNG and 900×620 for the SVG) through the applicable canonical
consumer boundary.

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
- **Ready for user acceptance** — parent review and required visual evidence
  passed; user stage acceptance and checkpoint remain separate.
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

The user-authorized 2026-08-24 Stage 20M worktree correction establishes the
current centered local grid-prototype contract and regenerates the current
Stage 3 fixture: local `(0, 0)` maps to the geometric canvas center, rotates
about that local origin, then receives document-axis translation. It does not
rewrite the historical `f60eb65` checkpoint or introduce a schema/persistence
migration.

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
- Generate two stable-ID straight dimensions in local grid coordinates: local
  `(0, 0)` maps to the geometric canvas center, authored rotation occurs about
  that local origin, then document-axis X/Y translation applies. Random-site
  distributions and parametric structural-source adapters remain outside this
  grid-prototype transform contract.
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

The Stage 5 `RGB`/`CMYK` CLI selector was a temporary output-background choice
for that single-channel vertical slice, not an authoritative halftone channel
model. Stage 9 replaces that conflated syntax with independent
`--channel-model` and `--background` contracts while retaining Stage 5's
historical validation commands as checkpoint evidence.

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

### Capability and resource contracts

- Replace the temporary global `2.0..=9.0` circular-mark diameter restriction
  with `PatternDefinition::maximum_support_radius`, a finite, nonnegative
  structural capability. A mark response is valid when both diameters are
  finite and nonnegative, minimum is no greater than maximum, and
  `maximum_diameter / 2.0` does not exceed the selected definition's declared
  capability. Direct realization must enforce the same capability boundary.
- Preserve the accepted baseline by declaring support radius `4.5` and using
  the existing `2.0..=9.0` response in the transient CLI definition. Values
  below 2.0 or above 9.0 are supported when the definition declares enough
  capability; `4.5` is not a physical pixel-size limit.
- Add immutable `EvaluationLimits` with nonzero
  `max_family_candidates`, defaulting to `1_048_576`. Compute the guide-range
  Cartesian product with checked arithmetic and reject an over-limit request
  at stable path `coverage.candidate_limit` before allocating candidates.
  Include the limit in the family key so a cached family cannot bypass a
  stricter policy.

### Cache and acceptance contract

- Keep a private engine `DerivedCache` with one last-successful slot for each
  active-channel layer: decoded source, family output, realization, scene, and
  transparent raster preview. Cached values are immutable and may use `Arc`
  internally; caches are neither persisted nor exposed as writable state.
- Evaluation reads a cache snapshot and stages misses in a private transaction.
  The worker never mutates the accepted cache. A new submission supersedes any
  unaccepted transaction for an older ticket.
- Preserve nonblocking, latest-only `try_receive_latest()`. Add
  `EvaluationScheduler::accept_completion(&completion, &DocumentSession)`;
  it validates both the latest ticket and current document token. Return
  `true` for a current success or current failure, `false` for a stale
  completion, and commit the staged cache transaction only for a current
  successful completion. Repeated acceptance is safe and does not recommit.
- Failed, canceled, stale, superseded, and successfully completed but
  unaccepted evaluations never replace last-successful cache entries.
- Add immutable `CacheDisposition::{Hit, Miss}` and `CacheDiagnostics` with
  decoded-source, family, realization, scene, and raster dispositions.
  `EvaluationCompletion::cache_diagnostics()` returns diagnostics only for a
  successful completion; failure reporting remains unchanged.

### Public evaluation interfaces

- Existing `evaluate()` and `EvaluationScheduler::new()` use default limits.
  Add `evaluate_with_limits(...)` and
  `EvaluationScheduler::new_with_limits(...)` for deliberate caller policy.
- Extend the existing grid, marks, and render CLI commands with
  `--max-family-candidates`; omitted values use the default.
- Cache policy is scheduler-owned and does not change authoritative
  `Document`, `DocumentSession`, canonical geometry, identities, pixels, or
  SVG output.

### Typed cache keys and pipeline order

- Decode before the family-cache lookup while retaining authoritative preflight
  validation first. Successful uncached outputs remain byte- and
  identity-equivalent to the accepted Stage 6/7 pipeline.
- The source key contains the logical source reference ID, exact immutable
  source bytes, `SourceFormatHint`, and a sampling-owned versioned decoder
  contract ID.
- The family key contains the source key and decoded-pixel identity, canvas,
  density, rotation, translation, guard depth, pattern structure/output,
  declared maximum support radius, and evaluation candidate limit.
- The realization key contains the family key, decoded identity, canvas,
  source component/placement, and mark response. The scene key adds channel
  ID, visibility, color, and opacity. The raster key adds the transparent
  rasterization contract.
- Source reference, byte-content, format, or decoder-contract edits miss all
  layers. The decoded-pixel identity remains in downstream keys because SVG
  system-font resolution can change decoded pixels without changing source
  bytes.

### Required reuse matrix

| Request relative to accepted cache | Source | Family | Realization | Scene | Raster |
| --- | --- | --- | --- | --- | --- |
| First evaluation | Miss | Miss | Miss | Miss | Miss |
| Exact repeat | Hit | Hit | Hit | Hit | Hit |
| Color, opacity, or visibility edit | Hit | Hit | Hit | Miss | Miss |
| Mark-size or source-mapping edit | Hit | Hit | Miss | Miss | Miss |
| Density, rotation, translation, canvas, or support-capability edit | Hit | Miss | Miss | Miss | Miss |
| Source reference, bytes, format, or decoder-contract edit | Miss | Miss | Miss | Miss | Miss |

### Stage 8 scope and verification

Allowed: `toniator-domain`, `toniator-sampling`, `toniator-patterns`,
`toniator-engine`, `toniator-cli`, their focused tests and narrowly necessary
Cargo manifests, architecture validation, tracker status, and checkout-aware
Stage 8 evidence.

Forbidden: GTK/app work, geometry or render-algorithm changes, new families or
outputs, multiple-channel composition, persistence, undo, presets, Legacy,
baseline-asset changes, normative specification edits, or Stage 9 work.

Tests must prove the reuse matrix; default and custom candidate limits without
oversized allocation; valid mark sizes below 2.0 and above 9.0 when within the
declared capability; direct-realization rejection beyond that capability; and
unchanged Stage 7 cancellation, coalescing, polling, error, and shutdown
behavior. Successful-unaccepted, document-stale, ticket-stale, canceled, and
failed evaluations must not replace the prior cache, demonstrated by a later
accepted request. For both immutable PNG and SVG baselines, cached and
uncached `EvaluationResult`, scene identities, raster bytes, and SVG bytes must
be identical; SVG downstream keys must include decoded-pixel identity.

Run focused domain/sampling/patterns/engine/CLI tests while iterating, then one
complete final workspace format/check/strict-Clippy/test and architecture
gate, baseline hashes, protected-tree checks, and final diff review. Stage 8
makes no graphical or GTK acceptance claim because it must not change pixels.

**Stop condition:** Report the reuse matrix, transactional acceptance evidence,
resource-limit evidence, and unchanged outputs; propose **Implemented awaiting
review**, leave the work uncommitted, and do not begin GTK or Stage 9.

## Stage 9 — Authoritative Multi-Channel Document Evaluation

**Status: Complete at `67e831a`.** Replace the temporary single-channel
document assumption with a bounded headless authority for RGB, CMYK, and
SourceColorAlpha channel topologies. The technical contract below is one
coherent Stage 9 contract, delivered through five separately accepted local
checkpoints:

1. **Stage 9A — Channel authority and topology.** Domain model, roles, stable
   IDs, canonical topology factory, mappings, validation, atomic replacement,
   revisions, affected-channel reporting, and invalidation.
2. **Stage 9B — Source fields and source-colored realization.** Linear RGB,
   deterministic full-UCR CMYK fields, mapping transforms, alpha association,
   SourceColorAlpha interpolation, zero-alpha suppression, and per-mark sampled
   paint/content identity.
3. **Stage 9C — Fixed model compositors.** Layer-local coverage, additive RGB,
   idealized subtractive CMYK, SourceColorAlpha source-over, straight-sRGBA,
   consumer-only PNG backing, and editable vector SVG semantic correspondence.
4. **Stage 9D — Complete-document engine evaluation.** Ordered multi-channel
   evaluation, aggregate identities, accepted-cache reuse, transactional
   scheduler semantics, cancellation, and diagnostics.
5. **Stage 9E — Authoritative CLI and integration evidence.** Complete-document
   `render`, channel-model/background migration, native artifacts, full
   workspace validation, and final Stage 9 visual review.

Each substage starts only from the previous accepted local checkpoint. One
writer implements one substage through its narrow allowlist, stops at
**Implemented awaiting review**, and waits for user acceptance. After
acceptance, create the local implementation and tracker/documentation
checkpoint commits before beginning the next substage. A push is optional and
occurs only when the user explicitly requests it. Stage 10 cannot begin until
Stage 9E is accepted and locally checkpointed.

### Stage 9 authoritative topology and mapping

- Add `HalftoneChannelModel::{Rgb, Cmyk, SourceColorAlpha}` to the authoritative
  document.
- Add ordered channel roles with canonical IDs: Red `1`, Green `2`, Blue `3`;
  Cyan `4`, Magenta `5`, Yellow `6`, Black `7`; SourceColor `8`.
- RGB topology is ordered Red, Green, Blue. CMYK is ordered Cyan, Magenta,
  Yellow, Black. SourceColorAlpha contains the SourceColor role.
- A canonical topology factory owns role, canonical stable ID, order, default
  mapping, default paint, visibility, and opacity. It clones one caller-supplied
  validated pattern/layout/mark-response template across every role. It does
  not choose pattern family settings, density, phase, or role-specific angles.
- Explicit topologies may use arbitrary valid stable IDs. Validation requires
  unique IDs, the model's roles exactly once and in canonical order, supported
  mappings and paint, valid pattern-definition references, and no extraneous
  roles.
- Add one atomic model/topology replacement command carrying the requested
  model and complete explicit ordered channels. It installs both or neither,
  never resets, guesses, or migrates settings. Report
  `InvalidationLevel::ChannelTopology` with affected IDs in deterministic old
  order followed by newly introduced IDs in new order.
- Keep per-channel source mapping independent from layer paint. Mapping changes
  are `Realization`; ordinary solid color, opacity, and visibility changes are
  `Presentation`.

Source mappings support Red, Green, Blue, Cyan, Magenta, Yellow, Black, Alpha,
and Luminance plus the existing placement. Mapping applies:

```text
mapped = clamp(gain * (inverted ? 1 - value : value) + bias)
```

Gain is finite and nonnegative; bias is finite. Canonical RGB, CMYK, and
SourceColorAlpha mappings use `inverted=false`, `gain=1`, and `bias=0`.
Existing single-channel Luminance/Alpha behavior remains expressible and must
retain its accepted Stage 3–8 identities and output under equivalent mapping.

Decode straight sRGB, convert color to linear light, and derive fields before
spatial interpolation. RGB uses linear `R`, `G`, and `B`. CMYK uses
profile-independent unnormalized full UCR:

```text
K = 1 - max(R, G, B)
C = 1 - R - K
M = 1 - G - K
Y = 1 - B - K
```

Clamp component values to `[0,1]`. Apply the mapping transform, then multiply
color-derived responses by source alpha exactly once before interpolation.
Alpha is independent and is not alpha-multiplied again. Do not add normalized
CMYK, ICC/profile handling, dot gain, configurable UCR/GCR, black-generation
curves, physical ink simulation, or soft proofing.

SourceColorAlpha has one special evaluated-paint contract:

1. Decode straight sRGB and independent alpha.
2. Convert RGB to linear light.
3. Associate linear RGB with alpha for spatial interpolation.
4. Interpolate associated RGB and alpha.
5. For positive sampled alpha, unassociate back to straight linear RGB and use
   it as per-mark paint.
6. Independently use sampled alpha as the mark-response field.

Positive alpha changes mark size only; it does not also scale mark color or
opacity. Exact zero sampled alpha suppresses visible paint even when the
configured minimum mark size is nonzero. Hidden RGB remains inspectable but
cannot bleed across transparent edges or contribute visible zero-alpha marks.

### Document evaluation and scheduling

- Promote the shared unprefixed evaluation request/result/scheduler path to
  complete-document evaluation. Retain the Stage 3–8 single-channel evaluator
  under explicit channel-diagnostic names for `inspect marks` and regression
  characterization, not as a competing render authority.
- One immutable document snapshot/token represents one authoritative revision.
  One resolved source is shared by all channels in Stage 9.
- Perform authoritative preflight for the complete topology, decode source
  bytes once, evaluate channels in authoritative order, and construct one
  ordered `RenderScene` containing every topology channel. Invisible channels
  remain ordered authoritative layers but contribute nothing.
- Failure of any required channel fails the complete evaluation. Do not expose
  or stage an acceptable partial scene. Check cancellation between channel
  evaluations as well as at the existing Stage 7 boundaries.
- Preserve latest-only polling, coalescing, cancellation-over-error, stale
  rejection, transactional acceptance, idempotent acceptance, checked tickets,
  shutdown, and `Drop` joining.
- Default/custom family-candidate limits remain checked per evaluated family
  before allocation.

### Fixed model compositing semantics

Each ordered layer first produces its accepted conventional layer-local
premultiplied-linear result `(P_i, A_i)` in stable mark order. Raster
antialiasing retains the Stage 5 8×8 grid: fractional mark coverage is
`q = covered_subsamples / 64`. With straight-linear paint `C`, paint alpha `a`,
and layer opacity `o`, each mark uses:

```text
s = clamp(q * a * o)
P <- s*C + P*(1-s)
A <- s   + A*(1-s)
```

Invisible layers use `(0,0)`. This local step preserves exact accepted
single-layer behavior, including overlapping marks.

RGB is fixed additive linear Porter-Duff lighter composition:

```text
P_rgb = clamp(sum(P_i))
A_rgb = clamp(sum(A_i))
```

The sums and clamp are componentwise. This yields red+green=yellow,
red+blue=magenta, green+blue=cyan, and red+green+blue=white for full canonical
coverage while using actual validated paint and opacity.

CMYK is fixed idealized subtractive transmittance. For positive `A_i`, let
`C_i = P_i / A_i`; zero-alpha layers have no factor:

```text
T = product(1 - A_i * (1 - C_i))
A_cmyk = 1 - product(1 - A_i)
P_cmyk = T - (1 - A_cmyk)
```

Products are componentwise and clamped for deterministic floating-point
boundaries. Canonical full coverage yields M+Y=red, C+Y=green, C+M=blue,
C+M+Y=black, and K independently reduces every transmittance component.
Compositing the transparent result over white exactly recovers `T`; white is
not baked into transparent output.

SourceColorAlpha uses its layer-local conventional ordered source-over result.
These three compositors are fixed semantics of the model, not selectable blend
modes.

For every model, if final alpha is positive, unassociate `P/A`, convert linear
RGB to sRGB, quantize each RGB component to nearest 8-bit, and quantize alpha
to nearest 8-bit. Zero alpha is transparent black. `RasterSurface` remains
straight 8-bit sRGBA. An explicit PNG black/white background is applied only
after this transparent scene result as a final consumer operation.

### Editable vector SVG semantic correspondence

- Export RGB and CMYK channels as ordinary first-class `<g>` children of one
  shared canvas group, in authoritative channel order. Apply the single canvas
  clip to that shared group. Keep every mark as ordinary editable vector
  geometry inside its channel group.
- SourceColorAlpha remains one ordinary clipped source-colored group whose
  marks carry their sampled colors; do not decompose it into RGB groups.
- Do not move channel geometry into `<defs>`, reconstruct it through `feImage`,
  duplicate or hide it solely for composition, replace the visible artwork
  with a filtered proxy, or embed a raster image.
- Apply paint alpha and layer opacity to each vector mark so overlapping marks
  reproduce the layer-local equation; do not move opacity to a post-composited
  group operation.
- Isolate the shared canvas group. Use editable artist-facing `screen` group
  blending for RGB, `multiply` group blending for CMYK, and ordinary ordered
  source-over for SourceColorAlpha. SVG remains transparent.
- The raster compositor remains the canonical exact linear implementation.
  SVG and raster require semantic correspondence, not unconditional pixel
  parity: full-opacity canonical relationships must match exactly, while known
  fractional-alpha differences from supported SVG viewer blend behavior must
  be tested and documented with RGB and alpha reported separately. Exact
  equality is required only where both representations can express the same
  result.
- Assert the serialized editable group structure and render human-readable
  synthetic SVG fixtures through the in-process SVG stack and Inkscape. Verify
  channel and mark editability/query visibility, exact full-opacity canonical
  relationships, and the explicitly characterized fractional-alpha behavior.

### Identity and cache layering

- A per-channel family content identity contains only inputs that determine
  Stage 3 structural output: canvas, resolved density values, rotation,
  translation, guard depth, structural family parameters, and declared support
  capability. It does not include source identity/bytes, aspect-lock authoring
  state, pattern output treatment, candidate-limit policy, mapping, sampled
  values, response, paint, visibility, opacity, model compositor, or consumer
  settings. The private family cache lookup additionally carries the configured
  candidate limit so accepted cached work cannot bypass a stricter policy; that
  safety policy is not part of the successful structural content fingerprint.
- The aggregate document family identity contains the model and ordered
  `(role, channel ID, per-channel family identity)` topology. A topology change
  changes this aggregate identity, but an immutable family artifact is reusable
  for any channel whose complete structural key matches, regardless of model,
  role, or channel ID.
- A per-channel realization/content key and identity contains its family
  identity, decoded source identity, complete source mapping, mark response,
  and resulting canonical geometry/content. SourceColorAlpha sampled per-mark
  color is evaluated content and participates here. Ordinary solid
  presentation color does not.
- The aggregate realization identity contains ordered topology and ordered
  per-channel realization/content identities.
- Scene identity adds ordered layers, solid or sampled paint, visibility,
  opacity, and the fixed halftone-model compositor contract.
- The transparent raster key adds only the versioned transparent rasterization
  and model-compositor contract. Export background, PNG/SVG selection, encoder
  options, and other final-consumer choices remain outside source/family/
  realization/scene identity and cannot invalidate `RenderScene`.

Retain exactly five private last-successful aggregate cache slots. Source holds
one decoded value. Family and realization each hold the last accepted
document's immutable per-channel keyed collection; scene and transparent raster
are aggregate values. Matching entries may be shared with `Arc`. A source edit
misses source and source-dependent realization/scene/raster but may reuse an
unchanged structural family. A structural edit misses the affected family and
downstream. A mapping/response edit misses only the affected channel
realization before aggregate scene/raster reconstruction. A presentation edit
reuses source/family/realization and misses scene/raster. Export background or
encoding changes reuse the scene.

Preserve the five-field immutable aggregate `CacheDiagnostics`; family or
realization is an aggregate Hit only when every required channel entry came
from the accepted cache. Add ordered immutable per-channel family/realization
diagnostics. `Hit` always means accepted-cache reuse, never intra-evaluation
memoization. Workers stage one private multi-channel transaction; only an
accepted current successful completion commits it.

### CLI migration

- `toniator render` constructs/evaluates the complete authoritative document
  topology and is the only product render path.
- Add required `--channel-model rgb|cmyk|source-color-alpha` for direct-source
  rendering. Reject the old `--mode` semantics rather than retaining a second
  interpretation.
- Use the existing common pattern/layout/response settings as the one factory
  template cloned across canonical roles. A global opacity override may be
  deliberately applied to every created channel. Canonical role mappings and
  paints replace the old global render `--source-component` and `--color`;
  those low-level controls remain available to `inspect marks`.
- Add `--background transparent|black|white` for PNG, defaulting to
  transparent. It is a final-consumer choice and is never inferred from model.
  Explicit black/white background with SVG fails clearly; SVG remains
  transparent.
- PNG encoding remains sRGB/sRGBA for every model. CMYK denotes halftone
  topology and fixed composition, never a CMYK file encoding.
- `inspect grid` and `inspect marks` retain their diagnostic roles and never
  construct an alternate document-render path.

Stage 9E does not implement the later source-native sizing or PNG
antialiasing contracts: its direct-source CLI still requires explicit
`--canvas`, and it has no `--antialiasing` option.

### Stage 9 substage scope and acceptance gates

The following allowlists are cumulative only through accepted commits, never
within one writer assignment. If a later integration substage exposes a defect
in an earlier accepted layer, stop and open a bounded corrective checkpoint for
that owning substage instead of silently widening the active allowlist.

#### Stage 9A — Channel authority and topology

Allowed: `crates/toniator-domain/{Cargo.toml,src/**,tests/**}`;
`ProgressTracker.md`; checkout-aware Stage 9A evidence; workspace
`Cargo.toml`/`Cargo.lock` only if narrowly required. No dependency is expected.

Acceptance proves all three canonical topologies, exact role order and IDs,
arbitrary valid explicit IDs, missing/duplicate/extraneous/out-of-order roles,
mapping and paint compatibility, atomic replacement success/failure, unchanged
state on failure, one revision advance on success, deterministic affected IDs,
`ChannelTopology` invalidation, mapping/response `Realization`, and ordinary
solid paint/opacity/visibility `Presentation`.

**9A stop:** Update only Stage 9A to **Implemented awaiting review**, report the
focused domain and workspace boundary evidence, and wait. After user acceptance,
create local implementation and tracker checkpoint commits before Stage 9B.

#### Stage 9B — Source fields and source-colored realization

Allowed: `crates/toniator-sampling/{Cargo.toml,src/**,tests/**}` and
`crates/toniator-patterns/{Cargo.toml,src/**,tests/**}`;
`ProgressTracker.md`; checkout-aware Stage 9B evidence; workspace
`Cargo.toml`/`Cargo.lock` only if narrowly required. The accepted Stage 9A
domain API is read-only in this substage.

Acceptance uses exact black, white, RGB primary/secondary, neutral-gray,
partial-alpha, zero-alpha hidden-RGB, and opaque/transparent-boundary fixtures.
It proves linear fields, unnormalized full UCR, transform order,
inversion/gain/bias/clamping, color alpha association exactly once, independent
Alpha, SourceColorAlpha unassociation and straight sampled paint, exact-zero
suppression, positive-alpha size-only behavior, immutable source-derived paint
content identity, and retained Stage 3–8 single-channel results. Both immutable
baseline sources are exercised without modifying them.

**9B stop:** Update only Stage 9B to **Implemented awaiting review**, report
sampling/realization evidence, and wait. After user acceptance, create local
implementation and tracker checkpoint commits before Stage 9C.

#### Stage 9C — Fixed model compositors

Allowed: `crates/toniator-render/{Cargo.toml,src/**,tests/**}`;
`ProgressTracker.md`; checkout-aware Stage 9C evidence; derived compositor
artifacts under `target/validation/stage-9c/`; workspace `Cargo.toml`/`Cargo.lock`
only if narrowly required. Accepted domain/sampling/pattern APIs are read-only.

Acceptance proves exact RGB and CMYK primary/secondary/neutral relationships,
overlap, fractional coverage, saturation, opacity, visibility, transparent
representation, K behavior, SourceColorAlpha source-over, straight-sRGBA
quantization, consumer-only transparent/black/white PNG backing, exact
single-layer compatibility, deterministic editable SVG channel structure, no
raster embedding or proxy composition, and in-process plus Inkscape SVG/raster
semantic correspondence with RGB and alpha measured separately. Exact parity
is required only where both representations can express the same result; known
fractional-alpha differences are explicitly tested and documented. Both
immutable sources are exercised.

**9C stop:** Update only Stage 9C to **Implemented awaiting review**, report the
equations, synthetic results, editable SVG structure, semantic-correspondence
evidence, and known fractional-alpha differences, and wait. After user
acceptance, create local implementation and tracker checkpoint commits before
Stage 9D.

#### Stage 9D entry seam correction — explicit channel diagnostics

Before Stage 9D, rename the accepted Stage 3–8 single-channel engine request,
result, synchronous evaluator, and scheduler APIs to explicit
`ChannelDiagnostic*` / `evaluate_channel_diagnostic*` names, and mechanically
migrate their existing CLI caller and regression tests. Remove the old
unprefixed single-channel names so Stage 9D can make that namespace the
complete-document authority without a compatibility dispatcher or competing
render path.

This corrective checkpoint may change only `toniator-engine`, `toniator-cli`,
their tests, checkout-aware evidence, this roadmap, and `ProgressTracker.md`.
It must preserve exact accepted Stage 3–8 behavior, identities, scheduler/cache
semantics, CLI syntax/output, and both immutable source results. It adds no
channel-model CLI integration, export-background behavior, complete-document
evaluation, or other Stage 9D/9E product behavior. Checkpoint the correction
and its documentation closeout before Stage 9D begins.

#### Stage 9D — Complete-document engine evaluation

Allowed: `crates/toniator-engine/{Cargo.toml,src/**,tests/**}`;
`ProgressTracker.md`; checkout-aware Stage 9D evidence; workspace
`Cargo.toml`/`Cargo.lock` only if narrowly required. All accepted lower-layer
APIs are read-only.

Acceptance proves 3 ordered RGB layers, 4 ordered CMYK layers, 1
SourceColorAlpha layer, one decode per source miss, invisible authoritative
layers, all-or-nothing failure, per-channel mapping/presentation/structural
reuse, source/family independence, safe topology reuse, aggregate identities,
ordered immutable per-channel diagnostics, five aggregate cache dispositions,
candidate limits, transactional failure/canceled/stale/unaccepted safety, and
every retained Stage 7–8 scheduler and single-channel primitive behavior. Both
immutable sources must produce equal cached/uncached results, identities,
raster bytes, and SVG bytes; decoded-pixel identity remains downstream.

**9D stop:** Update only Stage 9D to **Implemented awaiting review**, report the
complete reuse matrix and scheduler evidence, and wait. After user acceptance,
create local implementation and tracker checkpoint commits before Stage 9E.

#### Stage 9E — Authoritative CLI and integration evidence

Allowed: `crates/toniator-cli/{Cargo.toml,src/**,tests/**}`;
`scripts/validate_architecture.sh` only for a narrowly necessary boundary
assertion; `ProgressTracker.md`; checkout-aware Stage 9E evidence; derived
artifacts under `target/validation/stage-9/`; workspace `Cargo.toml`/`Cargo.lock`
only if narrowly required. Accepted core APIs are read-only. A lower-layer
defect pauses 9E for a separate bounded correction rather than widening 9E.

Acceptance proves all three models use the complete authoritative `render`
path; inspect commands remain diagnostics; obsolete `--mode` is rejected;
`--channel-model` is required for direct-source render; background is
consumer-only with transparent default; black/white PNG backing works; SVG
rejects opaque backing; PNG remains sRGB/sRGBA; and candidate limits remain
enforced. Run the one complete final workspace gate after the last executable
change and reuse it unless another executable change invalidates it.

Generate one native transparent PNG and one vector SVG for each of the six
model/source combinations under `target/validation/stage-9/`: RGB/PNG source,
RGB/SVG source, CMYK/PNG source, CMYK/SVG source, SourceColorAlpha/PNG source,
and SourceColorAlpha/SVG source. Preserve native alpha and vector geometry. Do
not flatten, checkerboard, or replace review files with composites. Inspect RGB
and alpha separately, distinguish viewer background from file content, verify
SVG XML/filter structure, and retain the live-text/system-font caveat. The
accepted artifact template uses channel opacity `1.0`. For SourceColorAlpha,
source alpha affects mark size only: every positive-alpha SVG mark is opaque,
and PNG mark interiors reach alpha `1.0`, with fractional alpha arising only
from antialiasing. This is a validation-template rule and does not remove or
alter the channel-opacity feature.

**9E stop (accepted):** The user accepted the complete Stage 9 evidence and all
twelve native artifacts. The Stage 9E implementation is checkpointed at
`67e831a`; this roadmap and the tracker record its documentation closeout. Do
not begin GTK or Stage 10 without the parent’s next-stage authorization, and do
not push unless explicitly requested.

### Complete Stage 9 test matrix

Domain/topology tests must cover all three canonical topologies, exact canonical
roles/order/IDs, arbitrary valid explicit IDs, missing/duplicate/extraneous or
out-of-order roles, mapping and paint validation, atomic replacement failure,
revision behavior, deterministic affected IDs, and `ChannelTopology`
invalidation.

Sampling/math tests use exact black, white, RGB primary/secondary, neutral-gray,
partial-alpha, and zero-alpha hidden-RGB fixtures. Prove linear fields,
unnormalized UCR, transform order, inversion/gain/bias/clamping, color alpha
association exactly once, independent Alpha, SourceColorAlpha interpolation,
zero-alpha suppression, and that positive alpha changes size without fading
paint.

Render tests prove exact RGB and CMYK primary/secondary/neutral relationships,
overlap, fractional coverage, opacity, visibility, transparent output,
straight-sRGBA quantization, single-layer compatibility, deterministic editable
SVG channel structure, and native-raster/SVG semantic correspondence without
raster embedding, `feImage` reconstruction, or proxy artwork. Exact parity is
required only for mutually expressible results; fractional-alpha differences
are characterized explicitly.

Engine/cache/scheduler tests prove 3 ordered RGB layers, 4 ordered CMYK layers,
1 SourceColorAlpha layer, per-channel mapping/presentation/topology reuse,
source/family independence, safe topology artifact reuse, transactional failure
safety, cancellation/stale/unaccepted safety, deterministic aggregate
identities, ordered per-channel diagnostics, and every retained Stage 7–8 and
single-channel primitive behavior. Both immutable sources must produce equal
cached/uncached document results, raster bytes, and SVG bytes in-process; SVG
decoded-pixel identity remains downstream where source-derived content needs
it.

CLI tests prove every model uses the authoritative document path, inspect
commands remain diagnostics, obsolete `--mode` render semantics are rejected,
background is consumer-only with transparent default, SVG rejects opaque
background, and PNG is always sRGB/sRGBA.

Run focused crate tests within every substage and a proportional workspace
boundary check before its review stop. Stage 9E owns the one complete final
workspace format/check/strict-Clippy/test, architecture, asset-hash, XML,
protected-tree, and diff/status gate after the last executable change. Reuse
successful gate evidence; do not rerun the full gate unless a later executable
change invalidates it.

**Stage 9 umbrella status: Complete at `67e831a`.** Stages 9A through 9E are
accepted and locally checkpointed, and the user visually accepted all six
native model/source artifact pairs. Do not begin GTK or Stage 10 without the
parent’s next-stage authorization.

## Stage 10 — View-only GTK preview

**Status: Complete at `980af50`.** The user accepted the bounded native
GTK/libadwaita preview and its intrinsic-document resolution corrections over
the locally checkpointed Stage 9E complete-document integration path.

### Stage 10 implementation contract

- Add GTK4/libadwaita dependencies only to `toniator-app`.
- Use tracked Blueprint sources and GResource; generated `.ui` files remain in
  Cargo `OUT_DIR`.
- Create an `AdwApplicationWindow` with header bar, Open action, visible model
  selector, empty/loading/error/success states, and a fit-to-window canvas.
- Support a normal file chooser and `toniator-app [PATH]`; the app accepts no
  canvas, model, edit, save, or export arguments.
- Opening artwork commits a new authoritative source reference, constructs the
  requested canonical Stage 9 topology, and schedules complete-document
  evaluation.
- Decoded PNG dimensions and resolved SVG intrinsic/`viewBox` dimensions always
  define the authoritative preview `CanvasSpec` and aspect. The view-only app
  has no document/canvas override; a future output-dimension override belongs
  to PNG export and never resizes preview.
- Display only a completion accepted by the current document revision.
- A renderer-owned preview target rerasterizes the unchanged scene into the
  fitted output pixel dimensions. It preserves aspect/centering and output
  pixel antialiasing, but never changes authoritative `CanvasSpec`, canonical
  geometry, native raster/export behavior, or CLI semantics. The engine keeps
  source/family/realization/scene caches reusable across target changes and
  keys only the transparent preview raster by its checked target contract.
- Final preview sampling clips to the exact transformed document rectangle
  (left/top inclusive, right/bottom exclusive), preserving guard geometry while
  preventing it from leaking into letterbox margins; `splash.png` verifies the
  1280×640 canvas at fitted rows 120..600 of a 960×720 preview.
- Wrap the exact straight-sRGBA `RasterSurface` in a GDK memory texture. Do not
  PNG-encode, flatten, checkerboard, recompose channels, or alter pixels.
- Use viewer-only backdrop defaults: RGB black, CMYK white, and
  SourceColorAlpha fixed neutral mid-gray. Backdrop never changes document,
  scene, pixels, SVG, or export background.
- Surface SVG live-text/system-font diagnostics.

Allowed: `toniator-app`, Blueprint/GResource/build files, workspace dependency
declarations, the renderer-owned `toniator-render` preview-target raster API
and tests, `toniator-engine` preview requests/raster-cache tests and the
focused identity helper, user-provided Reddit preview regression assets and
their README records, architecture validation, future plan/tracker text, and
Stage 10 validation artifacts.

Forbidden: pattern/channel editing, undo, save, export UI, recent files,
drag-and-drop, zoom tools, GTK geometry/composition, or alternate rendering.

### Stage 10 validation

```bash
cargo test -p toniator-app
cargo check -p toniator-app --all-targets
GDK_BACKEND=wayland cargo run --bin toniator-app -- assets/raster-sample.png
GDK_BACKEND=wayland cargo run --bin toniator-app -- assets/vector-sample.svg
```

Ordinary Stage 10 acceptance commands intentionally omit pixel dimensions.
The app does not implement a canvas override; preview dimensions are always
source-intrinsic.

Manually inspect every model with both original baseline sources and the small
Reddit regression inputs for sharp resize fitting, backdrop policy, SVG
diagnostics, and stale-preview rejection during rapid source/viewport changes.

**Stop condition (complete):** Parent review, automated verification, user
visual acceptance, and the local implementation checkpoint at `980af50` are
complete. Do not begin Stage 11 automatically or push without explicit
authorization.

## Stage 11 — Headless undo and redo

**Status: Complete at commit `341ad8e`.** Make authoritative commands reversible independently of
GTK widget state.

### Stage 11 public contract

- Add `DocumentHistory` in `toniator-domain` as the sole mutable wrapper around
  one owned `DocumentSession`:

  ```rust
  pub struct DocumentHistory { /* private */ }

  impl DocumentHistory {
      pub fn new(session: DocumentSession) -> Self;
      pub fn session(&self) -> &DocumentSession;
      pub fn document(&self) -> &Document;
      pub fn revision(&self) -> Revision;
      pub fn can_undo(&self) -> bool;
      pub fn can_redo(&self) -> bool;
      pub fn apply(
          &mut self,
          command: &DocumentCommand,
      ) -> Result<CommandResult, DocumentSessionError>;
      pub fn undo(
          &mut self,
      ) -> Result<Option<CommandResult>, DocumentSessionError>;
      pub fn redo(
          &mut self,
      ) -> Result<Option<CommandResult>, DocumentSessionError>;
  }
  ```

- Do not expose mutable access to the wrapped session. Existing evaluators and
  schedulers receive `history.session()` and continue to validate its current
  session-minted tokens.
- Each successful command records one private entry containing the exact
  validated before/after `Document` snapshots and the original
  `CommandResult`. The session retains every entry for its lifetime; Stage 11
  has no capacity limit, truncation policy, disk spill, or coalescing.
- `undo()` installs the before snapshot and `redo()` installs the after
  snapshot. Both advance the current revision exactly once rather than
  restoring an old revision, and both return the original affected-channel
  ordering and invalidation level.
- A successful new command after undo clears redo. A failed command changes no
  document, revision, undo stack, or redo stack and does not clear redo.
- Empty undo/redo return `Ok(None)` without advancing revision. Revision
  exhaustion is atomic and retains the document and both stacks unchanged.
- Existing successful semantic no-op commands retain their current authority:
  they advance revision and create one history entry.
- History stores authoritative document state only. It never owns source bytes,
  decoded pixels, derived caches, scheduler work, or GTK state.
- History treats `Document` as an opaque validated snapshot. Future typed
  pattern definitions, internal mechanism IDs, and shared-definition references
  automatically participate in history without adding pattern-aware undo code.

### Stage 11 implementation boundary

- Use exact snapshots instead of manufacturing inverse commands; undo must not
  re-run command validation against a later schema or reconstruct topology.
- Add only a private `DocumentSession` restoration seam for internally recorded,
  already-validated snapshots. It must check the next revision before changing
  either state or history.
- Cover every current `DocumentCommand`: density, rotation, translation, mark
  response, color, opacity, visibility, source reference, legacy source
  mapping, complete topology replacement, modeled source mapping, and modeled
  paint.
- No current domain command, invalidation classification, engine cache key,
  scheduler API, renderer, CLI command, or GTK behavior changes in this stage.

### Stage 11 tests

- Round-trip every supported command through apply, undo, and redo in its valid
  legacy or modeled document context. Assert exact before/after `Document`
  equality, including channel model, ordered roles/IDs, mappings, paint,
  layout, response, appearance, source reference, and pattern-definition
  references.
- Assert undo and redo return the exact original `CommandResult`, including the
  deterministic affected-channel order of atomic topology replacement and
  complete-document source assignment.
- Exercise multi-command stack ordering, repeated undo/redo, empty stacks,
  branching, a failed command while redo is available, successful semantic
  no-ops, and atomic revision exhaustion.
- Prove legacy channel tokens and complete-document tokens become stale after
  apply, undo, and redo while newly minted tokens remain current.
- In engine integration tests, evaluate both immutable baseline sources under
  RGB, CMYK, and SourceColorAlpha, mutate and undo authoritative state, and
  compare restored scene identity, straight RGBA raster bytes, and SVG bytes
  exactly with the pre-edit result. Reject held scheduler completions after a
  history revision change.

Allowed: `toniator-domain` implementation and focused history tests,
`toniator-engine` integration tests only, plan/tracker transitions, Stage 11
evidence, and ordinary build artifacts under `target/`.

Forbidden: engine implementation changes, GTK bindings, CLI behavior,
persistence, serialized history, source-byte ownership, cache-policy changes,
coalescing, capacity settings, editor controls, new dependencies, Legacy work,
or protected specification edits.

### Stage 11 verification and stop

Run focused domain and engine tests, then workspace formatting/check/strict
Clippy/all-target tests, architecture validation, protected-tree checks,
baseline hashes, SVG XML validation, and `git diff --check`. Stage 11 is
headless and must leave rendered outputs byte-identical, so it requires no new
visual artifacts or visual-acceptance claim.

Stop uncommitted at **Implemented awaiting review** and request user technical
acceptance. Do not begin persistence, GTK editing, Stage 12, or later roadmap
implementation automatically.

## Stage 12 — Portable `.toniator` container

**Status: Complete at commit `dd7ca56`.** Save the complete supported document and its exact source
artwork in one portable file, establish immutable version-1 interpretation and
the migration dispatch boundary, then load and render through the shared
engine.

### Stage 12 container and version contract

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

- Container-layout version and document-schema version are separate discrete
  `u32` values. Stage 12 accepts container version 1 and document schema version
  1; a future document-schema change does not rename or reinterpret container
  version 1 unless the ZIP layout itself changes.
- `document.json` is versioned UTF-8 JSON parsed into an IO-owned
  `DocumentDtoV1`. Never deserialize archive data directly into private domain
  structs or derive the persisted interpretation from current field layout.
- Source entries contain the exact original PNG or SVG bytes without decoding
  or recompression.
- The manifest records source ID, entry name, format, byte length, SHA-256, and
  optional non-authoritative display name.
- Source paths on the original filesystem are not persisted or used during
  loading.
- Canonical writer output uses exactly the two file entries above, in stable
  order with normalized timestamps, and stores both entries without archive
  compression for deterministic, lossless byte preservation.
- The v1 reader tolerates standard Deflate for either required file and may
  accept one exact, empty `sources/` directory marker from a benign manual
  repack. It rejects every other directory, wrapper root, extra entry, or
  unsupported required-file compression method; this input tolerance does not change v1 JSON/DTO
  interpretation or the canonical writer output.
- Reject duplicate required entries, missing entries, unsupported formats,
  invalid paths, hash/length mismatches, oversized entries, and malformed
  archives.
- Read named entries directly; never extract archive paths to the filesystem.
- Limit version-1 source and archive sizes to a documented safe boundary.
- Unknown container or document versions fail clearly.
- Loading always follows the version boundary, even while version 1 is current:

  ```text
  ZIP/container version dispatch
  -> version-specific document parser
  -> stored document DTO
  -> deterministic migration dispatcher
  -> current DTO
  -> validated authoritative Document
  ```

  Stage 12 registers no transforming migration: v1 is already current. Stage
  14 adds the first explicit v1-to-v2 document migration without changing the
  accepted v1 parser or embedded source interpretation.

### Stage 12 IO and CLI behavior

- `toniator-io` owns ZIP layout, version-specific DTOs, deterministic migration
  dispatch, DTO/domain conversion, validation, and atomic saving.
- Loading returns a validated `Document`, an immutable `SourceBundle` keyed by
  `SourceReferenceId`, the stored version information, and a migration report
  which is empty for accepted v1 files. Missing, duplicate, unreferenced, or
  format/hash-mismatched source entries fail before evaluation.
- Saving writes only the current accepted schema version. Stage 12 writes v1;
  it provides no downgrade or alternate interpretation.
- Saving writes to a same-directory temporary file, flushes it, and atomically
  renames it.
- Loading is not a document edit. A loaded document creates a new
  `DocumentSession`/`DocumentHistory` at revision zero with empty undo and redo
  stacks. History, revision progress, dirty state, window state, filesystem
  paths, and recovery state are never serialized.
- Add `toniator document create`.
- Add `toniator validate -i file.toniator`.
- Add `toniator render -i file.toniator -o output.png` and
  `toniator render -i file.toniator -o output.svg`.
- Document rendering uses saved state. CLI document overrides remain deferred.

### Stage 12 tests

- Exact round-trip of halftone channel model, ordered role/ID topology, canvas,
  pattern definition, density, transform, source mapping, size response, paint,
  appearance, and source bytes.
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
- Commit deterministic immutable v1 fixtures
  `assets/raster-sample-v1.toniator` and
  `assets/vector-sample-v1.toniator`, document their hashes, and exercise them
  as the permanent Stage 14 migration inputs. Future tests must not regenerate
  a purported v1 fixture with a later writer.
- Prove the version dispatcher takes the v1 path, returns an empty migration
  report, and rejects unknown container/document versions without consulting UI
  defaults.

Forbidden: external source references, GTK Open/Save integration, presets,
transforming migrations, serialized history, recovery/recent files, legacy
import, arbitrary attachments, direct domain serde coupling, or broad
compatibility.

**Stop condition:** User inspects both `.toniator` containers and their
rendered PNG/SVG outputs and accepts v1 as immutable. Do not begin GTK document
lifecycle or the v1-to-v2 migration automatically.

## Stage 13A — GTK document lifecycle

**Status: Complete at commit `36c7b44`.** Add document lifecycle around Stage 11 history and Stage
12 persistence while remaining completely ignorant of pattern internals. The
separately accepted app-only reentrancy correction at `02bc2c9` preserves this
lifecycle behavior while preventing nested model-selector and window-close
callbacks; it is not part of the Stage 14 schema checkpoint.

- Add New, Open, Save, Save As, and Close plus close-with-unsaved-work
  confirmation, stable title/document identity, in-window errors, and generic
  migration diagnostics.
- Open either a direct PNG/SVG source or a `.toniator` container. Delegate
  direct-source/default-document construction to a headless factory; GTK must
  not construct or inspect the current straight-grid/circular-mark definition.
- One app-owned document workspace contains `DocumentHistory`, immutable source
  bundle, optional container location, display metadata, and an exact saved
  content baseline. It is lifecycle/controller state, not a second document.
- Dirty state compares current authoritative document plus source-bundle
  identity with the accepted savepoint. It is not `revision != saved_revision`,
  so undoing to saved content and successful semantic no-ops behave correctly.
- New and load create a fresh history at revision zero with empty stacks. Save
  updates the location/savepoint only after atomic IO success; failure preserves
  the current document, location, history, and dirty state.
- Existing asynchronous evaluation, accepted-ticket/revision gating, intrinsic
  preview sizing, raw RGBA presentation, and model-specific viewer backdrops
  remain authoritative.

Forbidden: channel or pattern controls, temporary editors for the v1 schema,
pattern-aware GTK branches, export UI, recent files, autosave/recovery systems,
presets, or Stage 14 schema work.

**Stop condition:** User accepts lifecycle behavior for direct sources and both
frozen v1 containers. Checkpoint Stage 13A independently.

## Stage 13B — Dedicated output and export parity

**Status: Complete at `2a773a3`.** The user accepted the corrected native
output review after the implementation and final automated gate. This stage
resolves the deferred final-consumer requirements after document lifecycle and
before generalized pattern architecture.

- Direct-source CLI rendering uses decoded/intrinsic PNG dimensions or resolved
  SVG intrinsic/`viewBox` dimensions by default. An explicit `--canvas` remains
  available and overrides only the direct-source default.
- Add `--antialiasing on|off` to PNG rasterization, default `on`; `off` is
  hard-edged. The choice participates only in raster output/cache identity.
- Add GTK Export for native PNG/SVG outputs with the existing consumer-only
  background policy, PNG antialiasing, and an explicit PNG output-dimension
  override. Export dimensions rerasterize canonical geometry but never resize
  the authoritative document or preview canvas. SVG is unaffected by the
  antialiasing option.
- Save remains `.toniator` persistence and Export remains output generation;
  neither operation mutates the authoritative document.
- Native and explicit output targets pass through the checked renderer safety
  limit before allocation. PNG background, target dimensions, and
  antialiasing remain final-consumer choices outside source, family,
  realization, scene, and persistence identity.
- GTK export evaluates an immutable workspace snapshot and accepts completion
  only for the current lifecycle/workspace generation. Pending export disables
  conflicting lifecycle actions; success does not install document or preview
  state, and failed, cancelled, or stale work preserves the workspace.
- The corrected app-test matrix uses each source's native aspect and output
  size: 1024×1024 for the raster baseline and 900×620 for the vector baseline.
  The earlier 96×64 app-test comparison was a display-scale moire diagnostic,
  not renderer-defect evidence. This stage records native artifact inspection
  and automated GTK snapshot coverage, without claiming exhaustive manual
  dialog, accessibility, or interactive GTK acceptance.

Forbidden: pattern schema or evaluator changes, document-owned export state,
SVG antialiasing behavior, implicit flattening/checkerboards, or editor work.

**Stop condition (complete):** The user accepted native CLI/app PNG and SVG
output across both baseline sources and all three models. The implementation
checkpoint is `2a773a3`; Stage 14 is recorded below as complete at `88fc6dd`.

## Stage 14 — Typed pattern-definition authority and v1-to-v2 migration

**Status: Complete at commit `88fc6dd`.** The bounded v1
`PatternStructure`/`PatternOutput` metadata is replaced by a
generator/mechanism-agnostic typed schema without changing the accepted
meaning of v1 files. The implementation checkpoint verifies the one-root
definition authority, document-wide stable IDs and ordering,
`DocumentHistory`-only definition commands, immutable v1 parsing,
deterministic v1-to-v2 migration, v2-only saves within container layout v1,
exact embedded source bytes, and accepted RGB/CMYK/SourceColorAlpha output
parity. Native artifact inspection used raw RGBA and editable SVG; it does not
claim exhaustive manual GTK dialog, accessibility, or interactive acceptance.
The separate accepted app-only lifecycle correction is checkpointed at
`02bc2c9`; Stage 15 is complete below at `711058b`.

### Stage 14 schema boundary

- Each definition retains exactly one typed structural family root, consistent
  with the normative pattern model. The family owns reusable typed mechanism
  substructures and may later gain composite/hybrid variants; Stage 14 does not
  introduce an arbitrary node DAG, stringly typed property bag, plugin ABI, or
  GTK metadata.
- The top level separates structural family, ordered output layers, modulation,
  and coverage. It can host future guide/site generators, curves, paths,
  segments, faces, networks, regions, and hybrids without naming artistic
  results or changing document/channel authority.
- Definitions and addressable internal mechanisms/output layers use stable
  domain-owned IDs and deterministic ordering. Discrete counts and seeds use
  discrete types (`u32` or appropriate stable IDs); continuous authored values
  remain `f64`.
- Pattern definitions remain structural. Density, channel transform, geometry
  response, mapping, paint, opacity, and visibility remain channel-instance
  state.
- Schema validation owns legal structure and capability compatibility. Canvas
  boundaries never generate sites, close faces, or form topology; they remain
  final-consumer clipping only.

### Stage 14 definition commands and migration

- Add document-owned collision-checked ID allocation and atomic commands to add,
  duplicate, edit, reference, and remove unreferenced definitions.
- Ordinary selected-channel structural editing is one copy-on-edit command: if
  shared, allocate a fresh definition ID, clone and apply the typed edit, and
  retarget only that channel in one validated history transition. Other
  channels retain the original definition.
- A separate explicit shared-edit command mutates the referenced definition and
  reports every linked channel in deterministic document order. Stale editor
  bases, ID exhaustion/collision, invalid edits, and removal of referenced
  definitions fail atomically.
- Introduce document schema v2 while retaining container layout v1. Parse the
  frozen v1 DTO with the accepted Stage 12 parser, migrate deterministically to
  the typed v2 representation, and write only v2 thereafter. No downgrade is
  required.
- The v1 straight-grid/intersection/circular-mark definition maps to ordinary
  typed v2 mechanisms with deterministic internal IDs. Route that supported v2
  configuration through the accepted evaluator and prove exact geometry,
  raster, and SVG parity; do not add a preset-name branch or hidden legacy
  interpretation.

Forbidden: GTK controls, named artistic pattern variants, presets, new family
algorithms, arbitrary graphs, source/output policy changes, or modification of
the accepted v1 parser/fixtures.

**Stop condition (complete):** The v2 authority, atomic sharing semantics,
frozen-v1 migration, deterministic v2 persistence, and exact accepted-output
parity were accepted at implementation checkpoint `88fc6dd`. Generic
evaluation is recorded below as complete at `711058b`.

## Stage 15 — Generic pattern evaluation pipeline

**Status: Complete at commit `711058b`.** Engine dispatch and cache identity
now use the typed mechanism contract before any expanded family vocabulary.
The accepted straight-guide/intersection/circular-mark configuration evaluates
through one generic family-to-modulation-to-ordered-realization path with
stable structural and realization provenance, exact cache invalidation/reuse,
bounded candidate/cancellation checks, transactional scheduler acceptance, and
unchanged canonical geometry, raster, PNG, SVG, preview, CLI, persistence, and
GTK behavior.

```text
typed PatternDefinition
-> family evaluation
-> modulation
-> ordered output realization
-> canonical geometry
-> final-consumer canvas clipping
```

- Families alone generate structural guides/sites or later structural products.
  Modulation and output realizers consume declared products; renderers never
  regenerate them or inspect pattern names.
- Define reusable typed family/output capability interfaces, provenance,
  support-envelope planning, candidate limits, deterministic identities, and
  cancellation boundaries. Unsupported mechanism variants fail before partial
  output.
- Generalize family/realization/scene/raster cache keys so properties invalidate
  the earliest affected stage and matching artifacts remain reusable.
- Preserve the v2 straight-guide/intersection/circle configuration through the
  generic path with exact geometry and rendered parity.

Forbidden: GTK, presets, new grid/random vocabulary, canvas topology, or
renderer-owned pattern dispatch.

**Stop condition (complete):** The generic headless pipeline, capability and
provenance validation, exact identity/reuse boundaries, cancellation and
failure atomicity, and frozen-v1/saved-v2 RGB/CMYK/SourceColorAlpha parity were
accepted at implementation checkpoint `711058b`. Stage 17 follows below.

## Stage 16A — Generalized straight-guide mechanisms

**Status: Complete at commit `ccec466`.** Add reusable straight-guide vocabulary through the generic
pipeline, never named rectangular/triangular pattern branches.

- Support one to four ordered straight-guide dimensions with independent stable
  IDs, baseline angles, phase/repetition, and shared channel transform handling.
- Generate intersection sites for declared dimension selections and regular
  arc-length sites along guides where valid, retaining stable provenance and
  guard structure.
- Add reusable typed mark prototypes and orientation rules through the same
  output machinery; channel size response remains independent of family sites.
- Extend analytical inverse-domain coverage to every supported dimension and
  transformation without finite-grid-then-rotate shortcuts.

Acceptance configurations include orthogonal, nonorthogonal, three-direction,
four-direction, parallel-guide, and along-guide results expressed solely as
schema data. Names are test descriptions, not evaluator discriminants.

**Stop condition:** User accepts native outputs and generalized coverage before
random/site-distribution mechanisms begin.

The user provisionally accepted Stage 16A on 2026-08-09, and the implementation
is checkpointed at `ccec466`. The acceptance does not retroactively replace the
recorded 90×60 generalized review artifacts, but establishes the cross-stage
natural-resolution rule above for all future applicable tests.

The user-authorized 2026-08-24 Stage 20M worktree correction establishes the
current shared centered grid-prototype placement for generalized straight guides
and authored generic curve-grid prototypes. It leaves the historical `ccec466`
checkpoint, random distributions, parametric structural-source geometry, and
their fingerprints intact; no schema/persistence migration is introduced.

## Stage 16B — Random and site-distribution mechanisms

**Status: Complete at commit `77bad7c`.** The typed random-site family proves the
Stage 14–15 architecture is not grid-shaped while retaining the generic
family-to-modulation-to-realization pipeline.

- Add raw uniform random, genuinely even/exclusion-based placement, clustered
  placement, and artwork-weighted density modulation with stable `u32` seeds.
- Support minimum center spacing, visible-mark exclusion margin, deterministic
  achieved-density diagnostics when a request is unsatisfiable, and bounded
  candidate/work policies.
- Reuse the same modulation, mark/output realization, channel response,
  canonical geometry, clipping, cache, preview, PNG, and SVG machinery used by
  Stage 16A.
- Treat Poisson-disk methods or blue-noise measurements as defined reusable
  constructions/quality evidence, never unexplained named generator branches.

The family uses the ordered mechanism chain `RandomSiteProcess` (raw uniform,
even, or clustered) → `SiteDensityModulation` (uniform or artwork-weighted
with the fixed Linear/Smoothstep responses) → `SiteExclusion` (None,
minimum-center, or visible-mark policy) → `RandomSiteProduct`. A deterministic xorshift32
stream consumes the authored `u32` seed, and accepted sites retain stable
candidate/accepted ordinals and Canvas/Guard provenance. Bounded diagnostics
report requested versus achieved sites, candidate and rejection counts, and
scope counts; cancellation and candidate/neighbor work limits are enforced
without partial publication. Visible-mark exclusion uses the conservative
maximum-support policy for the current circular output.

Only artwork-weighted structure depends on decoded source content and pixel
identity; source-independent random families remain source-free, and logical
source references stay at decoder lookup. The additive current-v2 DTO variants
preserve the immutable v1 parser/migration and existing v2 forms. All variants
reuse the existing canonical geometry, clipping, preview, PNG, and SVG output
path. Natural-resolution raster (1024×1024) and vector (900×620) artifacts
exercise high-density raw/native output and save/reopen parity. Automated raw
artifact inspection, CLI parity, and bounded app liveness are recorded; no
separate manual visual, interactive, or accessibility acceptance is claimed.

**Stop condition (complete):** Deterministic distribution distinctions,
exclusion guarantees, weighted sampling, persistence preservation, and native
output parity were accepted at implementation checkpoint `77bad7c`. Stage 17
headless editor commands follow in the next section.

## Stage 17 — Headless pattern/channel editing and capabilities

**Status: Complete at commit `e777270`.** The authoritative command and
introspection surface is complete before creating GTK controls.

- Add typed commands for every supported channel property and structural edit,
  including density-axis/aspect behavior, transform/phase, geometry response,
  mapping, presentation, definition selection, mechanism configuration,
  output-layer ordering, copy-on-edit, and explicit shared editing.
- Every command validates atomically, reports deterministic affected channels,
  and returns the earliest correct invalidation level: Presentation,
  Realization, Family, Source, or ChannelTopology.
- Provide schema-derived read-only capability/property descriptors containing
  stable typed field IDs, value kinds, legal enum choices, bounds, units,
  dependencies/visibility, structural support, and invalidation metadata.
- Descriptors never own values, validation, serialization, commands, UI labels,
  widget layout, fallback behavior, or alternate pattern interpretation. A
  descriptor/schema mismatch is a test failure.
- Every future GUI edit must already be reproducible through the same headless
  command/capability surface used by tests and CLI-oriented tooling.

**Stop condition:** Accept command completeness, descriptor derivation,
copy/shared semantics, undo/redo, exact invalidation, and restored render parity
before GTK inspector work. These conditions were accepted at implementation
checkpoint `e777270`.

### Stage 17A — Explicit compound-variant transition drafts

**Status: Complete at commit `2a85252`.** The headless authority now exposes
immutable transient drafts for random-character, density-modulation,
exclusion-policy, and guided-output-orientation transitions. Each draft derives
the complete typed payload, bounds, units, choices, references, and stable
targets from domain contracts, requires explicit confirmation, and finalizes
only the existing compatible `PatternDefinitionEdit`. Drafts remain absent from
active descriptors, document/history state, persistence, cache/evaluator
inputs, and frontend state. Domain validation, no-op/stale rejection, IDs,
invalidation, and shared/copy history behavior remain authoritative.

## Stage 18 — Descriptor-driven GTK channel inspector

**Status: Complete at commit `2a85252`.** Add channel selection and per-channel
editing over Stage 17 authority without structural pattern mathematics in GTK.

- Present channel appearance, source mapping, definition selection/sharing
  state, family-appropriate density, layout, and output-compatible geometry
  response from authoritative descriptors.
- Use progressive disclosure: common compatible controls first; advanced
  anisotropy, seeds, exclusion, and mechanism-specific parameters only when the
  active descriptor exposes them.
- Read current values through the separate immutable typed value reader, retain
  selected channels by stable ID with deterministic fallback, and keep drafts,
  disclosure, status, and focus state runtime-only.
- GTK renders descriptor-driven controls and visible compound-transition drafts;
  explicit confirmation dispatches typed edits through `DocumentHistory`, with
  selected copy-on-edit and deliberate shared-definition editing using the
  accepted history commands. Invalid, rejected, and semantic no-op drafts keep
  the document, history, preview, and last-successful cache unchanged.
- Preserve asynchronous revision/ticket rejection, last-successful raw-RGBA
  preview protection, lifecycle behavior, v1/current-v2 persistence, and the
  canonical preview/PNG/SVG output path.

The structural Pattern Editor remains Stage 19B; headless preset registry
work is recorded in Stage 19A below, while GTK preset authoring remains
out of scope.

Automated/static checks, native raw-artifact inspection, and the bounded app
source/liveness evidence do not substitute for actual GNOME/Wayland manual
visual, keyboard/focus, or assistive-technology review.

## Stage 19A — Pure-schema preset registry

**Status: Complete at commit `9919d85`.** The version-1 headless preset
registry is implemented with standalone `preset_format_version: 1` records.
The bundled entries have stable order `even-random-circles`, then
`straight-grid-circles`, and each is an ordinary typed recipe using only
exposed mechanisms.

- Applying a preset creates an independent document-owned definition by
  default. Updating an existing shared definition requires an explicit shared
  operation and affected-channel disclosure.
- Preset names, categories, and thumbnails are metadata only. Removing a preset
  removes the shortcut, not evaluator capability; no evaluator/cache/renderer
  branch may inspect a preset name.
- Reconstruction tests build every bundled preset from a blank definition using
  exposed typed controls and Stage 17A transition drafts, serialize/reload it,
  and compare canonical output. Selected application and explicit shared
  replacement remain separate operations; shared replacement confirms the
  disclosed ordered affected-channel set before mutation.
- Serialization/reload and independent reconstruction preserve canonical PNG/SVG
  parity at natural 1024×1024 raster and 900×620 SVG resolution. Strengthened
  RGB-independence evidence applies distinct definitions per channel and proves
  isolated red edits leave green/blue definitions, identities, isolated PNG
  bytes, and visible geometry unchanged, while documenting the modeled SVG
  identity metadata caveat.
- Document schema v2, `.toniator` container v1, and the immutable v1
  parser/migration remain unchanged. The accepted persistent `StringList`
  selector correction (splice, deferred rebuild, and invalid-position
  rejection) is included in the checkpoint as a bounded GTK correction, not
  as GTK preset UI.

**Stop condition (complete):** Headless preset reconstruction, standalone
versioning, independent versus explicitly disclosed shared application, and
canonical output parity were accepted at implementation checkpoint `9919d85`.
The documented selector correction does not add GTK preset/pattern editing.

## Stage 19B — Feedback-ready GTK application remediation

**Status: Complete at commit `b0b84e4`.** The first descriptor-driven Pattern
Editor failed artist-usability review: raw artwork could not apply a Random or
Grid pattern, the window edited the main document immediately instead of owning
a private draft, Blueprint was only an unused probe, and engine terminology
dominated the workflow. This accepted remediation supersedes that implementation
without changing the accepted headless document, command, history, invalidation,
scheduler, persistence, preset, rendering, or canonical-output authorities
unless a focused test proves a defect. The user accepted the implementation at
the local `b0b84e4` checkpoint.

- Split `toniator-app` into an application model with no widgets, a typed-intent
  controller, immutable document/channel/pattern/lifecycle/preview view models,
  a generation- and ticket-aware preview coordinator, and GTK components that
  render view models and emit intents. Async completion returns through GLib
  main-context messages rather than one universal polling loop.
- Make Blueprint/GResource the actual composition system for the adaptive main
  shell, persistent channel editor, stable pattern-catalog rows, separate
  Pattern Editor, and dialogs. Stable IDs, unique accessible names, predictable
  focus order, and persistent list models are part of the component contract.
- The main window presents artist-facing channel selection, visibility, color,
  opacity, source mapping, named pattern selection, density/aspect lock,
  rotation, X/Y offset, compatible mark sizing, visible Undo/Redo, and an
  explicit **Edit Pattern...** action. Bundled **Even Random Circles** and
  **Straight Grid Circles** apply immediately through
  `PresetRegistry::apply_to_selected` as one undoable copy-on-edit transition.
  Shared replacement remains a separate affected-channel disclosure and
  confirmation workflow.
- The document-modal Pattern Editor owns a cloned private document/history
  draft and a simplified preview using the existing scheduler and canonical
  rendering path. It exposes only currently supported Grid and Random structure,
  with modulation, exclusion, coverage, and safety under Advanced. Cancel and
  standard close discard the draft, confirming when dirty. **Save as Preset...**
  remains visibly disabled as later-stage work; no preset storage or library
  management is added here.
- Numeric controls commit once per completed gesture or Enter. Choices and
  toggles may commit immediately. Loading, error, keyboard, focus, and
  accessibility states use artist language and never treat descriptors or
  engine IDs as the product interface.
- An internal opt-in JSONL event sink may observe immutable synchronization
  events for workspace generation, document revision, selected channel, active
  family, dirty/savepoint and lifecycle state, submitted/accepted preview
  identity, and export completion. It is evidence infrastructure, not document
  authority or a production control API.

**Stop condition (complete):** The stage-owned report under
`target/validation/stage-19b-gui-remediation/` records the bounded private Sway
GTK/AT-SPI evidence, responsive header/sidebar placement, selected preset
projection, direct persistence and canonical PNG/SVG witnesses for both
immutable inputs, and focused private Pattern Editor transition tests. The
private harness has explicit limits: portal dialogs are external surfaces,
UI-driven export encountered a private-session keyring surface, and injected
WayVNC keyboard/pointer actions did not reach GTK. Direct boundary tests cover
canonical output, but these results do not claim manual GNOME Shell/Mutter or
exhaustive usability acceptance. The accepted implementation checkpoint is
`b0b84e4`; **Save as Preset...** remains disabled and preset authoring,
library management, and Stage 20C+ work remain planned.

## Stage 20+ — Advanced reusable mechanisms

**Stage 20A — Complete at commit `b7fbd81`.** The accepted headless
geometry/pattern interchange publishes `FamilySiteSet` as the one
deterministic, truthful derived-site authority for each typed family result.
`TypedFamilyOutput` is an opaque result; generalized intersections,
along-guide sites, and random sites retain their actual provenance rather than
being represented as fabricated intersections. A private compatibility adapter
is used only by the existing circular realizers, preserving accepted circle
IDs, contributor bytes, realization/cache identity, canonical PNG/SVG output,
and UI behavior. No schema, persistence, cache-key, canonical primitive,
renderer, or GTK workflow changed. Focused complete-document cache/output
checks cover natural 1024×1024 PNG and 900×620 SVG inputs, treating SVG live
text structurally; read-only review passed. No GTK evidence was required for
this headless-only checkpoint.

**Stage 20B — Complete in the Stage 20B acceptance checkpoint.** The user approved the bounded canonical
curve/path geometry contract in
[`STAGE_20B_CANONICAL_CURVE_PATH_PLAN.md`](STAGE_20B_CANONICAL_CURVE_PATH_PLAN.md).
This headless geometry-only checkpoint adds validated finite line/polyline and
cubic Bézier path mathematics, deterministic bounded evaluation, bounds,
arc-length lookup, intersections, and ordered clipping without introducing a
persisted structure, consumer, canonical render-output variant, or
canvas-created topology. Exactly one `desktop_implementer` completed the
implementation allowlist; focused verification and independent read-only review
pass. User acceptance is complete. The single acceptance checkpoint includes
the implementation, the authorized current-format real-world `.toniator`
fixture, and durable documentation; this text intentionally does not invent a
self-referential checkpoint hash. Semantic-map was not used because direct
source, `rg`, Cargo, Git, and the architecture validator were more efficient
for this isolated new geometry subsystem.

**Stage 20C — Complete in the Stage 20C acceptance checkpoint.** The accepted bounded headless contract adds only
document-owned authored open paths and closed shapes, authoritative commands,
descriptors, history, deterministic current-v2 persistence, and exact
conversion to Stage 20B construction geometry. It adds no consumer, evaluator,
cache, canonical output, renderer/export, CLI, GTK, preset, or later-stage
behavior. The exact focused gate and independent read-only review pass, and the
user accepted the implementation and separately authorized its single local
checkpoint on 2026-08-13. The checkpoint includes the implementation and
synchronized durable documentation and is intentionally named rather than
self-referenced by hash.

**Stage 20D — Complete in the Stage 20D acceptance checkpoint.** The bounded
decision-complete contract in
[`Stage20D_planning_contract.md`](../Stage20D_planning_contract.md) defines
generic authored-open-path and circular-arc guide prototypes, baseline/phase
transforms, Single and TransformStack repetition, exact conservative coverage,
existing guide-site product consumption, identity/invalidation, current-v2
persistence, focused tests, and its implementation/review gates. The user
approved this planning contract on 2026-08-13. The complete implementation
began after a separate explicit request from exact planning checkpoint
`453104e39204afc1e10397b9d5bbf551dd85deac`; the focused gate and independent
read-only review pass. The implementation's document-aware resolution and
cache boundaries include resolved authored guide identity, and current-v2
persistence validates rebuilt guide resources before publication. A narrow
post-review `toniator-app` compilation correction adds presentation labels for
the new typed guide fields, choices, and authored references only; it does not
expose Stage 20D editing UI or move authority into the frontend. Private
Sway/AT-SPI evidence is automated only and does not claim manual GNOME/Mutter
acceptance. The acceptance checkpoint is intentionally named rather than
self-referenced by hash.

**Stage 20E1 — Complete in the Stage 20E1 acceptance checkpoint.**
[`STAGE_20E1_NORMALIZED_MARK_FILL_PLAN.md`](STAGE_20E1_NORMALIZED_MARK_FILL_PLAN.md)
replaces the temporary absolute mark-size/support-capability model with a
per-site nominal cell basis, normalized 0..2 fill response, complete derived
coverage, an intentional current-format transition, and synchronized existing
GUI/CLI controls. Current documents are schema v3 only and presets are format
v2 only; obsolete document-v1/v2 and preset-v1 decoders are rejected rather
than migrated. Focused verification and the independent repair re-review PASS;
the accepted checkpoint includes family-aware coverage preflight, explicit
per-site nominal diameters, and deterministic parallel/near-parallel contributor
rejection. It does not implement authored shapes or renderer algorithms.

**Stage 20E2 — Complete at commit `0c6b6a2e268f9306835038be747352a0cd64044c`.**
[`STAGE_20E2_USER_SHAPE_MARK_PLAN.md`](STAGE_20E2_USER_SHAPE_MARK_PLAN.md)
defines authored closed structures as ordinary canonical site marks. The
implementation adds document-owned typed shape references, exact normalized
closed-path realization, shared canonical preview/PNG/SVG consumption,
even-odd fill, bounded/cancellable path work, complete identity, and additive
current document-v3/preset-v2 persistence. At that checkpoint, those formats
were current; Stage 20N later supersedes them with schema v5 and preset v3.
Focused verification and intrinsic
immutable-source artifacts pass. The independent review's sampled-paint and
identity findings were repaired, and the focused repair re-review plus final
zero-alpha engine-to-render review found no confirmed remaining issue. The user
explicitly accepted Stage 20E2 on 2026-08-14. The local implementation
checkpoint contains the reviewed implementation and deliberate HolidayMugs
fixture/checksum update; Stage 20F+ remains separately gated.

**Stage 20F — Complete at commit
`7117e24b8c9e2e723c3c23e7e9050dc71277d15c`.**
[`STAGE_20F_GUIDE_SHAPE_EDITOR_PLAN.md`](STAGE_20F_GUIDE_SHAPE_EDITOR_PLAN.md)
defines the bounded two-modal descriptor-driven GTK workflow: the provisional
**Edit guide paths…** action exposes authored open guides, while **Edit mark
shapes…** exposes authored closed mark shapes. These are private-draft
authored-resource editors, not the final Pattern Wizard. It includes reusable
path editing, typed use disclosure, private-history squash, and accessible
main/draft pending-preview feedback.
It preserves current document schema v3, preset format v2, evaluator/cache and
canonical-output contracts. A real GNOME/Mutter run exposed a
construction-gesture crash, ineffective new-resource application, and an overly
narrow editor; the same implementation writer repaired those failures. Focused
verification and independent test and UX reviews pass. The user explicitly
accepted Stage 20F on 2026-08-21. Publication remains excluded; Stage 20G is
separately gated below.

**Stage 20G — Complete at commit
`de1320ba359beee42223ef994baebfd9ecd94c9c`.**
[`STAGE_20G_EFFECTIVE_PATTERN_AUTHORITY_PLAN.md`](STAGE_20G_EFFECTIVE_PATTERN_AUTHORITY_PLAN.md)
records the accepted bounded implementation of effective pattern authority. It
moves shared settings to a document base, retains only channel replacements and
additive deltas, and uses current-only schema-v4 persistence. The user explicitly
accepted Stage 20G on 2026-08-21; that acceptance did not itself authorize
Stage 20H or later work.

**Stage 20H — Complete at commit
`4b1cc08819eee36c2009e2abf5543dcaefe29929`.**
[`STAGE_20H_CAPABILITY_PROJECTION_PLAN.md`](STAGE_20H_CAPABILITY_PROJECTION_PLAN.md)
defines the bounded headless capability-projection implementation. It derives
read-only typed structural facts from the document base or a Stage 20G
effective channel definition, without introducing a second evaluator,
serialization, invalidation, cache identity, CLI, or GTK authority.
Focused domain/patterns/engine witnesses and bounded static checks pass, and
the independent implementation review passed. The user explicitly accepted
Stage 20H on 2026-08-21; publication remains separate.

**Stage 20I — Complete at implementation checkpoint
`de166f533379dc5b75d5a36e38baf145d0fac6c2`.**
[`STAGE_20I_CANONICAL_PATHS_STROKES_PLAN.md`](STAGE_20I_CANONICAL_PATHS_STROKES_PLAN.md)
records the accepted canonical guide-path output and reusable geometry-owned
compact variable-width filled-outline implementation. It preserves Stage 20G
effective authority, current schema v4/preset v2, final-consumer clipping,
native RGBA PNG/SVG parity, and the Holiday mark regression witness. The user
accepted Stage 20I on 2026-08-21; publication remains separate.

**Stage 20J — Complete at implementation checkpoint
`2edbb8659a82106ce8de904ef1ce9155e3b4d777`.**
[`STAGE_20J_PATH_OFFSET_CONSTANT_GAP_PLAN.md`](STAGE_20J_PATH_OFFSET_CONSTANT_GAP_PLAN.md)
records the accepted persisted absolute-gap `NormalOffset` repetition and the
reusable geometry-owned compact line/cubic centerline offset service. Coverage,
crossing cleanup, component identity, current-v4 persistence, Stage 20I outline
reuse, native RGBA PNG/SVG output, and the Holiday regression remain within the
accepted authority split. The user accepted Stage 20J on 2026-08-22;
publication remains separate. This historical terminology is retained for the
accepted checkpoint: current Stage 20S authority calls `NormalOffset` positive
parallel-centerline spacing and does not reuse it for region negative-space or
absolute-gap computation.

**Stage 20K — Complete at implementation checkpoint
`f848ff995c9e30f89a85fbc01b5b8d97cc8de3d5`.**
[`STAGE_20K_PARAMETRIC_CURVES_PLAN.md`](STAGE_20K_PARAMETRIC_CURVES_PLAN.md)
records the accepted headless parametric-curve family: finite round and square
spirals, raw canonical curve paths or equal-arc curve sites, reusable
repetition, current schema-v4 intent-only persistence, and canonical PNG/SVG
output. Verified intrinsic evidence uses five full turns with artboard-derived
pitch for both immutable inputs; all eight native PNG outputs and all eight
Inkscape-rendered SVG outputs were inspected directly. Bounded adaptive
five-point Gauss-Legendre arc-length measurement and row-active outline
filtering keep the complete eight-artifact matrix within the existing limits
without changing geometry ownership or final-consumer clipping. The user
accepted Stage 20K on 2026-08-22; publication remains separate.

**Stage 20L — Complete at implementation checkpoint
`b41fa3fcf2e1089ea422ba18524c2c4a26f568e8`.**
[`STAGE_20_PLUS_DECOMPOSITION.md`](STAGE_20_PLUS_DECOMPOSITION.md) provides the
detailed synchronized Stage 20L contract. The user accepted the
implementation on 2026-08-23. It provides
deterministic, mechanism-neutral derived site topology across geometry,
patterns, and engine, with guard-inclusive evaluation, bounded cancellable
construction, and no persisted connection intent or renderable paths. Focused
and broad affected-package verification is green, and the final independent
read-only implementation review found no material issue. The checkpoint also
contains the user-edited `.agents/skills/toniator-orchestrator/SKILL.md`, which
remains user-owned guidance and is not product authority. Publication and Stage
20M remain separate gates.

**Stage 20M — Complete at implementation checkpoint
`33f1bde3be9afdc3fb88f479c4ee7ec52b80114a`.** The user authorized the bounded headless
connection-program implementation on 2026-08-23 and accepted it on 2026-08-24 under
[`STAGE_20M_CONNECTION_PROGRAMS_PLAN.md`](STAGE_20M_CONNECTION_PROGRAMS_PLAN.md).
The implemented contract provides conventional wall-maze semantics over the
actual straight-grid intersections on or inside the canvas. Every inclusive
site remains candidate and fingerprint authority; geometry connects consecutive
real guide sites, extracts every positively oriented bounded face, and selects
the largest stable connected face component only when finite candidates are
disconnected. It emits only walls bounding selected cells, with no degree test,
fixed shell, stroke-width inset, or site-clearance policy. Positive wall width
and caps may extend past the canvas and are handled only by final canonical
clipping. The selected cells form one dual spanning tree with exactly two
deterministic perimeter openings and one derived cell-to-cell solution. A
rectangular final clip can leave disconnected transparent fringe outside bounded
maze cells because the canvas never invents closure.
Positive grid spanning trees remain a separate connection-path output. Focused
verification, artifact inspection, independent read-only review, bounded repair
re-review, and the final centered-origin review found no material findings. The
accepted headless scope retains the centered grid-prototype origin, positive
nearest/random/tree paths, normalized `0.0..=2.0` response, no GTK work, and no
renderer topology repair. The checkpoint-era current-v4/preset-v2 persistence
is superseded by Stage 20N's current schema-v5/preset-v3 boundary.
A final requirement audit
also made two-/three-guide geometry coverage direct and added public
connection/maze capability-projection coverage without changing production
behavior. Publication remains separate.

**Stage 20N — Complete at implementation checkpoint
`b8701686042a69fcd1ac68a4038adbad4c0ccdc9`.** The accepted headless foundation
adds atomic bundles with ordered keyed output settings and channel deltas,
explicit per-output realization/cache units with maximum-support aggregation,
canonical-region identity/normalization/validation, and ordered render-output
layers with solid nonzero region fills and final canvas clipping. Schema v5
documents and preset v3 recipes persist authored settings only; effective
values, regions, diagnostics, limits, caches, and scheduler state remain
derived. The one-output authoring/validation gate remains intentionally active,
so concrete region sources, treatments, heterogeneous composites, and GTK
workflow remain outside this stage. Focused tests, intrinsic native artifacts,
and independent correction re-review passed.

**Stage 20O — Complete at implementation checkpoint
`7ab97f01ec372ab1e6201b3913742476a1511c02`.** The user accepted Ordinary
Voronoi Regions on 2026-08-25 after independent re-review and final artifact
inspection. Its headless authority accepts eligible `FamilySiteSet` products,
including along-guide and `AlongParametricCurveSites`, rejects direct raw
`ParametricPaths`, preserves duplicate co-ownership through a geometry-private
Spade adapter, persists authored v5/v3 intent only, and renders fixed solid
Full regions with final clipping only. Stage 20R is now complete; Stage 20S
remains Planned and separately gated.

**Stage 20P — Complete at implementation checkpoint
`cd531eb65dd2e161e62f355905ad936b8c1ca3c4`.** The user accepted Guide
Arrangement Faces on 2026-08-25 after independent read-only review and final
artifact inspection. Its headless authority derives deterministic complete
bounded faces from two or three selected straight or authored-open guide
dimensions through the normal production family evaluator, preserves the
shared centered document origin and Stage 20M maze identities, persists
authored v5/v3 intent only, and renders canonical fixed Full regions with
final clipping only. The phase-aligned 0/60/120 witness proves equal physical
spacing and three-line equilateral faces. Existing generic one-through-four
guide support remains unchanged; Stage 20P adds no four-guide Guide Faces
behavior or evidence. Direct raw `ParametricPaths` remain Guide-Faces-ineligible,
while typed parametric site/Voronoi mechanisms remain valid. Stage 20R is now
complete; at that checkpoint, Stage 20S remained Planned and separately gated.

**Stage 20Q — Complete at implementation checkpoint
`071f3604098c0660a876fbe30050a64223fe41b3`.** The user reaccepted Filled-region
Realization on 2026-08-26 after the repaired implementation passed independent
review, focused verification, strict checks, protected-input and architecture
gates, read-only semantic-map impact/navigation/freshness reconciliation (not
`semantic-map check` or architecture authority), and intrinsic native PNG/SVG
inspection.
Positive ConstantGap shrinks and negative ConstantGap grows; convex outward
growth uses subdivided smooth cubic round joins, while inward shrink uses
tangent intersection plus crossing/coincident-branch dissolution. The
three-guide evidence uses positive inward gap and triangular line rings. The
authored-cubic outward witness uses fixed `-40` gap, producing 20-unit outward
edge growth and 40-unit neighbor overlap with smooth joins. Collapse evidence
is intentionally transparent; sparse authored-cubic coverage reflects six
complete bounded faces, not raster resolution. Schema-v5/preset-v3 persists
authored intent only, and the headless, final-clip-only, and
no-four-guide/raw-ParametricPaths boundaries remain in force. Publication
remains separate. This historical Stage 20Q record is superseded for current
region authority by Stage 20S normalized positive-only Scale/UniformOffset
fill; no Full or ConstantGap branch, absolute gap, or negative-space geometry
is current behavior. Stage 20S remains separately gated.

**Stage 20R — Complete at implementation checkpoint
`458c9a981dd349999240a18052e055a71c7b6c3c`.** The user accepted the bounded
ordered-composite and site-use-filter implementation on 2026-08-26 after
independent read-only review, parent verification, and direct native PNG/SVG
inspection. Stage 20R lifts the one-output gate, normalizes ordered typed
output layers with `All`, `SitesUsedBy`, and `SitesUnusedBy` filters, derives
site usage before final clipping, evaluates a deterministic dependency DAG
separately from authored painter order, and persists only authored v5/v3
intent. It also provides per-output cache identity, request-wide composite
limits, atomic cancellation/stale-publication behavior, and canonical
connection, maze, mark, path, and treated-region consumption. The focused
evidence keeps connection, maze, and sampled-region visual witnesses isolated;
the cross-channel witness verifies connections and regions remain separate
when authored in different channels. The implementation remains headless and
adds no GTK workflow, renderer topology repair, compatibility adapter,
publication, or Stage 20S work. The implementation checkpoint is
`458c9a981dd349999240a18052e055a71c7b6c3c`; the documentation closeout is
tracked separately.

**Stage 20N+ history and remaining roadmap — Stage 20S Complete at implementation commit
`55651dee7c744c2aa207924bf0dbb7737609942d`.** The user accepted the revised headless
remainder roadmap on 2026-08-24 under
[`STAGE_20N_20S_HEADLESS_PATTERN_COMPLETION_PLAN.md`](STAGE_20N_20S_HEADLESS_PATTERN_COMPLETION_PLAN.md).
Stage 20Q and 20R are complete; 20S completes headless capability projection
and ordinary gallery recipes. Stage 21 owns all remaining pattern-authoring GTK work, Stage 22 owns
the complete headless frame/media/sequence/simple-transition pipeline, and
Stage 23 owns temporal GTK with start/end pins only. Stage 20S completed its
headless implementation, including capability/descriptors, strict nested
preset-v3 DTO rejection, the 16-card registry after retiring
`regions-plus-marks`, normalized positive-only Scale/UniformOffset regions,
RGB-component/seed evidence, CoverCanvas spirals, and centered-local curved
guides. The user accepted it on 2026-08-26 after independent review/re-review,
verified evidence, and parent intrinsic RGB/alpha inspection. `semantic-map
check` is unavailable and inapplicable because Toniator has no semantic-map
architecture schema; project documentation is authority and
`scripts/validate_architecture.sh` is mechanical validation only. The
integrated final scrub is complete at implementation checkpoint
`dc7e988200c5be4d22791ca1d231336caac19a24` (accepted 2026-08-27); its durable
architecture and concurrency record includes the full-resolution scaling
proof. Stages 21–23 remain Planned. The GTK4/Blueprint re-baseline is recorded
below; push, publication, and every later stage remain separately gated.

**Final Stage 20 scrub — Complete at implementation checkpoint
`dc7e988200c5be4d22791ca1d231336caac19a24`.** The user accepted the integrated
Stage 20A–20S scrub on 2026-08-27. It reconciles Stage 20A–20S under the final
effective-pattern, multi-output, current-v5, normalized-region, capability,
and 16-card catalog authorities. It repairs confirmed bounded contract and
coverage defects and records the final evaluator/cache/concurrency design and
full-resolution scaling proof in
[`STAGE_20_FINAL_ARCHITECTURE_AND_CONCURRENCY.md`](STAGE_20_FINAL_ARCHITECTURE_AND_CONCURRENCY.md).
One complete evaluation remains the cancellation and transactional-publication
unit; deterministic per-site, per-region, and per-pixel work uses the bounded
shared Rayon pool, while global topology, dependency/budget traversal, painter
order, and publication remain serial. The retired `regions-plus-marks` debug
tool remains absent. The GTK4/Blueprint re-baseline is recorded below; Stage
21, publication, and push remain separately gated.

## GTK4/Blueprint re-baseline

**Complete in the accepted local checkpoint `ToniatorGUI` (2026-08-27).**
This bounded infrastructure change removes libadwaita from `toniator-app` and
uses GTK4-only widgets. Blueprint sources compiled by the build script and
registered in the GResource own the static main shell, private Pattern Editor
shell, and PNG-export-options structure. Rust remains authoritative for the
document and commands, dynamic descriptor/catalog rows, signal handlers,
preview scheduling, canvas interaction, export validation/submission, and the
GTK-only responsive allocation policy.

The accepted Stage 19B workflow and canonical preview/PNG/SVG paths remain in
place. This re-baseline does not decide or implement Stage 21's workflow,
information architecture, pattern wizard, expanded capability exposure, or
broader controller decomposition. Focused Blueprint compilation, resource
registration, app checks and strict Clippy, formatting, architecture
validation, and diff checks passed. Fresh private Sway/AT-SPI/grim evidence
covered raster and SVG inputs, normal and narrow layouts, sidebar visibility,
Pattern Editor reflow, draft/discard actions, and accessible control actions.
The evidence is automated wlroots evidence and native artifact inspection; it
does not claim manual GNOME Shell/Mutter acceptance.

Every mechanism must enter through the Stage 14 typed schema, Stage 15 generic
pipeline, Stage 17 command/descriptor contract, canonical geometry, and
final-consumer clipping. Artistic names remain pure-schema presets and adequacy
tests; they never become private variables, renderer branches, or alternate
evaluation paths.

## Common validation and Git gates

Unless a bounded stage contract specifies a proportional checkpoint gate,
every stage ends with:

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

Stage 9A–9D use their focused crate tests plus workspace check, strict Clippy,
architecture, asset-hash, protected-path, and diff/status gates before review.
Stage 9E runs the complete workspace test gate once after the last executable
Stage 9 change. This keeps every accepted local checkpoint independently
reviewable without repeating the full integration suite after every bounded
slice.

At each stage or Stage 9 substage transition:

1. Mark only the approved stage **In progress**.
2. One writer implements only its allowlist.
3. Record **Implemented awaiting review**.
4. Obtain automated and, where applicable, visual acceptance.
5. Mark **Accepted awaiting checkpoint**.
6. User acceptance authorizes the required local implementation checkpoint;
   do not begin the next substage before it exists.
7. Record the implementation SHA and **Complete at commit** status in a local
   documentation closeout commit before beginning the next substage.
8. Push only after separate explicit authorization; a local checkpoint does
   not imply a push.
9. Start the next substage from that accepted documentation checkpoint and a
   clean tracked worktree, then stop at its own review gate.

## Legacy quarry procedure

Assign one named responsibility per quarry. Record hidden dependencies and
characterization tests, adapt the algorithm to the receiving greenfield
interfaces, remove GTK/renderer/global/persistence assumptions, and perform an
isolated architecture review. Never copy a whole legacy module or treat its
structure as authority. Legacy remains read-only.
