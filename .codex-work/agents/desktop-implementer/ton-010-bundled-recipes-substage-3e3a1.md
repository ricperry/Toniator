# TON-010 bundled recipes — Substage 3E3A1 implementation evidence

Date: 2026-08-01
Repository: `/home/ricperry1/projects/Toniator`
Git HEAD inspected: `262c7e857446ded100d4a90fd23d651e52460665`
Producing agent: `desktop-implementer`

## Scope completed

Added the production-callable but non-dispatched
`execute_bundled_curves_recipe_cancellable` orchestration foundation only.

- It accepts `PreparedSource`, authoritative `WebCurveSettings`,
  `ArtworkPipelineSettings`, and cancellation; adapts through the accepted
  Curves definition/instance; executes the shared generic recipe once for each
  enabled semantic output channel; and merges its one-layer canonical Paths
  results into `CurveGeometry` in the exact retained channel order.
- Output-channel selection exactly matches retained Curves: legacy
  compatibility (including Crosshatch) uses CMYK; other assignments use the
  pipeline output model. Disabled channels are skipped before provider/native
  work and are omitted from the merged geometry as retained Curves does.
- Crosshatch remains external assignment compatibility: after each generic
  channel emits its own normal recipe layer, the orchestrator applies the
  existing monochrome `crosshatch_color` policy. No recipe embeds pipeline or
  crosshatch policy.
- `CurvesRecipeSourceProvider` resolves recipe-declared field dimensions from
  `resolve_channel_fields_cancellable` and caches full semantic field sets by
  dimensions, ordered enabled semantic IDs, source generation, and resolved
  generation. It preserves the generic source-provider boundary and never
  selects alternate source/pattern content per channel.
- The orchestrator preserves semantic channel identity, source generation,
  semantic and enabled-layer indices in each execution context, artboard,
  cancellation checkpoints, and actionable failures for unexpected output kind,
  layer count, channel identity, or enabled status.

`src/render.rs` remains intentionally on the retained Curves pipeline. The
added render test verifies recipe orchestration count stays zero during a live
Curves document render.

## Exact files changed for this substage

- `src/curves_native.rs`
- `src/render.rs` (test-only non-dispatch guard)
- `.codex-work/agents/desktop-implementer/ton-010-bundled-recipes-substage-3e3a1.md`

The shared dirty/untracked Curves-native module and broader render changes from
previous accepted work were preserved; no production render dispatch was
altered in this substage.

## Important decisions and reused abstractions

- Reused Shapes' accepted orchestration/provider pattern, the generic
  `RecipeSourceFieldProvider`, `resolve_channel_fields_cancellable`,
  `ResolvedChannelFields`, established `OutputChannelId` ordering, and retained
  `generate_curve_geometry_for_pipeline` solely as the non-production oracle.
- Reused the completed atomic Curves recipe bodies and their narrow typed
  values; orchestration does not duplicate deformation, modulation, or emit
  geometry logic.
- The final merged output is assembled directly from generic canonical Paths;
  it does not hard-code a Curves pattern ID or invoke a legacy adapter beyond
  selecting the accepted bundled-settings adapter.
- Test-only instrumentation records orchestration starts, native node calls,
  provider cache misses, and a deterministic cancellation-after-first-channel
  hook. It has no release behavior.

## Verification and artifacts

- `cargo test --locked bundled_orchestrator --lib` — 3 passed: retained exact
  equality/determinism for CMYK and RGB, both layout paths, shared/per-channel
  motif, disabled channels, Crosshatch color, provider hit/miss dimensions,
  disabled zero work, and cancellation before/between channels.
- `cargo test --locked live_curves_document_render_stays --lib` — passed:
  live document rendering leaves Curves orchestration at zero.
- `cargo test --locked` — 236 library tests and 48 binary/UI tests passed.
- `cargo check --locked --release` — passed.
- `cargo clippy --locked --all-targets -- -D warnings` — passed.
- `cargo fmt --check` and `git diff --check` — passed.
- `timeout 8s cargo run --locked` — built and launched `target/debug/toniator`
  until the expected timeout without a startup failure.

No screenshot, preview/PNG/SVG export artifact, or live consumer parity was
generated because production Curves dispatch remains retained. The startup
smoke is not human GNOME/Wayland acceptance.

## Limitations, review targets, documentation, and invalidation

- This is a non-dispatched foundation only. Public full-document orchestration,
  multi-channel cache installation, preview/PNG/SVG consumer parity, live
  dispatch, retained-code removal, persistence/schema/preset/UI work, and
  Stage 6 remain out of scope.
- The current execution context exposes one prepared generation, so source and
  resolved generations are both that authority; there is no independent stale
  generation input to validate until the context contract supplies one.
- Review provider-key/field-cache lifetime and final merged output resource
  policy with future dispatch/consumer work; keep disabled-work and semantic
  ordering guarantees intact.
- Invalidate this evidence if Curves adapter/recipe/runtime ports, retained
  generation behavior, pipeline channel ordering/assignment, field resolution
  cache keys, context generation contract, cancellation policy, final layer
  assembly, production dispatch, HEAD, or dirty-worktree assumptions change.

## Working-tree assumptions

The repository was materially dirty on the HEAD above from accepted TON-010
work and unrelated user edits. Those changes were preserved. No reset, clean,
commit, push, publication, deployment, or destructive operation was performed.
