# TON-010 Stage 4, Substage 4A — authority/schema read contract

- Timestamp: 2026-07-28T15:58:18-04:00
- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `f9c138c493a9d687b5300abddf14e78281f2ad63`
- Producing agent: `desktop-implementer`
- Scope: bounded Stage 4A only; establish UI-facing authority reads and registry schema lookup before selector or parameter callback refactoring.

## Checkout assumptions

The checkout was intentionally dirty before this work, including completed TON-010 Stages 1–3, TON-013 GTK/Blueprint work, presets, fixtures, evidence, and documentation. In particular, `src/lib.rs`, `src/model.rs`, `src/ui.rs`, persistence/export/render modules, legacy UI resources, and many docs/assets were already modified or untracked. The pre-existing `src/ui.rs` diff remained present (`336` added / `527` removed lines against HEAD); this substage made no direct UI edit. No reset, clean, revert, commit, push, deletion, or agent delegation occurred.

## Files inspected

- `src/model.rs`: `PatternDocumentState`, `Document`, `DocumentEditor`, legacy projection seam, and authority tests.
- `src/pattern.rs`: built-in Shapes/Curves metadata, descriptors, and registry lookup.
- `src/ui.rs`: existing direct `RenderVariant` reads and callbacks were inspected only.
- `docs/TON-010_STAGE_3_CANONICAL_OUTPUT.md`: confirmed that `Document.pattern_state` is selector/parameter authority and `RenderVariant` is an execution adapter.
- `.codex-work/evidence/ton-010-stage-3-canonical-output-f9c138c-dirty.md` and Stage 2 authority/schema handoff evidence.

## Exact files changed

- `src/model.rs`
- `src/pattern.rs`
- `src/lib.rs`
- `.codex-work/agents/desktop-implementer/ton-010-stage-4-substage-4a-authority-schema-api.md`

## Implementation decisions and reused abstractions

- Exposed read-only `PatternDocumentState` authority accessors: `selected_pattern_id`, `selected_metadata`, `selected_parameters`, `shape_settings`, and `curve_settings`. They read the persisted `selected`/`instances` state only and do not inspect `Document.render` or `RenderVariant`.
- Kept all writes on the existing `DocumentEditor::{set_pattern_state, select_pattern, set_shape_settings, set_curve_settings}` path. Internal `PatternDocumentState` write helpers remain crate-private.
- Added `PatternMetadata::parameter_for_control` and `PatternRegistry::parameter_for_control`, binding stable Blueprint control IDs to current descriptor metadata without selector UI work.
- Re-exported the authority state and schema descriptor types from `src/lib.rs` for binary/UI-facing use.
- Corrected the stale `PatternInspectorPanel` documentation to state the current authority policy.

## Verification

- `cargo fmt --all` — passed.
- `cargo fmt --all -- --check` — passed.
- `cargo test --locked --lib authority_read_accessors_ignore_a_contradictory_transient_adapter` — passed (1 test).
- `cargo test --locked --lib registry_exposes_shapes_and_curves_control_descriptors` — passed (1 test).
- `cargo check --locked --all-targets` — passed.
- `git diff --check` — passed.

The new model test deliberately leaves `Document.render` as a Curves adapter while the authoritative state selects Shapes with changed Shapes settings; selection, metadata, parameter record, and typed Shapes settings continue to resolve from `pattern_state`. The registry test verifies current Shapes and Curves selector metadata and control-ID descriptor discovery.

## Artifacts and known limitations

- No screenshot, GTK launch, or export artifact: this substage intentionally changes no UI behavior, rendering behavior, or graphics output.
- No selector UI implementation or parameter callback rewrite was performed; `src/ui.rs` still has its existing adapter reads/callbacks for Substage 4B.
- No new patterns, Weighted Voronoi, custom-pattern ecosystem, obsolete-format migration, or persistence schema changes were added.

## Follow-up review targets and documentation

- Substage 4B should migrate UI reads from `Document.render` to the new `PatternDocumentState` accessors and bind existing control visibility/help through descriptor metadata while preserving GTK deferred synchronization and callback safety.
- Review GTK selection changes for `DropDown` model identity, invalid positions, and bounded `RefCell` borrows before runtime acceptance.
- Durable documentation is likely affected only when the Stage 4 milestone closes; do not treat this evidence as a documentation substitute.

## Invalidation conditions

Reinspect this evidence if `src/model.rs`, `src/pattern.rs`, `src/lib.rs`, the Stage 3 authority contract, registry IDs/descriptors, or the relevant dirty TON-010/TON-013 changes change; if UI work begins; or if the Git HEAD/worktree baseline no longer matches the assumptions above.
