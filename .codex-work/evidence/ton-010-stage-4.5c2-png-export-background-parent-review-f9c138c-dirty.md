# TON-010 Stage 4.5C2 correction — parent review

- Recorded: 2026-07-28
- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `f9c138c493a9d687b5300abddf14e78281f2ad63`
- Scope: PNG export-background diagnosis and dialog correction only.

## Diagnosis

The supplied `/tmp/Toniator Example — Halftone.png` is 900×638 RGBA with
transparent pixels. The shipping path is correct:

`PngBackground::Document` → `render_document_export_cancellable` → saved
`Document.appearance.export_background`.

The screenshot's PNG dialog selected “Document Export Background,” but the
Appearance / Canvas & Export section was collapsed. The saved value was
`ExportBackground::None`, so transparent output was expected. The defect was
that the dialog did not disclose the effective saved value and therefore made
the correct transparent result appear incorrect.

## Correction

`src/ui.rs` now reports the effective value in the PNG dialog summary:

- `Document Export Background: None (transparent)`;
- `Document Export Background: #RRGGBBAA`; or
- an explicit override that states it ignores the saved setting.

The summary is also exposed through accessible label/description properties.
`src/png_export.rs` explicitly tests `PngBackground::Document` with both saved
None and saved opaque color. Preview Surface remains preview-only; no default,
appearance state, renderer branch, or persisted schema was changed.

## Parent verification

Passed focused PNG composition, PNG summary, and realized GTK tests; full
`cargo test --locked` (140 library and 47 binary/UI tests); all-targets check;
strict Clippy; formatting; and diff checks. No new artifact was generated;
the supplied screenshot and PNG were inspected. Live GNOME/Wayland review of
the revised wording remains outstanding. C2B, C3, 4.5D, and Stage 5 were not
started.

This correction is complete and paused for user feedback before C2B.

