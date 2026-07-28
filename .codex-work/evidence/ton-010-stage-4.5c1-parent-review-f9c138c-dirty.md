# TON-010 Stage 4.5C1 — parent review

- Recorded: 2026-07-28
- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `f9c138c493a9d687b5300abddf14e78281f2ad63`
- Scope: current-format testing-preset matrix and fixture foundation only.

## Accepted C1 deliverables

The production bundled-preset inventory now includes two visibly distinct,
current-format v5 complete-workflow fixtures:

- `assets/presets/Polygon Six.tntr`: authoritative Shapes selection with a
  shared six-sided Regular Polygon, 58-cell grid, mark range 6–76, and rotated
  scaled marks.
- `assets/presets/Motif Ladder.tntr`: authoritative Curves selection with a
  manual flipped motif layout, 34 cells, five tiles, and three stacks.

Both persist selection and typed parameters only under
`Document.pattern_state`; neither contains `treatment.render`. Both use the
existing production `BUNDLED_PRESETS` and preset candidate loader. No parallel
fixture loader or compatibility migration was introduced.

## Parent verification

Passed focused authority/schema, bundled-applicability, and UI-inventory tests;
JSON validation; the full `cargo test --locked` suite (139 library and 46
binary/UI tests); all-targets check; strict Clippy; formatting; and diff
checks. The focused test asserts current v5, both typed registry records,
authoritative selected pattern, and the listed Shapes/Curves values.

No save/reopen, undo/redo, contradictory-adapter, CMYK/RGB, or
preview/PNG/SVG parity work was included. Those remain the next bounded C2/C3
slices. No visual/export artifacts were created in C1.

The C1 evidence and matrix are recorded in:

- `.codex-work/evidence/ton-010-stage-4.5c1-current-format-matrix-f9c138c-dirty.md`
- `.codex-work/agents/desktop-implementer/ton-010-stage-4.5c1-current-format-fixture-foundation.md`

This substage is complete and paused for user feedback. Do not begin C2
automatically.

