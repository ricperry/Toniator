# TON-010 Stage 4.5C2A implementation evidence

Date: 2026-07-28  
Repository: `/home/ricperry1/projects/Toniator`  
Git HEAD: `f9c138c493a9d687b5300abddf14e78281f2ad63`  
Producing agent: desktop-implementer

## Working-tree assumptions

The worktree was intentionally dirty before C2A, including accepted TON-010
Stage 4 work, user-accepted 4.5B, C1 presets/evidence, TON-013 migration
changes, and unrelated documentation/assets. Those changes were preserved. No
reset, clean, commit, push, or deletion was performed.

## Scope and files changed

Completed only Stage 4.5C2A save/reopen and undo/redo coverage for the C1
fixtures.

* `src/persistence.rs` — adds
  `c2a_c1_fixtures_save_reopen_and_undo_redo_authoritative_pattern_edits`.
* `.codex-work/evidence/ton-010-stage-4.5c2a-persistence-history-f9c138c-dirty.md`
  — reusable C2A cache evidence.
* This implementation evidence file.

No C1 fixture, production persistence behavior, UI, shape/curve algorithm,
schema, adapter, output-model transition, preview, PNG, or SVG code changed.

## Verified findings

The test uses the production `include_bytes!` C1 fixtures and production
`parse_treatment`, `ParsedTreatment::candidate_for`,
`DocumentEditor::replace_with_preset_candidate`, `set_shape_settings` /
`set_curve_settings`, `save_document_atomic`, and `load_document` paths.

For both fixtures it verifies:

* selected pattern identity and typed parameters are read from
  `Document.pattern_state` after application;
* a representative Shapes or Curves edit is committed through the existing
  editor mutation API;
* saved current-document JSON has `pattern_state` and no serialized `render`;
* reopening reconstructs the same edited authority and exposes the exact typed
  values; and
* undo restores the fixture authority while redo restores the edited authority.

This is evidence that current persistence does not make `RenderVariant` a
durable authority. It intentionally is not a deliberately contradictory
adapter test.

## Commands and results

Passed:

* `cargo test --locked persistence::tests::c2a_c1_fixtures_save_reopen_and_undo_redo_authoritative_pattern_edits -- --exact`
* `cargo test --locked preset::tests::c1_matrix_presets_keep_selection_and_typed_parameters_in_authoritative_state -- --exact`
* `cargo test --locked persistence::tests::current_project_roundtrips_and_rejects_pre_release_versions -- --exact`
* `cargo fmt --all && git diff --check`
* `cargo test --locked` — 140 library tests and 46 binary/UI tests passed.
* `cargo check --locked --all-targets`
* `cargo clippy --locked --all-targets -- -D warnings`

Artifacts: none. No image or export artifact was produced because C3 owns
preview/PNG/SVG parity evidence.

## Unresolved follow-up boundaries

* C2B: deliberately contradictory transient-adapter persistence behavior and
  CMYK/RGB transition coverage.
* C3: preview/PNG/SVG parity artifacts and visual inspection.
* 4.5D: integrated readiness and manual acceptance.

Documentation likely affected: the parent/documentation maintainer may later
add C2A status to the Stage 4.5 durable record after review; this evidence is
not a substitute for that reconciliation.

## Invalidation conditions

Re-run this test if the current document schema/version, pattern-state serde
shape, preset candidate application, persistence loader/saver, editor history,
or either C1 fixture's selected typed contract changes.

CACHE_UPDATE: C2A covers both C1 runtime presets through the real current
document save/reopen path plus one authoritative edit and undo/redo. The saved
format contains `pattern_state`, not `render`; adapter-contradiction and
CMYK/RGB testing remain C2B, and output artifacts remain C3.
