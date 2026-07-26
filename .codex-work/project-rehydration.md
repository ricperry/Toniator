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
