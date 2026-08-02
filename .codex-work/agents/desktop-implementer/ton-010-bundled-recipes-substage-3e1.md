# TON-010 bundled recipes — Substage 3E1 implementation evidence

Date: 2026-08-01
Git HEAD inspected: `262c7e857446ded100d4a90fd23d651e52460665`

## Scope completed

Implemented the bounded Curves recipe-contract substage only:

- Added immutable bundled `compat.curves.v1.tnpattern` bytes and strict bundled-registry loading.
- Declared Curves-only typed recipe ports and six metadata operations:
  placement, source sampling, motif selection, deformation, width modulation,
  and paths emission.
- Assigned each current Curves setting to its semantic operation owner and
  provided complete authoring sections: Placement, Motif, Deformation,
  Modulation, and Output.
- Added a one-way adapter from `Document.pattern_state.curve_settings()` and
  direct typed-settings adapter. Editable cubic paths are represented as
  deterministic embedded SVG digest assets.

No Curves native operation bodies, native operation registry entries, runtime
recipe execution/orchestration, live render dispatch switch, document schema
change, preset change, or UI change was made.

## Exact files changed for this substage

- `assets/patterns/compat-curves.v1.tnpattern`
- `src/bundled_pattern_definitions.rs`
- `src/curves_recipe.rs`
- `src/pattern_definition.rs`
- `src/lib.rs`

The shared worktree already contained accepted preceding 3A–3D3B work in some
of these additive files; this substage only added the Curves contract pieces.

## Important decisions and reused abstractions

- Reused the existing strict `.tnpattern` parser, immutable bundled-definition
  registry, typed edge validator, `PatternInstanceParameters`, and
  `EmbeddedSvgAsset` digest validation.
- Reused `WebCurveSettings`, `WebCurveChannel`, `CurvePath`, and
  `OutputChannelId`; the adapter sends every render-relevant Curves field,
  including both placement offsets and all active motif/stack/alternate-
  transform controls.
- Kept `value_mode`, single/crosshatch assignments, and pipeline state out
  of the recipe because those are external semantic authority. `base_channel`
  remains inspector-only.
- The bundled default asset is the exact default soft-wave cubic and its
  SHA-256 digest is verified by the existing strict asset rule.
- Dedicated `Curve*` port types prevent accidental Shapes/Weighted Voronoi
  graph wiring. The bundle declares descriptors only; no corresponding native
  executor is registered.

## Parent-review correction: model-semantic bounds

The initial contract used several accidental bounds that did not match the
current `Document::validate` Curves branch. The correction audited every
Curves setting with an explicit model bound and aligned the declaration and
adapter:

- `max-size` is now `0..=10_000`; `curve-scale` is `0.1..=500`; and
  `motif-bleed` is `0..=100`.
- Tile and stack counts are `1..=10_000`; active stack spacing is
  `-10_000..=10_000`. `tile_spacing` is not an active renderer input and is
  therefore not represented by the recipe.
- The existing global artboard, grid-density, mark-width, scale, threshold,
  opacity, and choice bounds were already equal to model validation. The
  adapter now also enforces the model's `max_mark >= min_mark` relationship.
- Placement rotation/pivots/offsets and tile/stack angle/offset fields are
  model-bounded only by finiteness. Their recipe declarations now use the full
  finite `f64` domain, not earlier arbitrary recipe caps. The generic numeric
  step check preserves its normal behavior but treats only an overflowing
  quotient as continuous, so those truthful extrema remain usable while all
  finite bounds are still checked.
- Model validation requires resolution scale and output quality to be
  `(0, 100]`, while the v1 recipe constraint grammar only has inclusive
  intervals. The definition truthfully exposes the widest bounded interval
  `0..=100`; the Curves adapter's semantic seam rejects zero/non-finite values
  before producing an instance. It therefore admits every currently valid
  positive `f64`, including subnormals, without treating zero as valid Curves
  state.
- The adapter now requires each emitted channel color to parse as `#rrggbb`
  rather than relying on the general text parameter's length limit.
- Editable path assets now require exactly `1..=64` finite cubic segments,
  matching `validate_curve_path`.

No intentional resource-limit tightening remains in this contract. The only
format-level approximation is the inclusive zero lower bound noted above, and
the adapter closes it at the Curves semantic boundary.

## Parent-review correction: retained-render dependency ownership

The retained `curve_render.rs` call sites were re-audited before any future
execution work. The declarative graph now mirrors those consumers:

- Placement owns artboard/grid placement and per-channel resolution, rotation,
  pivots, and offsets.
