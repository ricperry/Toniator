# Desktop implementer — TON-010 Stage 4.5C2 export-background authoring correction

Date: 2026-07-28

## Scope and baseline

Bounded follow-up only. Git HEAD was `f9c138c493a9d687b5300abddf14e78281f2ad63`; the worktree was already intentionally dirty with completed TON-010/TON-013 work. No unrelated file was reset, cleaned, reverted, committed, or pushed.

## Implementation

Reused the existing shipping `resources/toniator-window.blp` -> build script/GResource -> `gtk::Builder::from_resource` route and existing `DocumentAppearance`, `AppUi::update_appearance`, and `DocumentEditor::set_appearance` abstractions. Added no duplicate state or alternative export path.

The former defect was in `export_background` selection: it copied the color button's inactive RGBA, which could be transparent. The bounded fix derives `Color` from the current authoritative saved export background and initializes only `None` to `RgbaColor::WHITE`. The visible label exposes None/transparent or the precise current RGBA; the retained color dialog is directly selectable and has a dynamic accessible name/description. The color and mode callbacks still enter the normal undo/autosave/render-refresh flow.

## Exact files changed

- `resources/toniator-window.blp`
- `src/ui.rs`
- `src/model.rs`
- `.codex-work/evidence/ton-010-stage-4.5c2-export-background-authoring-correction-f9c138c-dirty.md`
- this file

## Verification and runtime evidence

- Formatting, locked all-target check, locked full test suite (141 library + 48 binary/UI), strict locked Clippy, and diff check passed.
- Focused unit/model/PNG/persistence tests passed.
- The existing realized GTK AppUi regression was extended and passed through the shipped resource. It presents the app and checks None transparency, a visible label, Color -> opaque white, direct color selection, preview-surface isolation, and undo/redo.
- No screenshot was generated and no manual acceptance is claimed. Existing export-pixel test coverage verifies document-background compositing; the realized test verifies the authoring controls.

## Known limits and review targets

- Manual GNOME/Wayland inspection at wide and narrow inspector widths remains useful for presentation acceptance, especially the expanded Appearance section and color-dialog interaction.
- Accessibility properties are set dynamically; GTK test coverage validates the visible state, dialog title, tooltip, and construction path, but does not use a screen-reader harness.
- Durable docs/tracker reconciliation is intentionally left to the parent/documentation maintainer.

## Invalidation conditions

Re-run this correction's GTK/resource and export coverage if the Appearance Blueprint IDs, `update_appearance`, `DocumentAppearance`, or PNG dialog's `Document` background selection are changed. Revisit the policy only if the product changes its default export-background convention away from white.
