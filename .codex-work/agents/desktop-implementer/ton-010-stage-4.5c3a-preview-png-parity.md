# Desktop implementer — TON-010 Stage 4.5C3-A preview/PNG parity

Date: 2026-07-28

## Scope and checkout

Bounded C3-A only at Git HEAD `f9c138c493a9d687b5300abddf14e78281f2ad63`. The existing dirty worktree was preserved. C3-B SVG parity, 4.5D, Stage 5, migrations, and unrelated UI changes were not started.

## Exact files changed

- `src/png_export.rs` — focused C1 fixture preview/PNG authority and parity regression.
- `test-artifacts/ton-010-stage-4.5c3a/polygon-six-preview.png` (ignored generated artifact).
- `test-artifacts/ton-010-stage-4.5c3a/polygon-six-export.png` (ignored generated artifact).
- `test-artifacts/ton-010-stage-4.5c3a/motif-ladder-preview.png` (ignored generated artifact).
- `test-artifacts/ton-010-stage-4.5c3a/motif-ladder-export.png` (ignored generated artifact).
- `.codex-work/evidence/ton-010-stage-4.5c3a-preview-png-parity-f9c138c-dirty.md`.
- this record.

## Decisions and reused abstractions

Reused the production parser/candidate, `DocumentEditor`, `pattern_state` projection, preview renderer, PNG exporter, compositors, output cache, and CLI artifact route. The test compares pixel data through the shared transparent canonical pattern image instead of asserting preview and document PNG are directly equal: they intentionally differ only because Preview Surface and Export Background are independent presentation states.

Both active and inactive derived adapters are deliberately contradictory. The test proves renderer projection is read-only and that output/cache restoration reconstructs from typed authority. No implementation defect was observed, so no production path changed.

## Verification and visual inspection

The focused C3-A test passed for both fixtures, the locked full suite passed (145 library and 48 binary/UI tests), and locked check, strict Clippy, fmt, and diff checks passed. Shipping CLI preview and PNG artifacts were generated and opened: Polygon Six visibly has six-sided dot fields; Motif Ladder visibly has repeating curve motifs. Export images are 900×638; AppUi previews are 1280×820.

## Known limitations and follow-up targets

No SVG parity, human click-through, or screen-reader review is claimed. C3-B must separately inspect SVG parity/metadata/visual artifacts. Re-run C3-A when current fixture parsing, typed projection, preview, PNG export, compositors, cache restoration, or presentation ownership changes.
