# TON-010 Stage 4.5C2 export-background authoring correction

Date: 2026-07-28

Git baseline: `f9c138c493a9d687b5300abddf14e78281f2ad63`; the worktree was intentionally dirty before this bounded correction and remains preserved.

## Defect and resolution

The shipping Appearance section exposed `Export Background` as `None`/`Color` plus an unlabeled color button. Its `selected-notify` callback read the inactive button's RGBA. GTK's initial button value could be transparent, so switching from `None` to `Color` saved `ExportBackground::Color { alpha: 0 }`, which still exported transparent output.

The shipping Blueprint resource now keeps the existing color button but gives it a visible current-value label. `None` displays `Background Color · None (transparent)` and disables the button. `Color` selects the existing saved color, or initializes a former `None` value to opaque white, then displays `#RRGGBBAA`. The button has the accessible name `Export Background Color` and a state-specific accessible description/tooltip. The direct color callback remains the sole route for a user-chosen color.

All changes use `AppUi::update_appearance` -> `DocumentEditor::set_appearance`; that retains undo, autosave, rendered-preview refresh, and save/reopen behavior. `PreviewSurface` is not read or mutated by the export-background callbacks. PNG `Document Export Background` still reads only `Document.appearance.export_background`; prior coverage confirms `None` stays transparent.

## Files changed in this correction

- `resources/toniator-window.blp` — shipping GResource Blueprint label and color-control row.
- `src/ui.rs` — authoritative mode conversion/synchronization, accessible/current-value disclosure, help text, and pure plus realized GTK regression coverage.
- `src/model.rs` — focused authoritative appearance/undo regression.
- This evidence file and the matching desktop-implementer record.

## Verification

- `cargo fmt --all -- --check` — passed.
- `cargo check --locked --all-targets` — passed; recompiles the Blueprint/GResource path.
- `cargo test --locked export_background_color_selection_defaults_to_opaque_white_and_keeps_saved_color` — passed.
- `cargo test --locked export_background_is_authoritative_and_does_not_change_preview_surface` — passed.
- `cargo test --locked document_png_uses_saved_export_background_and_none_remains_transparent` — passed.
- `cargo test --locked appearance_roundtrips_with_output_treatment_preview_snapshots` — passed.
- `cargo test --locked realized_numeric_controls_leave_continuous_scroll_to_parent -- --nocapture` — passed. Its AppUi/GResource route exercises discoverability, visible None label, opaque-white Color initialization, direct `#0C2238FF` selection, preview-surface preservation, and undo/redo.
- `cargo test --locked` — passed: 141 library tests and 48 binary/UI tests.
- `cargo clippy --locked --all-targets -- -D warnings` — passed.
- `git diff --check` — passed.

## Artifacts and limits

No new raster artifact was created: the realized GTK test presents the shipping AppUi and asserts the control state, but this run did not claim human/manual visual acceptance or drive the user's desktop. The existing prior PNG coverage is the export-pixel evidence; a future manual review can inspect the expanded Appearance section at wide and narrow widths.

## CACHE_UPDATE

Stage 4.5C2 export-background authoring correction is bounded complete pending parent review. The actual defect was UI state ambiguity, not PNG compositor behavior: selecting Color from saved None had copied a transparent inactive widget value. The shipping Builder/GResource surface now provides an explicit label/current RGBA, defaults that transition to opaque white, preserves explicit None transparency, and writes only `Document.appearance.export_background` through `update_appearance`. No C2B, C3, 4.5D, Stage 5, schema, preset, fixture, or pattern-authority work was started.
