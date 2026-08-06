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
Stage 1 is committed at `567d307`; Stage 2 is accepted but remains
uncommitted in the current worktree until its checkpoint is made.

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

**Accepted awaiting checkpoint (implemented in the dirty worktree; not
committed).** Added validated authoritative in-memory domain state, stable IDs,
continuous `f64` layout/appearance values, validated commands and invalidation
levels (`Presentation`, `Realization`, `Family`, `Source`), immutable
`Document::apply_command`, `DocumentSession` revision ownership, and stale
evaluation-token rejection. Added headless `toniator validate` and nine
integration tests (four domain, three engine, two CLI). Verified workspace
format/check/clippy/tests, architecture validation, valid and invalid CLI
paths, and protected-spec/Legacy diffs. Stage 2 intentionally has no geometry,
rendering, persistence, source decoding, async evaluation, or GTK.

## Stage 3 — straight-guide family output (next bounded stage)

**Status: Planned; not authorized or started.** Implement only the bounded
family-output slice below. Do not begin marks or rendering in this stage.

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
presentation. Prove shape-size changes reuse the same Stage 3 sites.

### Stage 5 — RenderScene and preview/export consumers

Consume the same `RenderScene` from a shared headless `RasterSurface` (for
future preview and PNG) and SVG writer. Add CLI render with output-extension
selection, RGB PNG black background, CMYK PNG white background, and an explicit
transparent option. Clip only at final output.

Use this fixed reference: 900×600, 90×60, rotation 17°, offsets 3.25/−4.5,
channel color `#00b7ff`, opacity `0.72`. Inspect artifacts with `identify`,
`xmllint`, Inkscape SVG rasterization, ImageMagick RMSE `<= 0.02`, and visual
side-by-side inspection before accepting goldens.

The slice excludes curves, random, maze, Voronoi, regions/offset, video,
animation UI, plugins, and legacy import. GTK editing remains a later stage.

## Later roadmap (high level only)

- Stage 6: asynchronous scheduling, cancellation, caches, and revision safety.
- Stage 7: view-only GTK preview.
- Stage 8: command bindings, undo/redo, current persistence, and editors.
- Stage 9+: generalized families, connected output, regions, multiframe
  evaluation, and simple transitions. Each item receives a newly scoped and
  approved short-stage contract before implementation; these are not settled
  implementation specifications.

## Legacy quarry procedure

Assign one named responsibility per quarry. Record hidden dependencies and
characterization tests, adapt the algorithm to the receiving greenfield
interfaces, remove GTK/renderer/global/persistence assumptions, and perform an
isolated architecture review. Never copy a whole legacy module or treat its
structure as authority. Legacy remains read-only.
