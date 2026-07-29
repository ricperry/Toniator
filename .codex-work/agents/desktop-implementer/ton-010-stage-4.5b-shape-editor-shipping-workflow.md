# TON-010 Stage 4.5B — shipping shape-editor workflow

- Recorded: 2026-07-28
- Git HEAD: `f9c138c493a9d687b5300abddf14e78281f2ad63`
- Worktree assumption: intentionally dirty Stage 1–4, TON-013, preset, fixture,
  evidence, documentation, Blueprint/GResource, and tracker changes were present
  before this substage. They were preserved; no reset, clean, commit, push, or
  deletion was performed.
- Scope boundary: Stage 4.5B only. No Stage 4.5C/4.5D, presets, fixtures,
  persistence schemas, or Weighted Voronoi work was started.

## Files changed by this substage

- `src/ui.rs`
- `test-artifacts/ton-010-stage-4.5b/active-editor-wide.png` (ignored generated artifact)
- `test-artifacts/ton-010-stage-4.5b/completed-edit-preview.png` (ignored generated artifact)
- `test-artifacts/ton-010-stage-4.5b/shapes-entry-wide.png` (ignored generated artifact)
- `test-artifacts/ton-010-stage-4.5b/shapes-entry-narrow.png` (ignored generated artifact)
- This evidence entry.

## Implementation and authority decisions

- Reused the shipping `resources/toniator-window.blp` → Blueprint build script →
  GResource → `gtk::Builder::from_resource` path already used by `AppUi`; no
  duplicate static widget or test-only replacement control was introduced.
- Kept `open_shape_editor` on the existing authority path:
  `Document.pattern_state.shape_settings()` for reads and
  `DocumentEditor::set_shape_settings` through `change_web_treatment` for writes.
  Crosshatch remains derived through `document_uses_crosshatch`/`artwork_pipeline`.
  No `RenderVariant` is used for shape selector or parameter authority.
- Corrected a verified focus defect: the focusable canvas was losing focus during
  modal map/default-focus assignment. The dialog now assigns the canvas as its
  `GtkWindow` focus child in `connect_map`; the realized test checks the dialog's
  focused widget instead of the compositor-dependent `has_focus` property.
- Added explicit accessible name, description, and tooltip to the focusable shape
  canvas. Existing schema-driven entry label/help remain untouched.
- `--show-controls` artifact mode now collapses Source/Output and expands
  Treatment Settings before realization, so evidence captures expose the requested
  Shapes entry without changing normal interactive defaults.
- Preserved the protected editor algorithms and local edit state: no changes to
  click geometry, anchor/handle movement, insertion/deletion, keyboard edits,
  validation, or Done/Cancel commits.

## Coverage and checks

- Added `verify_realized_resource_shape_editor_authority_workflow`, executed from
  `realized_numeric_controls_leave_continuous_scroll_to_parent`. It creates a real
  `AppUi` from the shipping resource, selects Shapes, installs the existing curved
  fixture, overwrites only the transient adapter with contradictory shape data, and
  proves the resource-backed entry remains discoverable and opens the dialog.
- The same regression finds the production drawing area and `GestureClick`, performs
  a real double-click insertion, clicks Done, verifies authoritative persistence,
  undo/redo, reopens and clicks Cancel, then verifies ordinary Shapes workflow
  returns unchanged. It also allocates the shipping `OverlaySplitView` through its
  760px narrow branch and verifies collapsed/sidebar-toggle behavior.
- Commands passed:
  - `blueprint-compiler lint -r syntax resources/toniator-window.blp resources/toniator-channel-controls.blp resources/toniator-aggregate-channel-controls.blp`
  - `cargo test --locked` — 138 library + 46 binary tests passed.
  - `cargo check --locked --all-targets`
  - `cargo clippy --locked --all-targets -- -D warnings`
  - `cargo fmt --all -- --check`
  - `git diff --check`
  - Focused realized GTK run: `cargo test --locked realized_numeric_controls_leave_continuous_scroll_to_parent -- --nocapture`.

## Visual artifacts inspected

- `test-artifacts/ton-010-stage-4.5b/shapes-entry-wide.png` — 1280×820. Shipping
  Blueprint Shapes panel with User Defined selected and the Edit User-Defined Mark
  entry alongside the rendered preview.
- `test-artifacts/ton-010-stage-4.5b/active-editor-wide.png` — 560×620. Actual
  dialog capture with cubic outline, anchors, independent handle lines/points,
  instructional text, and Cancel/Done.
- `test-artifacts/ton-010-stage-4.5b/completed-edit-preview.png` — 1000×760.
  Shipping window shows the User Defined control state and rendered custom-mark
  preview. The realized test supplies the separate Done/undo/redo persistence proof.
- `test-artifacts/ton-010-stage-4.5b/shapes-entry-narrow.png` — 960×820. Launched
  with requested 720px artifact width; the current top-level minimum resolved it to
  960px, but the capture still shows the entry and preview. The 760px shipping
  `OverlaySplitView` narrow branch is covered in the realized GTK regression.

## Limitations and follow-up review targets

- No human manual acceptance was performed. Fedora/GNOME Wayland keyboard and
  pointer click-through (including physical focus traversal, handle drag, Delete,
  arrows, Escape, and narrow overlay at a compositor-resizable top-level size) still
  need manual Stage 4.5D review.
- The artifact runner's 720px requested top-level window is clamped to 960px by the
  current fit-artboard/window minimum; do not treat its screenshot as proof of an
  actual 720px top-level surface. The resource-backed narrow branch is automated,
  but any change to canvas sizing, `InspectorPaneController`, Blueprint layout, or
  GTK/Adwaita version invalidates that conclusion.
- Durable docs likely affected after parent acceptance: the Stage 4.5 baseline/
  workflow documentation and evidence index, not this substage's source work.
- Invalidate this evidence on changes to `src/ui.rs`, `resources/*.blp`, `build.rs`,
  GResource manifest/build path, model authority APIs, GTK/Adwaita version, Git HEAD,
  or relevant dirty worktree state.
