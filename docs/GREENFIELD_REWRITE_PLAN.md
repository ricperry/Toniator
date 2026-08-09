# Toniator Greenfield Rewrite Plan

Status: approved execution roadmap and stage contract (2026-08-08)

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
`02bc2c9`; Stage 15 remains planned.

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
parity were accepted at implementation checkpoint `88fc6dd`. Generalized
evaluation remains deferred to Stage 15.

## Stage 15 — Generic pattern evaluation pipeline

**Status: Planned.** Generalize engine dispatch and cache identity around the
typed mechanism contract before adding another family.

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

**Stop condition:** Accept the generic headless pipeline and parity before the
first expanded family mechanism.

## Stage 16A — Generalized straight-guide mechanisms

**Status: Planned.** Add reusable straight-guide vocabulary through the generic
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

## Stage 16B — Random and site-distribution mechanisms

**Status: Planned.** Immediately prove the Stage 14–15 architecture is not
grid-shaped by adding reusable deterministic site distributions.

- Add raw uniform random, genuinely even/exclusion-based placement, clustered
  placement, and source-weighted placement with stable `u32` seeds.
- Support minimum center spacing, visible-mark exclusion margin, deterministic
  achieved-density diagnostics when a request is unsatisfiable, and bounded
  candidate/work policies.
- Reuse the same modulation, mark/output realization, channel response,
  canonical geometry, clipping, cache, preview, PNG, and SVG machinery used by
  Stage 16A.
- Treat Poisson-disk methods or blue-noise measurements as defined reusable
  constructions/quality evidence, never unexplained named generator branches.

**Stop condition:** Accept deterministic distribution distinctions, exclusion
guarantees, weighted sampling, and native output parity before editor commands.

## Stage 17 — Headless pattern/channel editing and capabilities

**Status: Planned.** Complete the authoritative command and introspection
surface before creating GTK controls.

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
before GTK inspector work.

## Stage 18 — Descriptor-driven GTK channel inspector

**Status: Planned.** Add channel selection and per-channel editing over Stage
17 authority without structural pattern mathematics in GTK.

- Present channel appearance, source mapping, definition selection/sharing
  state, family-appropriate density, layout, and output-compatible geometry
  response from authoritative descriptors.
- Use progressive disclosure: common compatible controls first; advanced
  anisotropy, seeds, exclusion, and mechanism-specific parameters only when the
  active descriptor exposes them.
- GTK renders widgets, parses transient text, and dispatches typed commands
  through `DocumentHistory`. It never mutates definitions, allocates IDs,
  computes pattern geometry, or maintains hidden authoritative values.
- Preserve asynchronous revision/ticket rejection under rapid editing and keep
  the preview a raw-RGBA final consumer.

Structural Pattern Editor controls and preset authoring remain Stage 19.

**Stop condition:** User accepts channel workflow, undo/redo, focus/accessibility,
stale-preview rejection, and all supported model/source combinations.

## Stage 19A — Pure-schema preset registry

**Status: Planned.** Add a versioned headless preset registry whose entries are
ordinary typed pattern definitions using only exposed mechanisms.

- Applying a preset creates an independent document-owned definition by
  default. Updating an existing shared definition requires an explicit shared
  operation and affected-channel disclosure.
- Preset names, categories, and thumbnails are metadata only. Removing a preset
  removes the shortcut, not evaluator capability; no evaluator/cache/renderer
  branch may inspect a preset name.
- Reconstruction tests build every bundled preset from a blank definition using
  exposed typed controls, serialize/reload it, and compare canonical output.

**Stop condition:** Accept headless preset reconstruction and versioning before
GTK preset/pattern editing.

## Stage 19B — Structural GTK Pattern Editor

**Status: Planned.** Add a separate structural editor launched from selected
channel context and driven by Stage 17 descriptors.

- Default ordinary editing uses the atomic copy-on-edit command when the
  selected definition is shared. A separate deliberate **Edit Shared
  Definition** operation shows every affected channel before dispatch.
- Edit the typed family, mechanisms, modulation, coverage, and ordered output
  layers supported by the current evaluator. Raw schema JSON is not the primary
  workflow, and unsupported future mechanisms are not exposed.
- Transient widget/draft text is non-authoritative. Valid edits commit through
  typed commands, history, exact invalidation, and the shared scheduler/preview.
- Preset application creates an independent definition by default; deliberate
  shared replacement remains explicit.

**Stop condition:** User constructs, saves, reloads, edits, shares/copies, undoes,
and renders representative grid and random definitions without hidden or
named-pattern behavior.

## Stage 20+ — Advanced reusable mechanisms

**Status: Planned.** Continue through separately approved headless mechanism
and GTK exposure checkpoints: curved/procedural guide generators and coverage,
connected/network topology, regions and ordinary Voronoi, reusable region
offset/collapse behavior, composite output mechanisms, user-authored paths or
structures, multiframe sources, and simple transitions.

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
