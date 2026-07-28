# TON-010 Stage 4.5C2 PNG export-background correction evidence

Date: 2026-07-28

Repository baseline: `f9c138c493a9d687b5300abddf14e78281f2ad63`, intentionally
dirty worktree. This is a bounded C2 PNG dialog correction only; it does not
begin C2B, C3, 4.5D, or Stage 5.

## Shipping-path diagnosis

Verified production path:

`PNG dialog selection` → `PngBackground::Document` →
`png_export::png_bytes_cancellable` →
`render::render_document_export_cancellable` →
`Document.appearance.export_background`.

`PngBackground::Document` was already correct. The renderer composites the
saved export background only; it never uses `PreviewSurface`.

The supplied files were inspected:

* `/tmp/Screenshot From 2026-07-28 18-31-34.png` — 2544×1624 RGBA; the PNG
  dialog selected `Document Export Background`, while the Appearance / Canvas
  & Export section was collapsed.
* `/tmp/Toniator Example — Halftone.png` — 900×638 RGBA; alpha minimum is 0,
  consistent with a transparent saved Export Background. The screenshot alone
  cannot expose the saved appearance value.

Defect/cause: dialog ambiguity, not an export mismatch. The former summary
repeated only `Document Export Background`, so a creator could not tell whether
the effective saved value was `None`/transparent or a color without dismissing
the dialog and expanding Appearance.

## Correction

`src/ui.rs` now shows the effective exported background in the PNG dialog
summary:

* `Document Export Background: None (transparent)`;
* `Document Export Background: #RRGGBBAA` for a saved color; or
* an explicit override that says it ignores the saved Export Background.

The same current text is applied as the summary's accessible label and the
dropdown's accessible description. The PNG help copy now says that the dialog
summary reveals the current None/transparent or color value. No appearance
state, default, Preview Surface, or rendering behavior changes.

## Regression coverage and validation

* `png_export::tests::document_png_uses_saved_export_background_and_none_remains_transparent`
  explicitly selects `PngBackground::Document`; it proves `None` emits
  transparent pixels and a saved opaque color emits an opaque PNG containing
  the background color. It also proves preview-surface changes do not alter
  Document PNG bytes and a Transparent Override does not mutate the document.
* `ui::tests::png_background_summary_exposes_the_saved_document_value_and_overrides`
  verifies None/transparent, RGBA saved-color, and both override descriptions.
* `ui::tests::realized_numeric_controls_leave_continuous_scroll_to_parent`
  passed through the shipping GResource/Builder path.

Passed commands:

* the three focused tests above;
* `cargo fmt --all && git diff --check`;
* `cargo test --locked` — 140 library tests + 47 binary/UI tests passed;
* `cargo check --locked --all-targets`;
* `cargo clippy --locked --all-targets -- -D warnings`.

No new image artifact was produced. The supplied images were inspected only;
visual/manual confirmation of the newly visible dialog wording remains a
human GTK/Wayland check. PNG/SVG parity artifact work remains C3.

CACHE_UPDATE: `PngBackground::Document` correctly uses the saved
`Document.appearance.export_background`, with `None` preserving transparency;
Preview Surface is not export authority. The PNG dialog now exposes the
effective saved value and accessible equivalent. Re-run the named PNG/UI tests
if export composition, appearance state, PNG dialog labels, or accessibility
properties change. This record is invalidated by such changes.
