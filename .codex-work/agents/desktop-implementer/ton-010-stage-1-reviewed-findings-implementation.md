# SUPERSEDED — TON-010 Stage 1 reviewed-findings implementation evidence

This evidence describes the prior v6 compatibility-preservation approach. It
was superseded by the project-wide no-backwards-compatibility policy; current
document definitions now use version 7 and reject obsolete definitions.

- Repository absolute path: `/home/ricperry1/projects/Toniator`
- Git HEAD: `f9c138c493a9d687b5300abddf14e78281f2ad63`
- Producing agent: `desktop_implementer`
- Timestamp: 2026-07-28T13:14:46-04:00
- Task: Fix the reviewed TON-010 Stage 1 compatibility-pattern cache-loss and typed legacy-adapter gate findings without changing rendering, persistence ownership, UI, document version, or registry routing.

## Working-tree assumptions

- This is the same dirty checkout reviewed in `ton-010-stage-1-regression-review-f9c138c-dirty.md`.
- Preserved unrelated TON-013 work includes modified `ISSUES.md`, `docs/UI_ARCHITECTURE.md`, `src/lib.rs`, `src/persistence.rs`, and `src/ui.rs`; deleted legacy Builder UI files; and untracked Builder/resource/documentation files.
- `src/model.rs` and untracked `src/pattern.rs` already contained the two prior TON-010 writers' work. This pass extended only those authorized source files.
- Process inspection found no other active desktop writer or worktree lock before editing.

## Exact files changed

- `src/model.rs`
- `src/pattern.rs`
- `.codex-work/agents/desktop-implementer/ton-010-stage-1-reviewed-findings-implementation.md`
- `.codex-work/cache-index.md`

## Implementation decisions and reused abstractions

- `OutputTreatmentCache` now carries the optional non-authoritative `VersionedPatternParameters` envelope. Active snapshots and first-time RGB treatment creation clone it; treatment application restores it before synchronization.
- Shared model helpers retain opaque `values` when the active `PatternId` still matches the legacy `RenderVariant`; they create a registered default only for an absent/mismatched envelope or remove it for Native Basic. Cached current-v6 records are validated before cache canonicalization; absent records in old v6 cache snapshots default during canonicalization.
- `DOCUMENT_VERSION` remains `6`; no persistence-file, render, curve-render, preview, export, UI, resource, or registry-routing changes were made.
- `adapt_legacy_shapes` and `adapt_legacy_curves` are typed registry adapters from existing `MarkSet` and `CurveGeometry` into `CanonicalPatternOutput`. They reuse `PATTERN_REGISTRY`, `PatternMetadata`, `LegacyPatternCompatibility`, and `PatternOutputKind`, rejecting declared legacy-render or output-kind mismatches with `LegacyPatternAdapterError`.

## Tests and runtime checks

- Focused tests passed:
  - `cargo test --lib output_mode_restoration_preserves_compatibility_pattern_values`
  - `cargo test --lib pattern::tests::legacy_adapters`
- `rustfmt --edition 2024 src/model.rs src/pattern.rs` formatted only the two authorized source files.
- `cargo fmt --check` — passed.
- `cargo clippy --lib --tests -- -D warnings` — passed.
- `cargo test --lib` — passed: 129 tests.
- `git diff --check`, `git diff --check -- src/model.rs`, and `git diff --no-index --check /dev/null src/pattern.rs` — passed.
- No GTK launch, screenshots, preview/export artifacts, or graphical runtime check was applicable: this pass deliberately does not alter GTK or render/export behavior. Existing full render/export tests remain the output-preservation evidence.

## Known limitations and follow-up review targets

- The registry remains non-routing: preview/export continue to use existing legacy render paths, and no render algorithms changed.
- Compatibility parameter members remain opaque until a later schema-owning pattern slice. Explicit records are validated at save/load canonicalization boundaries; ordinary in-memory facade construction retains the established delayed-validation contract.
- Follow up when adding registry-based dispatch, per-channel pattern selection, or schema-owned compatibility values. Review durable documentation for the cached compatibility-envelope contract and the public legacy-adapter boundary at the next TON-010 milestone reconciliation.

## Invalidation conditions

- Reinspect this evidence after changes to `src/model.rs`, `src/pattern.rs`, `src/lib.rs`, `src/persistence.rs`, `src/render.rs`, `src/curve_render.rs`, output-treatment transition behavior, document format policy, Git HEAD, or the recorded dirty-worktree assumptions.
