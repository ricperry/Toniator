# TON-010 Stage 4.5C1 — current-format preset matrix

Date: 2026-07-28

Baseline: `f9c138c493a9d687b5300abddf14e78281f2ad63`, intentionally dirty
worktree. This record covers only the first current-format fixture rows. It
does not validate save/reopen, undo/redo, contradictory adapters, output-model
transitions, or preview/PNG/SVG parity; those remain C2/C3 work.

| Runtime bundled fixture | Authoritative selection and schema | Stable typed values | Intended visible distinction |
| --- | --- | --- | --- |
| `assets/presets/Polygon Six.tntr` (`Polygon Six`) | `pattern_state.selected = compat.shapes.v1`; both registered records declare schema/generator `1`; `render` is absent | Shapes: shared `regularpolygon`, 6 sides, 58-cell grid, marks 6–76, base rotation 15°, scale 0.82 | Six-sided marks, wider grid rhythm, and a rotated shared mark treatment distinguish it from the circle-based bundled Shapes example. |
| `assets/presets/Motif Ladder.tntr` (`Motif Ladder`) | `pattern_state.selected = compat.curves.v1`; both registered records declare schema/generator `1`; `render` is absent | Curves: `motif-pattern`, no background, 34 cells, manual coverage, curve scale 46, 5 tiles, 3 stacks, flipped alternate tiles | Repeated staggered wave motifs with intentionally sparse/manual coverage distinguish it from full-width curves and the existing motif stress preset. |

Both are v5 `complete-workflow` presets and are compiled into the production
`BUNDLED_PRESETS` inventory. The existing load-preset path parses each through
`parse_treatment`, constructs a candidate from `Document.pattern_state`, and
then derives the legacy execution projection. No fixture reads or serializes a
`RenderVariant` as its selection or parameter authority.

Focused automated coverage:

* `preset::tests::c1_matrix_presets_keep_selection_and_typed_parameters_in_authoritative_state`
  verifies the v5 envelope, no `treatment.render`, both typed schema records,
  selected authority, and every listed deterministic Shapes/Curves value.
* `preset::tests::every_runtime_bundled_preset_is_current_and_applicable`
  verifies every production bundled byte sequence, including these rows,
  parses and applies to a candidate document.
* `ui::tests::preset_paths_names_and_bundled_inventory_are_deterministic`
  verifies the real UI inventory labels and byte presence.

Artifacts: none. C1 establishes loadable, visibly distinct fixture inputs;
creating preview, PNG, or SVG comparison artifacts is explicitly deferred to
C3.

CACHE_UPDATE: C1 fixture authority is `Document.pattern_state` in current
preset v5. Use `Polygon Six` for Shapes/Regular Polygon (six sides) and
`Motif Ladder` for Curves/motif scalar coverage. Re-run the three named tests
after any preset schema, registry-version, or bundled-inventory change. This
evidence is invalidated if `CURRENT_PRESET_VERSION`, either compatibility
pattern schema/generator version, preset loader semantics, or the listed
typed values change.
