# TON-012 Stage 4 preset ownership evidence

- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `236cdb190a091029c1e7436d65716bf889b31010`
- Relevant working-tree assumptions: only untracked `AGENTS.md` and
  `.codex-work/backups/`; no relevant source edits.
- Producing agent: `codebase_explorer`, read-only targeted preset audit,
  2026-07-26.

## Verified ownership

- `Document.artwork_pipeline` is the semantic authority for Artwork Source,
  Source Alpha Policy, Output Model, Channel Assignment, and active semantic
  channel. `ArtworkPipelineSettings` already serializes stable dotted IDs.
- Treatment state is `Settings`, `WebShapeSettings`, or `WebCurveSettings`;
  per-channel geometry and appearance live in the treatment settings.
- Saved Shapes/Curves snapshots and inactive CMYK/RGB caches are paired with
  their semantic pipeline state. `DocumentEditor::TreatmentState` snapshots
  all of this for undo/redo.
- Save path: `ui::save_treatment_dialog` ->
  `preset::document_treatment_preset_bytes` -> canonicalize, normalize,
  validate, serialize v3, atomic write.
- Load path: `ui::import_preset_source` -> `preset::parse_treatment` in a
  worker -> candidate validation -> `DocumentEditor::set_treatment_with_pipeline`
  on the GTK thread -> one editor edit, control sync, one preview request.

## Stage 4 findings

- The current format is one unscoped treatment document containing `render`,
  optional native `settings`, and `artwork_pipeline`; scope must become
  explicit and must control application.
- The four runtime bundled presets are already schema v3 and use semantic
  pipeline IDs, but still contain renderer compatibility fields such as
  `value_mode` and `single_channel`, which parsing overwrites from the
  authoritative pipeline. Archived copies under `archive/webapp/presets/`
  are not runtime inputs.
- The remaining `ValueMode`, `output_mode`, projection functions, and
  Crosshatch assignment are active renderer compatibility adapters and must
  not be removed in a preset-only stage.
- Existing gaps are scope isolation, atomic failed-load behavior, exhaustive
  semantic channel/source/output coverage, cache pairing, and canonical
  bundled-preset assertions.

## Stage 4 design assumption

Use one explicit versioned `.tntr` document with a `scope` field and optional
pipeline, treatment, channel, and complete-workflow sections. Keep the
renderer compatibility facade internal to the treatment section until later
render/parity stages consume resolved fields directly. Do not migrate older
pre-release versions.

- Invalidation: changes to preset schema, `ArtworkPipelineSettings`,
  `DocumentEditor`, projection functions, bundled presets, or UI load/save
  paths.
