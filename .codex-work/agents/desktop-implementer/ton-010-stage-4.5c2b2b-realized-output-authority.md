# Desktop implementer — TON-010 Stage 4.5C2B2-B realized output authority

Date: 2026-07-28

## Scope and checkout

Bounded C2B2-B only at Git HEAD `f9c138c493a9d687b5300abddf14e78281f2ad63`. The worktree was intentionally dirty before implementation; all existing changes were preserved. C2C, C3, 4.5D, Stage 5, obsolete-format opening, and broad UI changes were not started.

## Exact files changed

- `src/ui.rs` — test-only realized `AppUi` CMYK/RGB cache-authority regression.
- `.codex-work/evidence/ton-010-stage-4.5c2b2b-realized-output-authority-f9c138c-dirty.md`.
- this record.

## Implementation decisions and reused abstractions

The regression uses the shipping GResource route (`AppUi::new`, `WINDOW_UI_RESOURCE`, real Blueprint objects), existing `BUNDLED_PRESETS` fixture bytes, `parse_treatment`/`candidate_for`, `DocumentEditor::set_output_mode`, normal real output-dropdown callbacks, `sync_controls`, and UI undo/redo. It does not construct a parallel widget tree or modify product behavior.

Both C1 fixtures are started from an unchanged production source document. The test corrupts only derived active and inactive cache adapters, verifies authority-projected controls, makes one real typed RGB control edit, then returns through CMYK and history. Preview Surface and Export Background are asserted separately.

One test-harness consideration was recorded: after the shipping output callback completes, the regression explicitly settles the ordinary projection before reading controls. This avoids test-order timing from unrelated GTK sources on the shared context; it is test-only and preserves the deferred production handler.

## Verification and artifacts

Focused realized GTK test, focused C2B2-A persistence test, full locked test suite (144 library + 48 binary/UI), locked all-target check, strict locked Clippy, formatting, and diff checks passed. No screenshots, exported images, or C3 artifacts were created because no visual behavior changed and output-artifact work is out of scope.

## Known limitations and follow-up review targets

No manual desktop or screen-reader acceptance is claimed. Revisit this test if the Blueprint/GResource path, output dropdown deferral, authority projection, cache lifecycle, C1 fixtures, or Preview/Export ownership changes. Durable documentation remains parent-owned.

## Invalidation conditions

Invalidate this record upon changes to `resources/toniator-window.blp`, GResource build wiring, `src/ui.rs` output/appearance synchronization, `src/model.rs` transition/cache logic, C1 fixtures, or presentation-state semantics.
