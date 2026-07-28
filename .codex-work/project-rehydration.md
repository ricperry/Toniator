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

## TON-012 artwork pipeline — Stage 5 implementation boundary

- Cache entries: `evidence/ton-012-stage-5-rendering-parity-audit-current-head-4161635.md`,
  `agents/desktop-implementer/ton-012-stage-5-rendering-parity-implementation.md`,
  `evidence/ton-012-stage-5-independent-export-parity-review-4161635.md`, and
  `agents/desktop-implementer/ton-012-stage-5-rgb-crosshatch-svg-correction.md`,
  and `evidence/ton-012-stage-5-artifact-creative-output-review-4161635.md`;
  validated against the current dirty worktree based on HEAD `4161635` on
  2026-07-26.
- Verified state: active `DocumentAppearance.preview_surface` is restored from
  an optional per-output snapshot in `OutputTreatmentCache`, with CMYK white
  and RGB black defaults. Export Background remains explicit and export-only.
  Document-facing Shapes/Curves rendering consumes semantic pipeline fields;
  RGB Curves contain Red/Green/Blue only. Crosshatch remains the temporary
  progressive K/C/M/Y compatibility treatment and uses Multiply consistently
  across raster preview/PNG and SVG, including RGB-output documents.
- Retained adapters: legacy renderer entrypoints that accept facade settings;
  document render/export paths use pipeline-authoritative functions. No
  obsolete format compatibility or future TON issue work was added.
- Verification: 117 library tests, 43 binary tests, strict Clippy with all
  features, locked release build, formatting, diff check, desktop/AppStream
  validation, and no coredumps. Artifacts are under ignored
  `test-artifacts/ton-012-stage5/`. Manual graphical verification remains the
  user acceptance gate.
- Final state: the user accepted the manual Stage 5 gate in the closeout
  request; TON-012 is ready for its closeout commit/PR workflow. The next
  planned issue is TON-014 Source-Sampled Mark Colors. Do not begin it here.

## TON-012 final closeout

- Evidence: `evidence/ton-012-closeout-4161635.md`.
- Tracker state: TON-012 Complete; TON-014 Source-Sampled Mark Colors Planned;
  TON-013 GtkBuilder/Cambalache remains Planned.
- Closeout state: final verification passed, the user accepted Stage 5, and
  the feature branch is ready to commit and publish through the authorized PR
  workflow. Preserve unrelated untracked `AGENTS.md` and `.codex-work/backups/`.

## TON-013 GtkBuilder migration — Stage 1

- Current checkout: HEAD `546ea4c`; dirty implementation files are `src/ui.rs`,
  `ISSUES.md`, `.codex-work/cache-index.md`, plus new `resources/`, `docs/`,
  and TON-013 evidence files; preserve unrelated `.codex-work/backups/`.
- Cache entries: `evidence/ton-013-gtkbuilder-migration-seams-546ea4c.md`,
  `agents/desktop-implementer/ton-013-stage-1-gtkbuilder-shell-implementation.md`,
  `evidence/ton-013-stage-1-independent-shell-ux-546ea4c-dirty.md`, and
  `agents/documentation-maintainer/ton-013-stage-1-documentation-reconciliation.md`.
- Verified path: `resources/ui/Toniator.ui` owns the static
  `AdwApplicationWindow` / `AdwToolbarView` / `AdwHeaderBar` /
  `AdwToastOverlay` / `GtkStack` shell. `src/ui.rs::build_top_level_shell`
  loads it with `include_str!` and `gtk::Builder::from_string`, retrieves stable
  IDs, and leaves `build_start_view`, `build_editor_view`, dynamic models,
  callbacks, drawing, dialogs, and synchronization in Rust.
- Verified stable IDs: `main_window`, `main_toolbar_view`, `main_header_bar`,
  `toast_overlay`, `main_stack`, `window_title`, all header actions, and
  `controls_toggle`; Rust inserts `start` and `editor` pages into the live
  Builder-owned stack.
- Verification: `cargo test --locked` passed with 117 library and 44 binary/UI
  tests; strict Clippy, release build, `cargo fmt --check`, XML parse, diff
  check, and real GTK 900x680 screenshot smoke passed. Cambalache 1.0.3 is
  installed, but was not launched; no round-trip claim is made.
