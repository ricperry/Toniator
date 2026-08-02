# TON-010 bundled recipes — Substage 3E3B implementation evidence

Date: 2026-08-01
Repository: `/home/ricperry1/projects/Toniator`
Git HEAD inspected: `262c7e857446ded100d4a90fd23d651e52460665`
Producing agent: `desktop-implementer`

## Scope completed

Switched the live Curves `RenderVariant::WebCurveV1` production branch to the
accepted bundled Curves orchestrator.

- `generate_document_pattern_output_cancellable` now calls
  `execute_bundled_curves_recipe_cancellable` with the decoded/prepared source,
  authoritative `canonical.pattern_state.curve_settings()`, existing artwork
  pipeline, and the existing cancellation token.
- It no longer reads the transient `RenderVariant` settings or adapts retained
  whole geometry. Source decoding, prepared generation (`0` for the live
  document path), pipeline assignment, cancellation checkpoints, canonical
  output validation, preview, PNG, and SVG consumer flow remain unchanged.
- Crosshatch remains an external pipeline assignment policy within the accepted
  orchestrator; CMYK/RGB channel order, disabled-channel behavior, output
  colors, opacity, and editable Curve SVG output retain retained-oracle parity.
- Retained whole-Curves generator/oracle entrypoints are now `cfg(test)` only.
  Test-only instrumentation counts them, while the existing Curves recipe
  instrumentation records live recipe entry. There is no remaining production
  call to the retained whole generator.

## Exact files changed

- `src/render.rs`
  - live Curves dispatch
  - test-only retained-facade/cache helpers
  - CMYK/RGB live-dispatch, authoritative-state, and cancellation guards
- `src/curve_render.rs`
  - retained whole-generator restricted to test builds
  - test-only invocation instrumentation
- `src/svg_export.rs`
  - consumer parity test now routes CMYK/RGB/Crosshatch through live dispatch
    before comparing preview, PNG, and editable SVG output
- `.codex-work/agents/desktop-implementer/ton-010-bundled-recipes-substage-3e3b.md`

The existing narrow Curve SVG bytes seam from 3E3A3 was reused. No changes were
made to `site_distribution.rs`, `voronoi_geometry.rs`, schema, persistence,
presets, UI, pattern library, or Stage 6.

## Verified tests and behavior

- Live CMYK and RGB documents each enter Curves recipe orchestration exactly
  once and invoke the retained whole generator zero times.
- A contradictory transient `RenderVariant::WebCurveV1` settings facade cannot
  change the authoritative pattern-state result; live dispatch remains recipe
  only.
- A pre-cancelled live Curves render returns before either recipe or retained
  whole-generator work begins. Existing orchestrator tests continue to prove
  prepared-generation/cache and before/between-channel cancellation semantics.
- The representative consumer matrix uses live dispatched output for modern
  CMYK (with a disabled Yellow layer), modern RGB, and Crosshatch. Each exactly
  equals the pre-dispatch retained oracle, then proves transparent/white and
  filtered preview pixels, deterministic canonical PNG bytes, and editable
  Curve SVG bytes/file output. The Crosshatch labels/color and RGB/CMYK
  semantic layers remain correct.
- The live Curve SVG file-export check now requires one recipe orchestration
  call and exact bytes from the retained-equivalent Curve SVG seam.

## Commands and artifacts

- Focused live dispatch, authoritative-state, cancellation, and consumer tests
  passed.
- `cargo test --locked` — 243 library tests and 48 binary/UI tests passed.
- `cargo check --locked --release` — passed.
- `cargo clippy --locked --all-targets -- -D warnings` — passed.
- `cargo fmt --check` and `git diff --check` — passed.
- `timeout 12s cargo run --locked` — built and launched `target/debug/toniator`
  without a startup failure before the final test-only cancellation assertion.

No screenshot, PNG/SVG fixture, or manual GNOME/Wayland acceptance was
generated. The runtime smoke is not manual graphical acceptance.

## Decisions, limitations, and invalidation

- `Document.pattern_state` is the semantic authority; transient renderer
  fields are derived compatibility projections and cannot influence dispatch.
- The retained generator remains only as a test oracle and atomic-helper owner;
  do not remove its seams until a separately authorized cleanup stage.
- Generic canonical SVG remains algebra-only for Paths; editable Curves use the
  established Curve SVG bytes/file seam. This is documented consumer ownership,
  not a dispatch regression.
- Follow-up review targets: live cache installation/generation ownership,
  actual preview/PNG/SVG artifacts for a later acceptance gate, retained oracle
  cleanup, and any future adapter/schema/pattern-library stage.
- Durable documentation likely affected: TON-010 Stage 5 architecture and
  recipe-contract material; milestone documentation reconciliation is separate.
- Invalidate this evidence if Curves adapter/orchestrator behavior, pattern
  authority, output pipeline semantics, canonical consumers, retained test
  oracle, cancellation/generation flow, HEAD, or dirty-worktree assumptions
  change.

## Working-tree assumptions

The repository was materially dirty on the HEAD above from accepted TON-010
work and unrelated user edits. Those edits were preserved and not staged. No
reset, clean, commit, push, publication, deployment, or destructive operation
was performed.
