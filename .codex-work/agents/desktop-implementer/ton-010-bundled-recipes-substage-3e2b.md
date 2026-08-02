# TON-010 bundled recipes — Substage 3E2B implementation evidence

Date: 2026-08-01
Repository: `/home/ricperry1/projects/Toniator`
Git HEAD inspected: `262c7e857446ded100d4a90fd23d651e52460665`
Producing agent: `desktop-implementer`

## Scope completed

Implemented only `curves.deformation` on the accepted 3E2A boundary.

- The operation consumes typed `CurvesPlacement` and `CurvesMotif`, validates
  every accepted deformation parameter, and returns narrow
  `CurvesDeformedPaths`: placement provenance, repeated/transformed point paths,
  and the close/open decision needed by later modulation.
- It consumes layout, curve scale, coverage, bleed, tile/stack controls,
  alternate transforms, output quality, and the four maximum-width coverage
  inputs (`min-mark`, `max-mark`, `max-size`, `scale`).
- Retained deformation was factored into one borrowed helper seam,
  `curve_render::deform_curve_paths_cancellable`. Both retained rendering and
  the native operation use that exact sampling, full-width baseline/repetition,
  motif normalization, automatic/manual coverage, transforms, and resampling
  implementation. Temporary `WebCurveSettings`/`WebCurveChannel` facades are
  built only inside the native call and never stored in runtime values.
- Native-only limits reject excessive full-width candidates, motif path counts,
  chained tile point products, and resampling totals before their corresponding
  large allocations. Checked multiplication yields actionable overflow errors;
  retained cancellation checkpoints run per repeat/row and regularly inside
  tile expansion.

No modulation, emission, generic Curves orchestration, live dispatch, schema,
preset, UI, preview, PNG, or SVG export behavior was added or changed.

## Exact files changed for this substage

- `src/curve_render.rs`
- `src/curves_native.rs`
- `.codex-work/agents/desktop-implementer/ton-010-bundled-recipes-substage-3e2b.md`

All accepted shared work was preserved. In particular, the earlier 3E2A typed
values/preflight and the pre-3E2B ownership correction remain intact.

## Important decisions and reused abstractions

- Reused retained `sample_curve_path`, `sample_motif_path`, full-width baseline
  and transforms, motif normalization/tiling, `motif_counts`, `max_curve_width`,
  and retained cancellation behavior through a single extracted helper rather
  than copying geometry math into the recipe module.
- `CurvesDeformedPaths` retains no settings/channel facade. It carries only
  paths, placement/grid provenance, and `closed`; smooth joining has already
  been consumed when sampling the motif.
- The resource limits are native-recipe bounds: 10,000 paths, 20,000 points per
  path, and 1,000,000 total points. Retained compatibility rendering invokes
  the same geometry helper without these recipe-specific limits.
- `curves.placement`, `curves.source-sample`, and `curves.motif-selection`
  remain working. `curves.width-modulation` and `curves.emit-paths` remain
  explicit unavailable bodies and were not broadened.
- A retained render invocation leaves the Curves native-operation counter at
  zero, and static scan of `src/render.rs`/`src/curve_render.rs` finds no
  Curves native registry or recipe dispatch reference.

## Verification and artifacts

- `cargo test --locked curves_native --lib` — 11 passed. Includes retained
  full-width/motif equality, shared/per-channel-compatible motif semantics,
  smooth/closed sampling, quality, transforms, auto/manual coverage guards,
  typed failures, cancellation, resource rejection, valid generic preflight,
  unavailable later bodies, and retained-dispatch zero.
- `cargo test --locked curves_recipe --lib` — 5 passed.
- `cargo test --locked curve_render --lib` — 12 passed.
- `cargo test --locked bundled_pattern_definitions --lib` — 4 passed.
- `cargo test --locked` — 225 library tests and 48 binary/UI tests passed.
- `cargo check --locked --release` — passed.
- `cargo clippy --locked --all-targets -- -D warnings` — passed.
- `cargo fmt --check` and `git diff --check` — passed.
- `timeout 8s cargo run --locked` — built and launched
  `target/debug/toniator`; expected timeout exit 124.

No screenshots, previews, PNGs, or SVGs were produced: live rendering and
export dispatch remain retained and unchanged. The timeout smoke is not a
human GNOME/Wayland acceptance test.

## Limitations, review targets, documentation, and invalidation

- Native Curves execution is intentionally incomplete until the separately
  authorized width-modulation and emission bodies are implemented and parity is
  proven through the final canonical output. A valid whole Curves recipe still
  stops at width modulation's explicit unavailable error.
- Future work must preserve this helper seam and the narrow deformed-path value
  while adding source width mapping, clipping/simplification, and canonical
  outline emission. Reassess resource ceilings against final modulation memory
  behavior.
- Durable documentation is likely affected only once native Curves execution or
  live dispatch becomes complete; this evidence is not a documentation substitute.
- Invalidate this evidence if retained Curves sampling/repetition/coverage or
  width-footprint behavior changes; Curves parameter bounds/ports change; typed
  runtime values change; resource policy changes; or a later stage implements
  modulation, emission, orchestration, or live dispatch.

## Working-tree assumptions

The repository remained materially dirty at the HEAD above from accepted
TON-010 work and unrelated user edits. Those changes were preserved; no reset,
clean, commit, push, publication, deployment, or destructive operation was
performed.
