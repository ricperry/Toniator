# TON-010 bundled recipes — Substage 3E2D implementation evidence

Date: 2026-08-01
Repository: `/home/ricperry1/projects/Toniator`
Git HEAD inspected: `262c7e857446ded100d4a90fd23d651e52460665`
Producing agent: `desktop-implementer`

## Scope completed

Implemented only `curves.emit-paths`, completing the six native bodies without
adding Curves orchestration or production dispatch.

- The operation consumes narrow `CurvesModulatedPaths`, requires its artboard
  to match the execution context, and requires a semantic output channel.
- It is the sole Curves-native owner of `enabled`, six-digit `#rrggbb` color,
  opacity, and `CurveInkLayer` policy. It returns one
  `CanonicalPatternOutput::Paths(PathPatternOutput { CurveGeometry })` layer
  for the selected semantic CMYK or RGB channel.
- Enabled output clones only the already-computed outlines; disabled output
  retains exactly one disabled semantic layer with no outlines, matching the
  established Shapes disabled-layer behavior.
- Emission validates final geometry without regenerating or mutating it:
  at most 10,000 outlines and 4,000,000 commands, with checked command-count
  arithmetic. It checks cancellation before consuming inputs and before return.
- Crosshatch/output-assignment compatibility is not represented here: no
  crosshatch color, pipeline assignment, multi-channel merging, caching, or
  consumer policy was added.

The valid generic bundled Curves recipe now executes all six registered native
bodies and returns canonical Paths. Production Curves rendering still uses the
retained pipeline and does not invoke this registry.

## Exact files changed for this substage

- `src/curves_native.rs`
- `.codex-work/agents/desktop-implementer/ton-010-bundled-recipes-substage-3e2d.md`

The dirty/untracked native Curves module also contains parent-accepted 3E2A–C
work, which was preserved unchanged except where this final operation consumed
its narrow output.

## Important decisions and reused abstractions

- Reused `CurveGeometry`, `CurveInkLayer`, `InkLayer`, `PathPatternOutput`,
  `CanonicalPatternOutput`, the semantic `OutputChannelId` mapping, and the
  maintained `parse_hex_color` parser. No new geometry or color representation
  was introduced.
- The selected channel is converted only through the existing semantic
  `OutputChannelId::to_legacy_ink` adapter, preserving exact CMYK/RGB identity
  in the existing path-layer representation.
- Final-limit validation is isolated from the modulation helper: it verifies
  the already-final outline list rather than duplicating interpolation,
  simplification, clipping, or outline construction logic.

## Verification and artifacts

- `cargo test --locked curves_native --lib` — 18 passed. Covers exact
  CMYK/RGB canonical single-layer structure, enabled/disabled and empty/nonempty
  outlines, color/opacity, retained single-channel path-fixture equality,
  wrong type/artboard/channel/color/opacity/cancellation/outline-and-command
  limit failures, all-six generic execution, preflight-before-invocation, and
  retained native-count zero.
- `cargo test --locked` — 232 library tests and 48 binary/UI tests passed.
- `cargo check --locked --release` — passed.
- `cargo clippy --locked --all-targets -- -D warnings` — passed.
- `cargo fmt --check` and `git diff --check` — passed.
- `timeout 8s cargo run --locked` — built and launched `target/debug/toniator`
  and remained up until the expected timeout, with no startup failure.

No screenshot, preview/PNG/SVG export, or production artifact was generated:
production Curves dispatch and consumer parity are deliberately out of scope.
The startup smoke is not human GNOME/Wayland acceptance.

## Limitations, review targets, documentation, and invalidation

- Native Curves now has all six atomic bodies, but public full-document
  orchestration, multi-channel merge/cache behavior, canonical-consumer parity,
  live dispatch, UI/preset/schema work, and Stage 6 are not implemented here.
- Review final outline/command ceilings together with later orchestration and
  consumer memory contracts; retain the narrow typed boundaries and shared
  retained geometry seams.
- Durable documentation may need reconciliation only when native Curves gains
  public orchestration or dispatch; this evidence is not durable documentation.
- Invalidate if Curves runtime types, canonical path/layer representation,
  semantic channel mapping, output parameter bounds, final limits, retained
  geometry helpers, generic executor behavior, live dispatch, HEAD, or the
  dirty-worktree assumptions change.

## Working-tree assumptions

The repository was materially dirty on the HEAD above from accepted TON-010
work and unrelated user edits. Those changes were preserved. No reset, clean,
commit, push, publication, deployment, or destructive operation was performed.
