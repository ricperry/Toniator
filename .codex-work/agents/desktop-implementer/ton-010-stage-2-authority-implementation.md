# TON-010 Stage 2 authoritative pattern-state implementation

- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `f9c138c`
- Checkout: dirty; pre-existing TON-013 Blueprint and TON-010 Stage 1 work was preserved.
- Date: 2026-07-28

## Delivered contract

`Document.pattern_state` is the sole persisted pattern selector and parameter
authority. It contains the selected stable pattern ID and validated,
versioned typed parameter instances for the current Shapes and Curves adapters.
The current document format is v8 and the current preset format is v5. Bundled
presets and fixtures were rewritten to those definitions. Load/parse paths
reject obsolete versions, missing or mismatched pattern instances, unsupported
schema versions, unknown nested fields, and unsupported IDs; no obsolete-file
migration path was added.

`Document.render` and `OutputTreatmentCache.render` are serde-skipped derived
execution adapters. `sync_legacy_projection` and renderer/export canonical
boundaries rebuild them from `pattern_state`, then apply only the semantic
artwork-pipeline projection. `value_mode` and `single_channel` are transient
pipeline projections. Test-only adapter helpers require the caller to select
the authoritative pattern first and never infer or mutate selection.

## Adapter inventory

- `Document.render` and `OutputTreatmentCache.render`: retained until Stage 3
  canonical output algebra and the Shapes/Curves execution branches are
  replaced. They are not persisted, undo authority, or selectors.
- `saved_web_shape`, `saved_web_curve`, and their pipeline snapshots: retained
  only for current Crosshatch exit, output-model transitions, and atomic
  undo/redo restoration. They are transient and skipped from serialization;
  remove after Stage 3/4 canonical rendering and inactive-state UI flow.
- Preset channel extraction and SVG/PNG/Curve renderer entry points: retained
  as narrow legacy projections until Stage 3 canonical outputs are consumed;
  they read derived adapters and never write pattern selection.
- Registry adapter metadata and test fixtures: retained as the explicit
  Shapes/Curves compatibility contract through Stage 3/4; remove only after
  all consumers use canonical outputs.

## Verification

- `cargo test --locked --lib`: 130 passed; `cargo test --locked --bin toniator`:
  46 passed; the complete `cargo test --locked` matrix passed.
- `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`,
  `cargo check --locked --all-targets`, and `git diff --check` passed.
- Coverage includes save/reopen, current bundled presets, unknown/obsolete
  document and preset rejection, nested-field rejection, undo/redo selection
  and parameters, CMYK/RGB transitions, Shapes/Curves transitions, Crosshatch
  restoration, and contradictory transient adapter state.
- Preview, raster output, PNG, and SVG paths canonicalize from authoritative
  pattern state before rendering. The Stage 2 authority gate is complete.

Stage 3 has not started. Weighted Voronoi remains the required Stage 5
deliverable; canonical cells, boundaries/networks, negative-space polarity,
and the full custom-pattern ecosystem remain later scope as recorded in
`ISSUES.md`.
