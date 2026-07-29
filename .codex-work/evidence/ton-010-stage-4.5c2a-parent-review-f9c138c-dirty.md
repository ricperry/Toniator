# TON-010 Stage 4.5C2A — parent review

- Recorded: 2026-07-28
- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `f9c138c493a9d687b5300abddf14e78281f2ad63`
- Scope: save/reopen and undo/redo authority coverage only.

## Accepted C2A deliverable

`src/persistence.rs` now tests both production C1 fixtures through the real
current paths: `parse_treatment`, `ParsedTreatment::candidate_for`,
`DocumentEditor::replace_with_preset_candidate`, authoritative
`set_shape_settings`/`set_curve_settings`, `save_document_atomic`, and
`load_document`.

For `Polygon Six`, the test edits polygon sides to 3 and rotation to 27°.
For `Motif Ladder`, it edits curve scale to 52, tile count to 6, and stack
count to 4. Each saved document contains `pattern_state` and no serialized
`render`; reopen preserves the selected pattern and exact typed values. Undo
restores the fixture authority and redo reapplies the edited authority.

## Parent verification

Passed the focused C2A test, C1 authority/schema test, current-project
roundtrip/obsolete-format rejection test, all-targets check, strict Clippy,
formatting, and diff checks. The writer also ran the full suite: 140 library
and 46 binary/UI tests passed.

C2A intentionally does not cover contradictory adapters, CMYK/RGB transitions,
or preview/PNG/SVG parity. Those remain C2B and C3. No artifacts were created.
This substage is complete and paused for user feedback; do not begin C2B
automatically.

