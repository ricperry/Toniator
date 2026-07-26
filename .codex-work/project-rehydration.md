# Toniator project rehydration summary

Use this file as a compact map to help a later thread resume work without
repeating broad repository discovery. It contains no findings by default.

When populated, add one subsection per subsystem with:

- Current cache-entry links and their validation status
- Relevant implementation paths and symbols
- Verified facts, clearly separated from inferences
- Open questions, artifacts, and the next smallest verification step
- Git HEAD and dirty-file assumptions

Always validate the summary against current repository contents; invalidate or
replace entries whose relevant files or assumptions changed.

## TON-012 artwork pipeline — Stage 1B

- Cache entry: `evidence/ton-012-stage-1b-complete.md` — valid for HEAD
  `32022df` while the listed source files and relevant working-tree assumptions
  remain unchanged.
- Verified path: `src/artwork_pipeline.rs`, `src/model.rs`,
  `src/persistence.rs`, `src/preset.rs`, `src/render.rs`, `src/svg_export.rs`,
  and inline test modules.
- Verified state: `Document.artwork_pipeline` is authoritative; project v6 and
  preset v3 persist validated stable IDs; active, saved, and inactive treatment
  snapshots retain paired pipeline state; legacy render/GTK fields are
  projections. Stage 1B is complete; later resolved-field and parity stages
  remain.
- Verification: `cargo fmt --check`; `cargo test --locked` — 93 library tests
  and 44 binary tests passed.

## TON-012 artwork pipeline — Stage 3

- Cache entries: `agents/desktop-implementer/ton-012-stage-3-implementation.md`,
  `agents/desktop-implementer/ton-012-stage-3-correction-implementation.md`,
  `evidence/ton-012-stage-3-review-current-head.md`,
  `evidence/ton-012-stage-3-creative-usability-review.md`, and
  `agents/documentation-maintainer/ton-012-stage-3-documentation-reconciliation.md`.
  These are valid for the current uncommitted Stage 3 files based on HEAD
  `bac55f7` and must be revalidated after a commit or source change.
- Verified UI ownership: `src/ui.rs` shared Document controls and direct
  callbacks own Artwork Source, Source Alpha, Output Model, scalar assignment,
  semantic Active Channel, and temporary Crosshatch action. `src/model.rs`
  `DocumentEditor::set_artwork_pipeline`, `switch_output_mode`, and
  `exit_crosshatch_treatment` provide the authoritative mutation/cache seams.
- Verified invariants: source and output remain independent; Full Color uses
  model-specific automatic separation; scalar assignment uses semantic channel
  IDs; RGB cannot retain Black and CMYK cannot retain RGB channels; Alpha hides
  Source Alpha; UI refresh splices installed StringLists and rejects transient
  invalid positions; each semantic edit is one undo operation.
- Verification: 103 library tests, 43 binary tests, strict Clippy, locked
  release build, desktop/AppStream validation, `cargo fmt --check`,
  `git diff --check`, and a current automated GTK screenshot. Human manual
  click-through remains pending.
- Remaining scope: Stage 4 preset cleanup, final RGB Curves completion, broad
  preview/export parity, and manual creative-workflow verification. TON-012
  remains In Progress.

## TON-012 artwork pipeline — Stage 4 preset boundary

- Cache entry: `evidence/ton-012-stage-4-preset-ownership-current-head.md` —
  valid for HEAD `236cdb1` while preset, model, UI, bundled assets, and the
  listed dirty-file assumptions remain unchanged.
- Verified path: `src/preset.rs`, `src/model.rs`, `src/artwork_pipeline.rs`,
  `src/ui.rs`, `assets/presets/*.tntr`.
- Verified state: current v3 is one unscoped treatment document; semantic
  pipeline IDs are authoritative; save/load call paths and undo/cache seams
  are identified; four runtime bundled presets exist; renderer compatibility
  fields remain active adapters.
- Implementation entry: `agents/desktop-implementer/ton-012-stage-4-implementation.md`
  records the v4 scoped `.tntr` representation, atomic editor application,
  four converted bundled presets, UI scope chooser, tests, and retained
  compatibility adapters.
- Review entry: `evidence/ton-012-stage-4-preset-review-236cdb1.md` records the
  independent review; its nested unknown-field and treatment/channel isolation
  findings were corrected and revalidated in the implementation evidence.
- Stage 4 gate: user-confirmed manual smoke exited status `0`; Stage 4 is
  complete. Final RGB Curves and broad preview/export parity remain outside
  this stage.
- Stage 5 defect: RGB Screen to CMYK Print can retain RGB's black Preview
  Surface instead of CMYK's white default. Stage 5 must give each output model
  independent cached/default Preview Surface state and verify switching,
  save/reopen, preset application, and PNG/SVG export isolation from Preview
  Surface and Export Background.
- Creative review entry: `evidence/ton-012-stage-4-creative-preset-review-236cdb1.md`;
  all four bundled names/output pairs were judged coherent, with no creative
  correction required.
