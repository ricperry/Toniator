# TON-010 bundled recipes — Substage 3E2A implementation evidence

Date: 2026-08-01
Repository: `/home/ricperry1/projects/Toniator`
Git HEAD inspected: `262c7e857446ded100d4a90fd23d651e52460665`
Producing agent: `desktop-implementer`

## Scope completed

Implemented only the first executable Curves-native boundary:

- Added typed Curves placement, source-sample, and motif runtime values, plus
  reserved values for the already-declared later ports.
- Added a bounded Curves native registry and its preflight hook. Only
  `curves.placement`, `curves.source-sample`, and `curves.motif-selection` are
  working in this substage.
- Placement reuses `calculate_web_grid`, checks typed scalar inputs, artboard
  equality and cancellation, carries only its downstream grid transform scalars
  (not a defaulted Curves settings/channel facade), and rejects source lattices
  above one million cells before a source request.
- Sampling consumes only typed placement, requests declared dimensions through
  `RecipeSourceFieldProvider`, and preserves the supplied CMYK/RGB
  `OutputChannelId` exactly.
- Motif selection resolves digest-addressed definition assets. Its strict SVG
  decoder accepts only the adapter's exact SVG wrapper containing `M` then
  1..=64 finite cubic `C` commands.
- Generic preflight walks every `curves.motif-selection` node and resolves its
  actual literal or parameter bindings at their declared pattern/output-channel
  scope for the selected semantic channel. Coverage proves malformed literal
  and selected per-channel assets fail before a native node runs.

The retained renderer remains authoritative. No live `RenderVariant::WebCurveV1`
dispatch, Curves orchestration, document schema, preset, UI, preview, PNG, or
SVG export behavior changed.

## Exact files changed for this substage

- `src/curves_native.rs` (new)
- `src/curve_render.rs` (only `sample_curve_path` is `pub(crate)` for the
  retained motif seam; its algorithm and call sites are unchanged)
- `src/pattern_definition.rs` (Curves `RecipeRuntimeValue` variants and typed
  port mapping)
- `src/lib.rs` (Curves-native module/type/registry exports)
- `.codex-work/agents/desktop-implementer/ton-010-bundled-recipes-substage-3e2a.md`

The shared worktree already contained accepted 3A–3E1 changes. They were
preserved; this record does not claim those preceding files as 3E2A work.

## Decisions and reused abstractions

- Reused the existing strict pattern/asset contracts,
  `NativeRecipeOperationRegistry::with_preflight`, typed recipe ports,
  `RecipeExecutionContext`, cancellation, `RecipeSourceFieldProvider`,
  `DistributionField`, `OutputChannelId`, and Curves setting models.
- Reused retained `calculate_web_grid` and `curve_render::sample_curve_path`
  exactly. The native-decoded/default retained cubic samples are asserted equal;
  no duplicate motif sampling math was added.
- The decoder intentionally accepts only the canonical adapter representation,
  rejecting missing digests, non-cubic/incomplete/malformed paths, transforms,
  and other SVG structure rather than interpreting a second SVG subset.
- `CurvesPlacement` is narrow like `ShapesLattice`: artboard, calculated grid,
  and only grid rotation/pivot/offset scalars downstream retained helpers need.
  It never stores `WebCurveSettings` or `WebCurveChannel` as a shadow authority.
- Preflight resolves each motif node from `RecipeNode::parameters`, not assumed
  bundled instance keys. A binding may be a literal asset or a definition
  parameter; the latter is selected from global values or the current semantic
  output-channel values by its declared scope.
- `curves.deformation`, `curves.width-modulation`, and `curves.emit-paths`
  remain scaffold entries whose bodies return an explicit unavailable error.
  They were neither exercised nor claimed as working; whole-recipe execution is
  intentionally incomplete in 3E2A.
- Static scan of `src/render.rs` and `src/curve_render.rs` found no
  `curves_native`, `CURVES_NATIVE`, `execute_recipe`, or `compat.curves`
  reference. Live retained dispatch/recipe entry is zero.

## Verification and artifacts

- `cargo fmt --check` — passed.
- `cargo test --locked curves_native --lib` — 6 passed: typed values and input
  failure, retained placement/motif equality, strict asset rejection, generic
  graph-driven literal/per-channel preflight with zero native invocations,
  RGB/CMYK source identity, cancellation, and resource bounds.
- `cargo test --locked curves_recipe --lib` — 5 passed.
- `cargo test --locked bundled_pattern_definitions --lib` — 4 passed.
- `cargo test --locked` — 220 library tests and 48 binary/UI tests passed.
- `cargo check --locked --release` — passed.
- `cargo clippy --locked --all-targets -- -D warnings` — passed.
- `git diff --check` — passed.
- `timeout 8s cargo run --locked` — built and launched
  `target/debug/toniator`; expected timeout exit 124.

No screenshot, preview, PNG, or SVG artifact was produced because this stage
does not change visible dispatch or export behavior. The launch smoke is not a
human GNOME/Wayland acceptance test.

## Limitations, review targets, documentation, and invalidation

- Later work must implement deformation, width interpolation/simplification,
  clipping, and canonical emission, then establish retained/native whole-recipe
  parity before executor or dispatch changes.
- The decoder must be reviewed with any future canonical asset serialization
  change. Curves recipe/adapter, asset/digest validation, settings/channel and
  execution-context semantics, retained placement/motif helpers, later native
  bodies, or live dispatch changes invalidate this evidence.
- Durable documentation is likely affected only when Curves execution or
  dispatch becomes live; this file is implementation evidence, not a substitute
  for milestone documentation reconciliation.

## Working-tree assumptions

The checkout was already materially dirty at the HEAD above, including prior
TON-010 modules, documentation, assets, and unrelated user work. It was
preserved: no reset, clean, commit, push, publication, deployment, or
destructive operation was performed.
