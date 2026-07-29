# TON-010 Stage 4.5C2B2-A — output transition/cache authority evidence

- Timestamp: 2026-07-28
- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `f9c138c493a9d687b5300abddf14e78281f2ad63`
- Producing agent: `desktop_implementer`
- Working tree: intentionally dirty before this bounded slice; unrelated work was preserved.

## Scope

C2B2-A only: production `Document`/`DocumentEditor` CMYK↔RGB transition, inactive-cache, renderer, persistence, and undo/redo authority semantics. No realized GTK/UI slice (C2B2-B), C2C, C3 output artifacts, 4.5D, Stage 5, preset/schema change, or obsolete-format opening was started.

## Subsystems inspected

- `src/model.rs`: `OutputTreatmentCache`, `Document::{active_treatment,new_rgb_treatment,switch_output_mode,apply_treatment,sync_legacy_projection,canonicalize_pipeline_facades}`, `DocumentEditor::set_output_mode`, and history snapshots.
- `src/render.rs`: production preview/output canonicalization.
- `src/persistence.rs`: document serialization/load canonicalization and current cache validation.
- Existing C1 preset application and presentation-state coverage.

## Verified findings

The existing transition implementation is authority-correct. On a switch it stores the outgoing treatment cache, applies the target cache/default, then calls `sync_legacy_projection`; that final step replaces the active `RenderVariant` with an adapter rebuilt from the target cache's `pattern_state` and semantic pipeline. Rendering and save/load independently clone and canonicalize before retained legacy dispatch. Cache serialization contains `pattern_state` and pipeline/presentation state, not `render`.

No production defect was demonstrated, so no transition/model source change was made. The new regression intentionally uses opposite-kind/incompatible active and inactive adapters against both C1 production presets:

1. `Polygon Six.tntr` (Shapes) and `Motif Ladder.tntr` (Curves) begin with a contradictory active adapter; CMYK rendering and the first RGB transition retain selected typed authority.
2. The newly created inactive CMYK cache is then given another opposite-kind adapter. RGB edits remain authority-only; returning to CMYK restores the cached CMYK `pattern_state` and its rendered pixels.
3. Undo/redo restores/reapplies the transition as ordinary history.
4. Save/reopen preserves active CMYK and inactive RGB pattern states; reopening and returning to RGB restores its edited typed state and rendered pixels.
5. CMYK preview surface restores independently, while the saved Export Background remains unchanged throughout transitions and reopen.

## Adapter/cache inventory

| State | Purpose | Verified authority behavior |
| --- | --- | --- |
| Active `Document.render` | Legacy Shapes/Curves execution facade. | `sync_legacy_projection` overwrites it from active `pattern_state` after every output transition; renderer/save/load also canonicalize local clones. |
| `inactive_cmyk` / `inactive_rgb` `pattern_state` | Full per-output typed treatment authority. | Cached/restored across transitions and serialized current-format. The new test proves their contradictory `render` fields cannot select a family or change typed parameters. |
| Cache `render` | Skipped legacy executor snapshot retained for compatibility. | Rebuilt by `OutputTreatmentCache::canonicalize_pipeline_facades` before persistence validation and overwritten when its cache becomes active. |
| Cache `preview_surface` | Per-output presentation snapshot. | Restored only to `Document.appearance.preview_surface`; export background is not cached or changed. |
| `saved_web_*` transition snapshots | Crosshatch/ordinary restoration support. | Not exercised beyond normal cache preservation in C2B2-A; C2B-1 already moved Crosshatch source selection to `pattern_state`. |

## Commands and results

- `cargo test --locked persistence::tests::c2b2a_c1_fixtures_keep_pattern_authority_across_output_caches_and_roundtrips -- --exact` — passed.
- Focused existing transition/presentation tests — passed: `model::tests::first_uncached_rgb_transition_preserves_scalar_pipeline_and_restores_cmyk_cache`, `model::tests::rgb_mode_is_lossless_cached_and_one_undoable_edit`, `persistence::tests::appearance_roundtrips_with_output_treatment_preview_snapshots`, and `render::tests::preview_and_export_apply_only_their_own_presentation_state_for_shapes_and_curves`.
- `cargo test --locked` — passed: 144 library tests, 48 binary/UI tests, 0 doc tests.
- `cargo fmt --all -- --check`, `cargo check --locked --all-targets`, `cargo clippy --locked --all-targets -- -D warnings`, and `git diff --check` — passed.

## Files, artifacts, and uncertainty

- Changed product/test file: `src/persistence.rs` (new regression only).
- Changed evidence files: this record and the matching desktop-implementer record.
- Artifacts: none; C3 preview/PNG/SVG parity artifacts are explicitly out of scope.
- No manual GNOME/Wayland, screen-reader, or realized GTK transition validation is claimed; that is C2B2-B if separately approved.

## Invalidation conditions

Re-run this slice if output switching, cache serialization, projection, `DocumentEditor` history, C1 fixtures, or Preview Surface/Export Background ownership changes. C2B2-B remains the next possible, separately authorized realized UI handoff.

## CACHE_UPDATE

4.5C2B2-A is complete pending parent review. Both current-format C1 fixtures now prove that deliberately contradictory active and inactive CMYK/RGB adapter state cannot override selected pattern/typed parameters during output transitions, cache restore, render, save/reopen, undo, or redo. Preview Surface remains per-output and Export Background remains document-wide/export-only. No real authority defect was found; C2B2-B and all later work remain unstarted.
