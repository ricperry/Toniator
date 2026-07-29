# TON-010 Stage 4.5C2 PNG export-background correction

Date: 2026-07-28  
Repository: `/home/ricperry1/projects/Toniator`  
Git HEAD: `f9c138c493a9d687b5300abddf14e78281f2ad63`  
Producing agent: desktop-implementer

## Working-tree assumptions

The checkout was intentionally dirty before this correction, with accepted
TON-010 work, TON-013 migration work, C1/C2A evidence and fixtures, and other
user changes. All were preserved. No reset, clean, commit, push, or destructive
operation was performed.

## Files changed

* `src/ui.rs` — effective saved-background summary, accessible label/
  description, help clarification, and focused summary test.
* `src/png_export.rs` — strengthens and renames the existing regression to
  select `PngBackground::Document` explicitly for transparent and colored
  saved background cases.
* `.codex-work/evidence/ton-010-stage-4.5c2-png-export-background-correction-f9c138c-dirty.md`
  — reusable diagnosis/evidence.
* This file.

## Verified findings

* `PngBackground::Document` already routes to
  `render_document_export_cancellable`, which composites only
  `Document.appearance.export_background`.
* `PreviewSurface` remains preview-only and no `RenderVariant` participates in
  appearance authority.
* The supplied output PNG is 900×638 RGBA with alpha minimum 0; the supplied
  dialog screenshot is 2544×1624 RGBA and does not expose its collapsed saved
  Appearance setting.
* The real defect was visibility/ambiguity: the former dialog text did not
  reveal whether Document meant saved None/transparent or an actual color.

## Existing abstractions reused

The correction reuses the current `ExportBackground` enum, `PngBackground`
choice, GTK accessible-property API, and existing PNG dialog summary. No new
state, renderer branch, persistence field, or dialog widget was introduced.

## Verification

Passed:

* `cargo test --locked png_export::tests::document_png_uses_saved_export_background_and_none_remains_transparent -- --exact`
* `cargo test --locked ui::tests::png_background_summary_exposes_the_saved_document_value_and_overrides -- --exact`
* `cargo test --locked ui::tests::realized_numeric_controls_leave_continuous_scroll_to_parent -- --exact`
* `cargo fmt --all && git diff --check`
* `cargo test --locked` — 140 library tests and 47 binary/UI tests passed.
* `cargo check --locked --all-targets`
* `cargo clippy --locked --all-targets -- -D warnings`

Artifacts inspected, not generated:

* `/tmp/Screenshot From 2026-07-28 18-31-34.png` (2544×1624 RGBA)
* `/tmp/Toniator Example — Halftone.png` (900×638 RGBA; transparent pixels)

## Limitations and handoff boundary

No live manual GTK/Wayland review of the new dialog wording was performed and
no screenshot/export parity artifact was generated. C2B adapter-contradiction
and CMYK/RGB-transition work, C3 output parity artifacts, 4.5D, and Stage 5
remain explicitly out of scope.

Documentation likely affected: parent/documentation maintainer may later add
the resolved PNG-dialog ambiguity to the durable Stage 4.5 record after review.

## Invalidation conditions

Re-run the named tests if `PngBackground`, `ExportBackground`, PNG export
composition, dialog-summary construction, help text, or GTK accessibility
properties change.

CACHE_UPDATE: The shipping Document PNG path is correct: it uses
`Document.appearance.export_background`; `None` is transparent and a saved
color is flattened. This correction makes the PNG dialog disclose that current
effective value with accessible text, without changing Preview Surface or
saved appearance state.
