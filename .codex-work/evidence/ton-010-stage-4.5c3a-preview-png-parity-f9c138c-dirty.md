# TON-010 Stage 4.5C3-A — current-format preview/PNG parity evidence

- Timestamp: 2026-07-28
- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `f9c138c493a9d687b5300abddf14e78281f2ad63`
- Producing agent: `desktop_implementer`
- Working tree: intentionally dirty before this bounded slice; all prior source, preset, resource, evidence, and documentation work was preserved.

## Scope

C3-A only: current-format `Polygon Six` and `Motif Ladder` preview/PNG parity. SVG parity/C3-B, 4.5D, Stage 5, migrations, and unrelated UI work were not started.

## Production paths inspected

- Current preset parser/candidate path: `preset::parse_treatment` -> `ParsedTreatment::candidate_for` -> `DocumentEditor::replace_with_preset_candidate`.
- Preview: `render::render_document_preview` -> cloned document normalization/canonicalization -> canonical pattern output -> `composite_preview`.
- PNG: `png_export::png_bytes` -> `render_document_export_cancellable` for `PngBackground::Document`, or `render_document_output_cancellable` for explicit transparent output.
- Authority projection: `Document::projected_for_render` / `canonicalize_pipeline_facades`; typed `Document.pattern_state` remains the selection/parameter source.

## Verified parity method

The new focused regression loads each production C1 fixture through the real parser/candidate path, creates a deliberately opposite-kind active adapter, and establishes the transparent canonical pattern image by calling `render_document_output(..., false, None)`.

It then proves exact pixel equality for the same authoritative raw image across both presentation routes:

1. Preview pixels equal `composite_preview(raw_pattern, Document.appearance)`.
2. Transparent PNG decodes exactly to `raw_pattern`; two encodes are byte-identical and retain transparent pixels.
3. `PngBackground::Document` with saved `ExportBackground::None` also decodes exactly to `raw_pattern`.
4. Changing Preview Surface changes preview but leaves document PNG bytes unchanged.
5. Setting saved Export Background to opaque `#0C2238FF` changes document PNG to `composite_export_background(raw_pattern, export_background)`, makes every PNG pixel opaque, and leaves preview pixels unchanged.
6. The test creates an RGB cache from the contradictory active facade, corrupts its inactive CMYK adapter, returns to CMYK, and proves the restored preview/raw/PNG pixels still equal the typed-authority results.

Both fixtures preserve their intended current-format visual distinction: `Polygon Six` produces six-sided CMYK dot fields; `Motif Ladder` produces the sparse/repeating wavy motif treatment. No production defect was demonstrated, so only regression coverage was added.

## Inspectable artifacts

Generated through the shipping CLI demo + preset-import + preview screenshot + PNG-export route; `test-artifacts/` is intentionally ignored by Git.

| Fixture | Preview artifact | PNG export artifact | Dimensions | SHA-256 |
| --- | --- | --- | --- | --- |
| Polygon Six | `test-artifacts/ton-010-stage-4.5c3a/polygon-six-preview.png` | `test-artifacts/ton-010-stage-4.5c3a/polygon-six-export.png` | preview 1280×820; export 900×638 | preview `e0ae0c38c7c6fd6e9a5fd76f7fb5203887cc9117e44eb9b572f32d9ee1e9ad54`; export `4b03308fe82a47e4a5155b62df18bae9b97ffd8d80e673a035998d77dc03f48d` |
| Motif Ladder | `test-artifacts/ton-010-stage-4.5c3a/motif-ladder-preview.png` | `test-artifacts/ton-010-stage-4.5c3a/motif-ladder-export.png` | preview 1280×820; export 900×638 | preview `d9271a4f48f70f203a9b588e8076942b638d4859c165298b2af88a6600756551`; export `230ad574347867f797f7cdddd20f2b310f43e2a5d605fa4d3da81fa497d67853` |

All four images were opened and inspected. The preview images show the real shipping AppUi with `Shapes · All inks` / `Curves · All inks`; exported PNGs show the corresponding artwork-only treatments. The export images visually display their transparent gaps against the viewer's dark backdrop, consistent with the default saved Export Background of None; the regression separately verifies alpha and saved-color composition exactly.

## Commands and results

- `cargo fmt --all` — passed.
- `cargo test --locked png_export::tests::c3a_c1_fixtures_preview_and_png_share_authoritative_pattern_output -- --exact` — passed (both fixtures; 18.20s).
- `cargo run --locked -- --demo --preset 'assets/presets/Polygon Six.tntr' --screenshot test-artifacts/ton-010-stage-4.5c3a/polygon-six-preview.png --export-png test-artifacts/ton-010-stage-4.5c3a/polygon-six-export.png` — passed.
- Corresponding `Motif Ladder` CLI artifact command — passed.
- `cargo test --locked` — passed: 145 library tests, 48 binary/UI tests, 0 doc tests.
- `cargo check --locked --all-targets` — passed.
- `cargo clippy --locked --all-targets -- -D warnings` — passed.
- `cargo fmt --all -- --check` and `git diff --check` — passed.

## Files and limitations

- Product/test source changed: `src/png_export.rs` (one C3-A regression; no shipping logic change).
- Generated ignored artifacts: `test-artifacts/ton-010-stage-4.5c3a/*.png`.
- Evidence changed: this record and the matching desktop-implementer record.
- No SVG parity, SVG artifact, C3-B work, manual desktop click-through, or screen-reader validation is claimed.

## Invalidation conditions

Re-run C3-A if current preset fixtures, parser/candidate application, `Document.pattern_state` projection, preview composition, PNG rendering/encoding, Export Background, Preview Surface, output cache lifecycle, or artifact CLI routing changes.

## CACHE_UPDATE

4.5C3-A is complete pending parent review. `Polygon Six` and `Motif Ladder` now have focused current-format coverage proving preview and PNG share the same authoritative typed pattern output despite contradictory active and inactive adapters; Preview Surface and saved Export Background remain deliberately separate. Inspected preview/export artifacts are available under `test-artifacts/ton-010-stage-4.5c3a/`. SVG/C3-B and all later stages remain unstarted.
