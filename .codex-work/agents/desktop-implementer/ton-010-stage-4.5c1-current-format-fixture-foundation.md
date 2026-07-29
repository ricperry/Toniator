# TON-010 Stage 4.5C1 implementation evidence

Date: 2026-07-28  
Repository: `/home/ricperry1/projects/Toniator`  
Git HEAD: `f9c138c493a9d687b5300abddf14e78281f2ad63`  
Worktree assumption: intentionally dirty before this slice, including accepted
TON-010 stages, TON-013 migration work, existing bundled-preset rewrites,
evidence, and documentation. Those changes were preserved; no reset, clean,
commit, or push was performed.

## Scope completed

Stage 4.5C1 only: current-format testing-preset matrix and observable fixture
foundation. No 4.5B code was reopened. No C2 persistence/undo/redo/adapter or
CMYK/RGB transition work, C3 preview/PNG/SVG parity artifacts, 4.5D, or
Weighted Voronoi work was started.

## Files changed by this slice

* `assets/presets/Polygon Six.tntr` — new v5 complete-workflow Shapes fixture.
* `assets/presets/Motif Ladder.tntr` — new v5 complete-workflow Curves fixture.
* `src/ui.rs` — adds the two preset bytes to the existing production
  `BUNDLED_PRESETS` inventory and updates its deterministic inventory test.
* `src/preset.rs` — adds the fixtures to the runtime bundled-preset validation
  and a focused authority/schema test.
* `.codex-work/evidence/ton-010-stage-4.5c1-current-format-matrix-f9c138c-dirty.md`
  — concise matrix and C1 boundary record.
* This file — implementation evidence.

## Important decisions and reused abstractions

* Reused the actual bundled-preset path (`BUNDLED_PRESETS`, `parse_treatment`,
  and `ParsedTreatment::candidate_for`) instead of adding a demo-only widget,
  fixture framework, or alternate loader.
* Both presets are current `toniator-preset` v5 `complete-workflow` documents.
  Their `treatment.pattern_state` contains both registered pattern records at
  schema/generator version 1, while only the selected record has the scenario
  values. This is the existing valid authority contract.
* `Polygon Six` selects Shapes and encodes a shared six-sided regular polygon,
  58-cell grid, and changed mark scale/rotation. `Motif Ladder` selects Curves
  and encodes a manual, flipped, five-tile/three-stack motif arrangement.
* The wire fixtures contain no `treatment.render`; the existing candidate path
  derives execution state from authoritative `Document.pattern_state`.

## Verification

Passed:

* `cargo test --locked preset::tests::c1_matrix_presets_keep_selection_and_typed_parameters_in_authoritative_state -- --exact`
* `cargo test --locked preset::tests::every_runtime_bundled_preset_is_current_and_applicable -- --exact`
* `cargo test --locked ui::tests::preset_paths_names_and_bundled_inventory_are_deterministic -- --exact`
* `cargo fmt --all && git diff --check`
* `cargo test --locked` — 139 library tests + 46 binary/UI tests passed.
* `cargo check --locked --all-targets`
* `cargo clippy --locked --all-targets -- -D warnings`
* `jq empty assets/presets/Polygon\\ Six.tntr assets/presets/Motif\\ Ladder.tntr`
* final `cargo fmt --all --check && git diff --check`

No runtime screenshot/export artifact was produced. That is intentional: C1
validates observable production inputs and loader behavior, while C3 owns
preview/PNG/SVG comparison artifacts and parity claims.

## Known limitations and follow-up review targets

* C1 does not prove save/reopen, undo/redo, deliberately contradictory adapters,
  or CMYK/RGB transitions; those remain C2.
* It does not claim a human GNOME/Wayland visual review or image parity; C3 and
  4.5D own those gates.
* Parent should review the matrix values as the baseline for later C2/C3
  scenarios and decide whether more rows are needed after this first Shapes and
  Curves pair.
* Durable tracker/documentation reconciliation is intentionally left to the
  parent/documentation maintainer after milestone review.

## Invalidation conditions

Re-run the focused tests and revise the matrix if `CURRENT_PRESET_VERSION`,
the Shapes or Curves registry schema/generator version, the preset candidate
loader, `BUNDLED_PRESETS`, or either listed typed scenario value changes.

CACHE_UPDATE: Stage 4.5C1 adds two production-loadable v5 test presets.
`Polygon Six` is the regular-polygon Shapes row (six sides); `Motif Ladder` is
the manual flipped motif Curves row. Their authority is only
`Document.pattern_state`; no fixture persists or selects through a
`RenderVariant`. The three focused tests named above are the first rerun gate;
visual/export parity remains deferred to C3.
