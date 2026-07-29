# SUPERSEDED — TON-010 Stage 1 compatibility-pattern persistence evidence

The original v6/defaulted migration approach in this evidence was superseded
by the project policy that obsolete document and preset definitions are
rejected rather than preserved in code. The current implementation uses
document version 7 and requires the current compatibility record.

- Repository absolute path: `/home/ricperry1/projects/Toniator`
- Git HEAD: `f9c138c493a9d687b5300abddf14e78281f2ad63`
- Producing agent: `desktop_implementer`
- Timestamp: 2026-07-28T13:07:46-04:00
- Task: Close the bounded TON-010 Stage 1 persisted compatibility-pattern limitation without changing render authority, UI, document version, or output formulas.

## Working-tree assumptions

- The starting checkout matched the Stage 1 registry evidence at the same HEAD.
- Preserved pre-existing TON-013 work includes `ISSUES.md`, `docs/UI_ARCHITECTURE.md`, `src/ui.rs`, deleted legacy Builder UI files, and untracked Builder/resource/documentation additions.
- The prior writer's `src/pattern.rs` and `src/lib.rs` changes were already present and were not edited.
- No active writer marker was found under `.codex-work/` during inspection.

## Exact files changed

- `src/model.rs`
- `src/persistence.rs`
- `.codex-work/agents/desktop-implementer/ton-010-stage-1-compatibility-pattern-persistence.md`

## Implementation decisions and reused abstractions

- `Document.compatibility_pattern` is an optional serde-defaulted `VersionedPatternParameters` record. It is a non-authoritative compatibility projection of the active `RenderVariant`, not a render input.
- New documents initialize `compat.shapes.v1`; canonical legacy projection derives Shapes or Curves records, and Native Basic carries none.
- The record retains its opaque `values` map when its pattern ID remains current; a render-kind transition replaces only the ID/version envelope with the registered built-in default.
- `Document::apply_treatment`, Crosshatch normalization, and undo/redo state keep the active projection synchronized across output-treatment transitions rather than adding duplicate records to `OutputTreatmentCache`.
- Load accepts a missing record from old current-v6 documents, but validates any explicit record before canonicalization. Save canonicalizes first and then strictly validates the resulting record. This preserves existing direct public `render`-facade construction until it reaches a canonical persistence boundary.
- Reused `PatternId`, `VersionedPatternParameters`, and `PATTERN_REGISTRY` from `src/pattern.rs`; reused the existing `sync_legacy_projection` and `canonicalize_pipeline_facades` save/load conventions. `DOCUMENT_VERSION` remains `6`.

## Tests and checks

- Focused tests passed:
  - `cargo test --lib new_document_initializes_the_shapes_compatibility_pattern`
  - `cargo test --lib old_v6_documents_infer_compatibility_patterns_from_shapes_and_curves`
  - `cargo test --lib current_v6_rejects_mismatched_or_unsupported_compatibility_patterns`
- `cargo fmt --check` — passed.
- `cargo clippy --lib --tests -- -D warnings` — passed.
- `cargo test --lib` — passed: 126 tests.
- `git diff --check` and `git diff --check -- src/model.rs src/persistence.rs` — passed.
- No GTK launch, screenshot, or graphics-export artifact was needed: this is persistence/model-only and render formulas were intentionally unchanged.

## Known limitations and follow-up review targets

- No UI selection, per-channel assignment, registry-based render dispatch, or new pattern generation was added; those remain later TON-010 stages.
- General in-memory `Document::validate` intentionally permits a stale compatibility projection while callers are constructing a public legacy facade directly. Persistence boundaries canonicalize it, and load rejects any explicit mismatched/unsupported serialized record before canonicalization.
- Review later pattern-routing work for whether compatibility `values` gain schema-owned meaning, and then replace opaque-map handling with per-pattern validation.
- Durable documentation likely affected at milestone review: the document compatibility-projection contract and stable persisted pattern ID catalog.

## Invalidation conditions

- Reinspect this evidence if `src/pattern.rs`, `src/lib.rs`, `src/model.rs`, `src/persistence.rs`, output-transition behavior, document persistence policy, Git HEAD, or the recorded dirty-worktree assumptions change.
