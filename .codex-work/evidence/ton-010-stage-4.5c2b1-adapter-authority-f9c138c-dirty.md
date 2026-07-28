# TON-010 Stage 4.5C2B-1 — contradictory adapter authority evidence

- Timestamp: 2026-07-28
- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `f9c138c493a9d687b5300abddf14e78281f2ad63`
- Producing agent: `desktop_implementer`
- Working tree: intentionally dirty before this bounded substage; unrelated TON-010/TON-013, resources, preset, fixture, documentation, and evidence work was preserved.

## Scope

C2B-1 only: current-format contradictory in-memory Shapes/Curves adapter
coverage and the one real Crosshatch transition authority leak found during the
audit. It does not start C2B-2 CMYK/RGB transition work, C3 output artifacts,
4.5D, or Stage 5.

## Subsystems and symbols inspected

- `src/model.rs`: `PatternDocumentState::{adapter,shape_settings,curve_settings}`, `Document::{sync_legacy_projection,canonicalize_pipeline_facades,projected_for_render}`, `DocumentEditor::{set_pattern_state,set_shape_settings,set_curve_settings,apply_legacy_mapping_action,undo,redo}`, and transition/cache snapshots.
- `src/render.rs`: `generate_document_pattern_output_cancellable`, preview/output rendering.
- `src/persistence.rs`: `document_json`, `save_document_atomic`, `load_document`, and C1 fixture persistence tests.
- `src/png_export.rs`, `src/svg_export.rs`, and `src/preset.rs`: canonicalization before retained adapter consumers.
- `src/ui.rs`: existing realized `AppUi`/GResource contradiction and selector transition coverage.

## Verified findings and correction

`Document.pattern_state` remains the sole serialized Shapes/Curves selector and
typed parameter authority. Rendering clones and canonicalizes the document
before legacy dispatch; current save and load similarly rebuild `render` from
pattern authority. PNG artboard, SVG export, and preset serialization operate
on a canonicalized local document before using the retained adapter.

The audit found one exception in the Crosshatch entry transition:
`DocumentEditor::apply_legacy_mapping_action(CrosshatchLuminance)` selected
its Shapes/Curves source by matching `Document.render`. A deliberately
contradictory adapter could therefore make that transition save/configure the
wrong family. The bounded correction now chooses and reads the source only
from `PatternDocumentState`; it continues to use the same existing snapshots,
pipeline projection, undo/history, and renderer behavior.

New current-format proof uses the production `Polygon Six.tntr` and `Motif
Ladder.tntr` preset bytes, `parse_treatment`/candidate application, a valid
PNG source, the production renderer, atomic document save/reopen, and
`DocumentEditor` history. For each family it deliberately installs the
opposite adapter kind with incompatible dimensions/parameters and proves:

1. render pixels still equal the authority-only fixture output;
2. a typed authority edit saves only `pattern_state` and reopens with the
   selected typed settings and corresponding rendered pixels;
3. undo restores the fixture `pattern_state` while rendering ignores the
   restored contradictory adapter; and redo restores the edited authority.

The model regression separately proves a Shapes-authority/Curves-adapter
contradiction enters Crosshatch from the authoritative Shape settings, saves
the Shape snapshot rather than a Curve snapshot, and has correct undo/redo.
Existing realized GTK coverage continues to verify contradictory Shapes and
Curves selector/panel/control synchronization and both selector transitions
through the shipping AppUi/GResource path.

## Remaining adapter inventory

| Adapter/projection | Current bounded purpose | Authority status and next boundary |
| --- | --- | --- |
| `Document.render` | Derived legacy Shapes/Curves executor consumed only after document canonicalization in render/export paths. | Never serialized; this substage proves contradictory kind/parameters cannot affect render, save/reopen, or history rendering. Retain until canonical generators replace legacy dispatch. |
| `OutputTreatmentCache.render` | Per-output cached derived executor. | Rebuilt from each cache's `pattern_state` by `canonicalize_pipeline_facades`; CMYK/RGB cache-transition proof is C2B-2. |
| `saved_web_shape` / `saved_web_curve` plus pipeline snapshots | In-memory ordinary-treatment restore support for Crosshatch and treatment/output transitions. | Skipped from persistence. Crosshatch source is now authority-derived; broader CMYK/RGB restoration remains C2B-2. |
| Preset channel extraction and PNG/SVG artboard helpers | Read retained adapter fields for legacy channel/artboard encoding after a local canonical projection. | Cannot choose or persist selection; retain until canonical output owns those legacy encoders. |
| Test-only direct adapter setters/fixtures | Create deliberate contradictions and legacy projection assertions. | Test-only; retain while compatibility adapter proof is required. |

## Commands and results

- `cargo test --locked persistence::tests::c2b1_c1_fixtures_ignore_contradictory_transient_adapters_across_render_save_and_history -- --exact` — passed.
- `cargo test --locked model::tests::crosshatch_transition_uses_authoritative_shapes_not_a_contradictory_curve_adapter -- --exact` — passed.
- Existing authority/render/history/GTK focused tests — all passed:
  - `model::tests::authority_read_accessors_ignore_a_contradictory_transient_adapter`
  - `render::tests::renderer_projection_ignores_contradictory_facades_without_mutating_input`
  - `persistence::tests::c2a_c1_fixtures_save_reopen_and_undo_redo_authoritative_pattern_edits`
  - `ui::tests::realized_numeric_controls_leave_continuous_scroll_to_parent`
- `cargo test --locked` — passed: 143 library tests, 48 binary/UI tests, 0 doc tests.
- `cargo fmt --all -- --check` — passed.
- `cargo check --locked --all-targets` — passed.
- `cargo clippy --locked --all-targets -- -D warnings` — passed.
- `git diff --check` — passed.

## Files and artifacts

- Changed product/test files: `src/model.rs`, `src/persistence.rs`.
- Changed evidence files: this record and the matching desktop-implementer record.
- Artifacts: none. This is state/render/persistence proof; no C3 preview/PNG/SVG parity artifact was created.

## Uncertainty and invalidation

No manual GNOME/Wayland interaction or screen-reader acceptance is claimed.
Re-run the C2B-1 focused tests if `PatternDocumentState` projection,
Crosshatch entry/exit, legacy renderer dispatch, persistence canonicalization,
the two C1 presets, or GTK selector synchronization changes. C2B-2 still owns
the CMYK/RGB cache/transition contradiction matrix.

## CACHE_UPDATE

4.5C2B-1 is complete pending parent review. The C1 Shapes and Curves presets
now have end-to-end contradictory-adapter proof for rendering, save/reopen,
undo, redo, and existing shipped UI selector transitions. One Crosshatch entry
defect was found and fixed: it had read `Document.render` to select its source
treatment; it now reads `Document.pattern_state` only. `Document.render`,
inactive output caches, saved transition snapshots, and legacy preset/export
projections remain deliberately bounded derived adapters. CMYK/RGB transition
work was not begun and belongs only to C2B-2.