- Status: TON-013 is `In Progress`. Editor hierarchy, selector-driven controls,
  formal resource packaging/GResource choice, and reusable channel composite
  remain open. Do not call TON-013 complete from this stage alone.

## TON-013 GtkBuilder migration — Stage 2

- Current checkout: HEAD `546ea4c`; preserve modified `src/ui.rs`, `ISSUES.md`,
  `.codex-work/`, new `docs/`, new `resources/`, and unrelated
  `.codex-work/backups/`.
- Cache entries: `agents/desktop-implementer/ton-013-stage-2-channel-inspector-implementation.md`,
  `agents/desktop-implementer/ton-013-stage-2-treatment-scope-correction.md`,
  `agents/documentation-maintainer/ton-013-stage-2-documentation-reconciliation.md`,
  `evidence/ton-013-stage-2-treatment-scope-final-ux-546ea4c-dirty.md`, plus
  the earlier Stage 2 architecture/UX review records.
- Verified resources: `ToniatorInspector.ui` owns actual top inspector order
  Source -> Output -> Channel Settings -> Appearance -> Treatment;
  `ToniatorChannelControls.ui` is instantiated/cached once per real semantic
  CMYK/RGB `OutputChannelId`; `ToniatorAggregateChannelControls.ui` is a
  separate aggregate status/context composite. `Toniator.cmb` hashes all four
  current UI files, including the unchanged main `Toniator.ui`.
- Verified semantics: Output `Channel Assignment` and conditional `Active
  Channel` remain sole scalar-routing controls. `Treatment Editing Scope` is
  the sole visible treatment-recipient selector and synchronizes Shapes/Curves
  target models without mutating pipeline assignment. Legacy Adjust Ink/Channel
  rows remain hidden compatibility internals.
- Verification: `cargo test --locked` passed with 117 library and 45 binary/UI
  tests; strict Clippy, locked release build, formatting, XML/CMB parsing, and
  diff checks passed. `test-artifacts/ton-013/stage2-treatment-scope-correction.png`
  (1000x760) was visually inspected. Separate normal/narrow captures were
  attempted but the Wayland compositor timed out without a GTK render node.
- Status: TON-013 remains `In Progress`. Substantial treatment-specific editor
  layout, custom drawing, dialogs, and dynamic content remain Rust-built.
  Assistive-technology/focus-order and narrow-window checks remain follow-up
  verification, not a completion claim.

## TON-013 GtkBuilder migration — control exposure correction

- Cache entries: `agents/desktop-implementer/ton-013-control-exposure-stage-implementation.md`,
  `agents/desktop-implementer/ton-013-control-exposure-stage-correction.md`,
  `agents/documentation-maintainer/ton-013-control-exposure-documentation-reconciliation.md`,
  `evidence/ton-013-control-exposure-independent-ux-546ea4c-dirty.md`, and
  `evidence/ton-013-control-exposure-inventory-546ea4c-dirty.md`.
- Verified resource: `resources/ui/ToniatorEditorControls.ui` is registered
  in `resources/ui/Toniator.cmb` and owns the practical static Source, Output,
  Appearance, treatment chrome, and Basic/native Sampling Detail, Coverage,
  Contrast, and Screen Angle row/scale structure. `src/ui.rs` attaches live
  dropdown models, scale adjustments/precision entries, callbacks,
  visibility/sensitivity, dynamic Shapes/Curves/Motif detail rows, mixed-value
  and help content, custom drawing/dialogs, and synchronization.
- Verified accessibility: Artwork Source, Source Alpha, Output Model, Channel
  Assignment, and Active Channel use explicit Builder label relations; focused
  realized GTK coverage passes. Output routing and Treatment Editing Scope
  semantics remain unchanged, and aggregate controls remain separate from the
  reusable real-channel template.
- Verification: `cargo fmt --check`, `cargo test --locked` (117 library and
  46 binary/UI tests), strict Clippy, locked release build, XML/CMB parsing,
  `git diff --check`, and
  `test-artifacts/ton-013/control-exposure-stage-corrected.png` at 1000x980
  passed; the corrected screenshot was visually inspected.
- Status: TON-013 remains `In Progress`. Shapes, Curves, and Motif detail rows,
  dynamic help/mixed-value content, custom DrawingAreas, dialogs, and runtime
  synchronization remain Rust-built. Cambalache 1.0.3 is installed, but no
  round-trip edit was performed; narrow and assistive-technology checks remain
  follow-up verification.
