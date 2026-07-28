# TON-010 Stage 4.5C2B2-B — realized CMYK/RGB AppUi authority evidence

- Timestamp: 2026-07-28
- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `f9c138c493a9d687b5300abddf14e78281f2ad63`
- Producing agent: `desktop_implementer`
- Working tree: intentionally dirty before this bounded slice; prior TON-010, TON-013, preset, resource, evidence, and documentation changes were preserved.

## Scope

C2B2-B only: realized shipping Blueprint/GResource `AppUi` coverage for CMYK↔RGB output switching. C2C, C3, 4.5D, Stage 5, obsolete-format support, and UI reorganization were not started.

## Production route inspected

- `resources/toniator-window.blp` -> Blueprint compiler -> `resources/toniator.gresource.xml` -> `gtk::Builder::from_resource(WINDOW_UI_RESOURCE)` -> `AppUi::new`.
- `src/ui.rs`: output-mode `selected-notify`, `after_output_mode_edit`, deferred `sync_controls_when_idle`, `sync_controls`, Shapes/Curves selector and scalar controls, Preview Surface, and Export Background controls.
- `src/model.rs`: `DocumentEditor::set_output_mode`, `Document::switch_output_mode`, and `OutputTreatmentCache`.
- Current production fixtures: bundled `assets/presets/Polygon Six.tntr` and `assets/presets/Motif Ladder.tntr`.

## Verified findings

No shipping UI authority defect was demonstrated. `AppUi` receives its controls from the shipping resource, the output selector uses `DocumentEditor::set_output_mode`, and its deferred synchronization preserves live dropdown model identity.

The new realized regression applies each bundled C1 fixture through the production preset parser/candidate path, then deliberately installs an opposite-kind active adapter and later corrupts the inactive CMYK adapter. It verifies that real selector panels and typed controls continue to reflect the active `Document.pattern_state` through the following user-visible route:

1. CMYK Shapes/Regular Polygon or Curves/Motif panel and typed values are selected from the fixture's authority, not its contradictory active facade.
2. The shipping output dropdown switches to RGB and preserves authoritative selected pattern/typed values.
3. An RGB edit uses the real polygon-side or curve-weight callback and persists through `DocumentEditor` authority APIs.
4. Returning through the shipping dropdown restores the cached CMYK authority despite its contradictory cached facade.
5. UI undo/redo restores the RGB edit and CMYK cache as ordinary workflow states.
6. CMYK Preview Surface restores independently (warm custom color -> RGB black default -> warm custom color); document Export Background remains the saved dark color and its actual controls throughout.

The regression explicitly settles the normal `sync_controls` projection after the callback/idle boundary before asserting widgets. This keeps the test deterministic when the full parallel test suite has unrelated GTK sources pending on the shared main context; it does not alter shipping synchronization.

## Adapter/cache inventory implications

| Adapter/cache | C2B2-B result |
| --- | --- |
| Active `Document.render` | Deliberately opposite-kind facade cannot change realized selector panel, polygon sides/coverage, or curve weight/coverage. |
| Inactive CMYK cache `render` | Deliberately opposite-kind facade cannot change active RGB controls or the restored CMYK pattern state. |
| Active/inactive `pattern_state` | Remains typed selection/parameter authority through the real dropdown, callback edit, undo, and redo. |
| `preview_surface` cache | Per-output presentation state restores on return to CMYK. |
| `export_background` | Document-wide export-only state remains independent of transitions and Preview Surface. |

## Commands and results

- `cargo fmt --all` — passed.
- `cargo test --locked --bin toniator realized_numeric_controls_leave_continuous_scroll_to_parent -- --nocapture` — passed; exercised actual shipping `AppUi` Builder/GResource surface and existing realized GTK coverage.
- `cargo test --locked persistence::tests::c2b2a_c1_fixtures_keep_pattern_authority_across_output_caches_and_roundtrips -- --exact` — passed.
- `cargo test --locked` — passed: 144 library tests, 48 binary/UI tests, 0 doc tests.
- `cargo check --locked --all-targets` — passed.
- `cargo clippy --locked --all-targets -- -D warnings` — passed.
- `cargo fmt --all -- --check` — passed.
- `git diff --check` — passed.

## Files and artifacts

- Product/test source changed: `src/ui.rs` (realized regression only; no shipping logic/resource/schema/preset change).
- Evidence changed: this record and the matching desktop-implementer record.
- Visual artifacts: none. This slice added no visual behavior and C3 preview/PNG/SVG artifact generation is explicitly out of scope.

## Remaining uncertainty and review targets

Automated realized GTK coverage is complete for the bounded route, but no human GNOME/Wayland click-through or screen-reader session was performed. Review the actual desktop interaction only if a later stage changes output controls, deferred synchronization, resource IDs, cache lifecycle, or accessibility semantics. C2C and later stages remain unstarted.

## Invalidation conditions

Re-run this evidence if `resources/toniator-window.blp`, GResource compilation, AppUi output callbacks/synchronization, `DocumentEditor::set_output_mode`, `OutputTreatmentCache`, C1 fixtures, or Preview Surface/Export Background ownership changes.

## CACHE_UPDATE

4.5C2B2-B is complete pending parent review. The actual shipping Blueprint/GResource `AppUi` now has realized regression coverage proving CMYK/RGB switching, selectors, typed Shapes/Curves parameters, RGB edits, cache restoration, undo/redo, and presentation separation follow authoritative `Document.pattern_state`/appearance despite deliberately contradictory active and inactive adapters. No shipping UI defect was found and no later stage was started.