- Motif selection owns only shared-versus-channel path selection and closure/
  smoothing semantics.
- Deformation owns layout selection plus active motif normalization, coverage,
  counts, transforms, offsets, stack spacing, and the sampling aspect of
  `output-quality`.
- Width modulation owns mark response values and also `output-quality`, which
  controls post-width `simplify_segment` tolerance. The same parameter is
  intentionally bound to both deformation and modulation nodes.
- Emission owns enabled, color, and opacity only.

`WebCurveChannel::tile_spacing` is deliberately ignored by retained Curves
rendering, and `WebCurveSettings::show_background` has no retained render or
export consumer. Both remain untouched compatibility/document state, but are
explicitly classified as legacy no-op fields pending a later v9 cleanup; they
are absent from the recipe parameter list, operation descriptors/nodes,
authoring sections, and adapted instance.

## Parent-review correction: automatic coverage footprint ownership

The retained `motif_counts` path computes its automatic coverage guard from
`max_curve_width`. That helper consumes global `min-mark` and `max-mark` plus
per-channel `max-size` and `scale`, before width modulation emits the final
variable-width points. All four parameters therefore bind to both
`curves.deformation` and `curves.width-modulation`:

- Deformation consumes them only to establish a sufficient automatic motif
  coverage footprint, avoiding artboard-edge clipping when the eventual curve
  can be wide.
- Width modulation retains ownership of the actual source-response width values.

The creator-facing authoring layout continues to list each parameter once,
under **Modulation**. This is a graph dependency correction, not a UI or
semantic-authority move.

## Verification

- `cargo fmt --check` — passed.
- `cargo test --locked bundled_pattern_definitions --lib` — 4 passed.
- `cargo test --locked curves_recipe --lib` — 5 passed, including model-bound
  declarations/accepted endpoints, `f64` coordinate extrema, oversized-path,
  non-hex-color, exclusive-positive, parameter ownership, dual
  `output-quality`, dual deformation/modulation automatic-coverage inputs,
  full-consumption, and legacy-no-op regressions.
- `cargo test --locked bundled_pattern_definitions --lib` — 4 passed after the
  automatic-coverage ownership correction.
- `cargo test --locked` — 220 library tests and 48 UI/CLI tests passed after
  the corrections.
- `cargo check --locked --release` — passed.
- `cargo clippy --locked --all-targets -- -D warnings` — passed.
- `git diff --check` — passed.
- `timeout 8s cargo run --locked` — compiled and launched
  `target/debug/toniator`; termination was the intentional timeout.
- Static retained-boundary scan found Curves operation IDs only in the
  definition/contract locations, with no Curves native-operation registry or
  runtime executor.

Focused tests prove that the bundle is registry-backed, has the expected
Curves-only port types, rejects a cross-family edge type, has complete
authoring metadata, validates all defaults, maps each active recipe field to
its retained-render consumer, preserves current model boundaries, rejects
malformed paths/colors, excludes the two legacy no-ops, and reads persisted
pattern state rather than a contradictory transient `RenderVariant`.

## Artifacts

No screenshots, previews, PNGs, or SVG exports were created: this is a
contract-only substage with no UI, export, or live Curves dispatch change.

## Known limitations and follow-up review targets

- Live Curves rendering intentionally remains the retained
  `curve_render` compatibility path. No proof of native Curves recipe
  execution exists yet because 3E1 deliberately introduces no native bodies.
- `tile_spacing` and `show_background` remain persisted compatibility state
  until a separately authorized v9 cleanup; this contract makes no migration
  or schema decision for them.
- Subsequent Curves execution work must preserve this bundle's parameter
  ownership, use only the declared typed ports, and establish retained/native
  parity before dispatch changes.
- Parent review should verify that future cache/evidence records continue to
  distinguish the declarative contract from any later executable registry.

## Documentation and worktree assumptions

Durable documentation is not changed by this implementation evidence. The
existing Stage 5 architecture/framework documentation may need a later
milestone reconciliation to describe Curves contract availability, but not a
claim of executable Curves recipes.

Assumed the shared dirty worktree at the HEAD above was intentional and
preserved all unrelated modified/untracked files. No reset, clean, commit,
push, or destructive operation was performed.

## Invalidation conditions

Re-review this evidence if any of these change: `WebCurveSettings` or
`WebCurveChannel` fields/defaults/validation; authoritative
`PatternDocumentState::curve_settings()` semantics; operation-port
validation; embedded SVG digest rules; the Curves retained renderer; or a
later substage adds Curves native execution/dispatch.
