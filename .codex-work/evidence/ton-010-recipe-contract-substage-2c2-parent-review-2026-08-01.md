# TON-010 declarative recipe contract — Substage 2C2 parent review

- Repository: `/home/ricperry1/projects/Toniator`
- Timestamp: 2026-08-01
- Git HEAD: `262c7e857446ded100d4a90fd23d651e52460665`
- Branch: `TON-010-Stage5-Framework-Restart`
- Producing agent: `desktop_implementer`
- Parent review: inspected runtime value/context types, static native registry,
  DAG execution, cancellation/error handling, capability checks, exports,
  tests, and writer evidence; reran focused execution tests and diff checks.

## Accepted findings

- Recipe definitions remain data-only. Executable behavior is limited to
  explicitly registered Rust function pointers paired with static operation
  descriptors; there is no script, plugin, native-code, or dynamic-loader path.
- Runtime values reuse `SiteDistribution`, `DistributionField`,
  `VoronoiDiagram`, and authoritative `CanonicalPatternOutput`; no competing
  geometry model was introduced.
- Execution validates the definition, operation registry, artboard, and strict
  instance before work. Pattern and selected output-channel parameters are
  bound explicitly, including exact `u64` values.
- Stable lexical Kahn traversal makes independent-node order deterministic and
  independent of serialized node/edge order. Runtime inputs/outputs are checked
  against descriptor port types and failures name the node/operation/version.
- Cancellation is checked before execution, before each node, and before final
  output; native operations receive the same token for internal checkpoints.
- Only validated `CanonicalPatternOutput` with the execution artboard can
  succeed. Actual canonical kinds must match both native emitter capability and
  the definition's declared output capabilities.

## Verification

- Writer: full `cargo test --locked` (180 library, 48 binary/UI), formatting,
  locked check, strict all-target Clippy, and diff checks passed.
- Parent: `cargo test --locked recipe_execution` passed 2 focused tests;
  `git diff --check` passed.

## Safe handoff and invalidation

The declarative contract/executor milestone is accepted. Production operation
implementations, bundled `.tnpattern` resources, renderer dispatch, strict
document/preset version bumps, library I/O, and editor UI remain later work.
Invalidate if definition/instance contracts, runtime value types, native
registry, execution ordering, cancellation, canonical algebra/capability
checks, public exports, HEAD, or recorded dirty state changes.
