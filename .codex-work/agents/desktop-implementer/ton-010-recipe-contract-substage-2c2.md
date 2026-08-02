# TON-010 Recipe Contract — Substage 2C2 implementation evidence

- Timestamp: 2026-08-01T10:57:39-04:00
- Implementer scope: safe declarative recipe execution boundary only. No built-in Shapes/Curves/Weighted implementation, bundled definition/resource loading, renderer dispatch, document/preset schema, library I/O, UI/Blueprint, or algorithm migration.
- Git HEAD assumption: `262c7e857446ded100d4a90fd23d651e52460665` on `TON-010-Stage5-Framework-Restart` before and after this work.
- Working-tree assumption: accepted/user changes already present in `Cargo.toml`, `Cargo.lock`, `src/lib.rs`, `src/model.rs`, `src/pattern.rs`, `src/persistence.rs`, `src/png_export.rs`, `src/preset.rs`, `src/svg_export.rs`, `src/ui.rs`, `ISSUES.md`, `.codex-work/cache-index.md`, and untracked 2A/2B/2C1/manual/assets/prompt evidence were preserved. `src/pattern_definition.rs` remains the untracked 2A/2C1 implementation file extended in place.

## Changed files

- `src/pattern_definition.rs`
  - Added a typed `RecipeRuntimeValue` algebra that directly reuses neutral `SiteDistribution`, `DistributionField`, `VoronoiDiagram`, and authoritative `CanonicalPatternOutput`; no parallel geometry model was introduced.
  - Added `RecipeExecutionContext` with artboard, optional selected output channel, optional source field, and cooperative `CancellationToken`.
  - Added the bounded function-pointer-only `NativeRecipeOperationRegistry` and explicit native implementation registration/error interfaces. Definitions remain data-only and have no script/plugin/dynamic-loader path.
  - Implemented `PatternDefinition::execute_recipe`: descriptor/instance validation before start, deterministic lexical Kahn topological order, literal/scoped binding, runtime port/value checks, cancellation checkpoints, contextual operation errors, canonical output validation, artboard consistency, and declared-output capability enforcement.
  - Added internal descriptor `canonical_output_kinds`; the existing generic canonical emitter is explicitly Regions-capable so v1 declared output metadata is now validated rather than unused. No format field changed.
  - Added two bounded native-operation test suites covering ordering, literal and scoped `u64::MAX` binding, selected-channel absence, missing implementation/version, native error context, runtime output type mismatch, canonical capability enforcement, and cancellation before/between nodes.
- `src/lib.rs`
  - Re-exported the public execution/registry/context/runtime/error types.
- `.codex-work/agents/desktop-implementer/ton-010-recipe-contract-substage-2c2.md`
  - This evidence entry.

## Decisions and reused abstractions

- Reused `CancellationToken`, `ArtboardSpace`, `CanonicalPatternOutput`, `OutputChannelId`, `DistributionField`, `SiteDistribution`, and `VoronoiDiagram` rather than duplicating site-distribution, Voronoi, or geometry authority.
- The executor accepts only explicitly registered native function pointers paired by operation ID/version with static descriptors. No untrusted recipe data controls executable behavior.
- The public registry can borrow a bounded caller-owned implementation slice for testability, but it exposes no dynamic registration or loading mechanism.
- Stable Kahn traversal uses node IDs, so execution order is independent of JSON node/edge order.
- Instance validation with a supplied descriptor registry supports custom/native-test descriptors while preserving the existing default-registry API for current v1 parsing/serialization.

## Verification

- `cargo fmt --check` — passed.
- `cargo test --locked pattern_definition --lib` — passed (12 matching tests).
- `cargo test --locked` — passed (180 library tests, 48 application tests, 0 doc tests).
- `cargo check --locked` — passed.
- `cargo clippy --locked --all-targets -- -D warnings` — passed.
- `git diff --check` — passed.
- Runtime/GTK screenshot: not run; no UI/application wiring was changed.
- Exports/artifacts: none produced; no renderer/export path changed.

## Limitations, review targets, and invalidation

- `REGISTERED_OPERATIONS` has no production native implementations yet. The new executor is intentionally exercised only by bounded test-native operations until a later approved substage wires production behavior.
- Intermediate runtime values are typed neutral pipeline values; actual Shapes/Curves/Weighted operation bodies, bundled definitions/resources, renderer integration, and persistence remain out of scope.
- Parent review should inspect the function-pointer registry lifetime/registration posture, lexical topological-order decision, and exact canonical-output capability matching before authorizing production operation or resource work.
- Durable documentation is likely affected only when recipes become user-visible/loadable; no docs or tracker files were changed here.
- Invalidate this evidence if `src/pattern_definition.rs` or `src/lib.rs` changes; if HEAD or dirty-worktree assumptions change; or if later work adds production operations, definition resources, persistence, rendering/UI dispatch, or TON-011 source-selection behavior.
