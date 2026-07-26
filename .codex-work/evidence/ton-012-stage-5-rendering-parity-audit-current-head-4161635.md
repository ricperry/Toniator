# TON-012 Stage 5 rendering/parity audit

- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `4161635d90ee81421ffa1f2dc52e2a381d18c6d7`
- Scope: targeted read-only rendering, export, transition, persistence, and
  legacy-adapter audit; no source edits.
- Relevant working-tree assumptions: untracked `AGENTS.md` and
  `.codex-work/backups/`; no relevant source edits.

## Verified

- Document-facing Shapes and Curves rendering use resolved semantic pipeline
  fields. CMYK/RGB field resolution and blend routing are implemented.
- Preview renders transparent artwork then applies `DocumentAppearance`; PNG
  applies only `ExportBackground`; SVG uses semantic Shapes/Curves paths.
- Curves SVG Crosshatch labels still read legacy `settings.value_mode`, so a
  stale facade can produce incorrect layer labels. Labels must derive from
  `artwork_pipeline.assignment`.
- Output-treatment caches and `TreatmentState` preserve treatment/pipeline
  state and appearance is persisted in document v6 and included in undo/redo.

## Stage 5 decisions and risks

- A document-wide Preview Surface cannot satisfy independent CMYK/RGB cached
  values. Add appearance snapshots to the existing output-treatment caches,
  with CMYK white and RGB dark defaults, while retaining the active value in
  `DocumentAppearance` for the current model.
- Preview composition must not consult `ExportBackground`; export background
  remains explicit in raster/SVG export only.
- Keep legacy wrappers as explicitly bounded compatibility adapters unless a
  behavioral semantic boundary replaces them. Document-facing tests should
  exercise pipeline-authoritative entrypoints.
- Add semantic RGB Curves coverage with a deliberately stale legacy facade,
  CMYK/RGB Shapes and Curves preview/PNG/SVG parity coverage, appearance
  persistence/transition undo coverage, and authoritative Crosshatch labels.

## Relevant paths

- `src/render.rs`: `render_document_preview_cancellable`,
  `render_document_output_cancellable`, `composite_preview`,
  `legacy_pipeline_from_facade`, `generate_document_marks_cancellable`.
- `src/curve_render.rs`: `generate_curve_geometry_for_pipeline`, output
  renderer, legacy wrappers.
- `src/svg_export.rs`: `export_svg_cancellable`, `export_curve_svg`.
- `src/png_export.rs`: `png_bytes_cancellable` and background routing.
- `src/model.rs`: `DocumentAppearance`, `OutputTreatmentCache`,
  `switch_output_mode`, `TreatmentState`, persistence validation.
- `src/persistence.rs`: current v6 save/load.

## Unresolved

- No graphical runtime/creative review was performed by the explorer.
- The exact retained-adapter list must be finalized after implementation and
  focused parity tests.

## Producing audit

- Producing role: `codebase_explorer`
- Date: 2026-07-26
- Invalidate when inspected rendering, export, pipeline, model, persistence,
  or test files change, or when the proposed fixes land.
