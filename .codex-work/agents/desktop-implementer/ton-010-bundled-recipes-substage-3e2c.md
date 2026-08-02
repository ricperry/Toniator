# TON-010 bundled recipes — Substage 3E2C implementation evidence

Date: 2026-08-01
Repository: `/home/ricperry1/projects/Toniator`
Git HEAD inspected: `262c7e857446ded100d4a90fd23d651e52460665`
Producing agent: `desktop-implementer`

## Scope completed

Implemented only `curves.width-modulation` on the parent-accepted 3E2A/3E2B
typed boundary.

- The operation consumes `CurvesDeformedPaths` and `CurvesSamples`, rejects
  missing/wrong types, placement/grid provenance mismatches, source-dimension
  mismatches, and artboard/context mismatches, then returns the narrow
  `CurvesModulatedPaths { artboard, outlines }` product.
- It validates and consumes `min-mark`, `max-mark`, `threshold`, `max-size`,
  `scale`, and `output-quality` with the retained semantic bounds and the
  min/max relation. No layer, color, opacity, enabled state, `CurveGeometry`,
  or persisted settings/channel facade crosses this boundary.
- `curve_render::modulate_curve_paths_cancellable` is the sole shared retained
  implementation of source interpolation, threshold mapping, active-segment
  splitting, clipping/margins, simplification, and open/closed outline
  construction. The retained renderer was routed through this helper; native
  execution calls it with typed samples rather than copying geometry math.
- Native limits reject paths over 20,000 points, totals over 1,000,000 input
  points, more than 10,000 outlines, and more than 4,000,000 commands. The
  helper checks cancellation before and during the retained repeat cadence and
  after completion. Saturating interpolation-neighbor arithmetic prevents
  out-of-range transformed coordinates from overflowing debug arithmetic while
  retaining the same clamped sampling behavior.
- `curves.emit-paths` remains deliberately and explicitly unavailable. A valid
  generic Curves execution now reaches width modulation and stops only at that
  emit boundary.

No emission, canonical-output construction, Curves live dispatch, persistence,
preset/schema, UI, preview, PNG, or SVG export behavior was added or changed.

## Exact files changed for this substage

- `src/curve_render.rs`
- `src/curves_native.rs`
- `.codex-work/agents/desktop-implementer/ton-010-bundled-recipes-substage-3e2c.md`

The existing dirty `src/curve_render.rs` and untracked `src/curves_native.rs`
also contain parent-accepted 3E2A/3E2B work; this substage preserved it.

## Important decisions and reused abstractions

- Reused retained `map_web_threshold`, bicubic scalar sampling, `max_curve_width`,
  `split_active_segments`, `clip_segment_to_artboard`, `simplify_segment`, and
  `outline_from_points` through one atomic helper.
- The helper takes a scalar callback so `ResolvedChannelField` (retained) and
  `DistributionField` (native) share the exact width/outline path without a
  compatibility adapter or duplicated field representation.
- `CurvesModulatedPaths` carries only the final typed outlines and artboard;
  the later emit stage remains the sole owner of layer/presentation policy.
- Native execution keeps its resource policy local by supplying limits; the
  retained renderer supplies none and retains its compatibility behavior.

## Verification and artifacts

- `cargo test --locked curves_native --lib` — 15 passed. Covers exact outline
  equality with the retained helper across full-width/motif layouts and
  open/closed paths; parameter sensitivity; zero-active and fully clipped
  output; typed/provenance/dimension failures; cancellation; input limits;
  generic execution through modulation to explicit emit; and retained native
  invocation count zero.
- `cargo test --locked` — 229 library tests and 48 binary/UI tests passed.
- `cargo check --locked --release` — passed.
- `cargo clippy --locked --all-targets -- -D warnings` — passed.
- `cargo fmt --check` and `git diff --check` — passed.
- `timeout 8s cargo run --locked` — built and launched `target/debug/toniator`
  and stayed up for the expected timeout; no startup failure was reported.

No screenshot, preview, PNG, or SVG artifact was generated: this substage does
not alter live rendering or export dispatch. The timeout smoke is not human
GNOME/Wayland acceptance.

## Limitations, review targets, documentation, and invalidation

- Native Curves remains incomplete: `curves.emit-paths`, canonical output,
  dispatch, and preview/PNG/SVG parity are later, separately authorized work.
- Review the command/outlines ceilings when final emission establishes its
  memory contract, and retain the shared helper seam rather than introducing a
  second geometry implementation.
- Durable documentation may need reconciliation after native Curves execution
  reaches live dispatch; this implementation evidence is not durable product
  documentation.
- Invalidate this record if retained interpolation/threshold/outline behavior,
  Curves parameter bounds/ports, typed runtime values, resource policy,
  artboard provenance rules, emission, or live dispatch changes; also if HEAD
  or the dirty-worktree assumptions below change.

## Working-tree assumptions

The repository was materially dirty on the HEAD above from accepted TON-010
work and unrelated user changes. Those edits were preserved. No reset, clean,
commit, push, publication, deployment, or destructive operation was performed.
