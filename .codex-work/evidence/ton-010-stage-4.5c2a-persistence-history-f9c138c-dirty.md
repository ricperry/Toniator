# TON-010 Stage 4.5C2A — fixture persistence and history evidence

Date: 2026-07-28

Baseline: `f9c138c493a9d687b5300abddf14e78281f2ad63`, intentionally dirty
worktree. C1's `Polygon Six.tntr` and `Motif Ladder.tntr` are reused as the
production preset inputs. This record is limited to current-document save/
reopen and undo/redo of authoritative parameter edits.

Verified path for each fixture:

1. `preset::parse_treatment` parses the compiled production bytes and builds a
   candidate against a real `Document`.
2. `DocumentEditor::replace_with_preset_candidate` commits the selected
   authoritative pattern state.
3. A representative edit is made through `set_shape_settings` (polygon sides
   3, rotation 27°) or `set_curve_settings` (curve scale 52, 6 tiles, 4
   stacks).
4. `persistence::save_document_atomic` writes the current project and
   `persistence::load_document` reopens it.
5. The reopened `pattern_state` exactly equals the edited authoritative state;
   typed reads retain the selected fixture's values. Serialized project JSON
   contains `pattern_state` and no `render` field.
6. Undo restores the post-fixture/pre-edit authority; redo restores the edited
   authority.

The test deliberately does not install a contradictory transient adapter.
Current document persistence proves it cannot serialize an adapter as
authority; the contradiction and CMYK/RGB transition matrix remain C2B.

Focused and full validation passed:

* `cargo test --locked persistence::tests::c2a_c1_fixtures_save_reopen_and_undo_redo_authoritative_pattern_edits -- --exact`
* `cargo test --locked preset::tests::c1_matrix_presets_keep_selection_and_typed_parameters_in_authoritative_state -- --exact`
* `cargo test --locked persistence::tests::current_project_roundtrips_and_rejects_pre_release_versions -- --exact`
* `cargo fmt --all && git diff --check`
* `cargo test --locked` — 140 library tests + 46 binary/UI tests passed.
* `cargo check --locked --all-targets`
* `cargo clippy --locked --all-targets -- -D warnings`

Artifacts: none. C2A is persistence/history evidence; preview/PNG/SVG output
artifacts remain exclusively C3 work.

CACHE_UPDATE: C2A's current-document round trip persists only
`Document.pattern_state`, then reconstructs execution state at load. The C1
Shapes and Curves fixtures retain their selection and edited typed parameters
across save/reopen; one undo/redo reverses/reapplies each authoritative edit.
Re-run the named persistence test when project schema, preset application,
`DocumentEditor` history, or pattern-state serialization changes. This entry
is invalidated by changes to those paths or either fixture's typed contract.
