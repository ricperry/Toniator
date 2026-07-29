# TON-010 Stage 4.5B — parent review and manual-inspection pause

- Recorded: 2026-07-28
- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `f9c138c493a9d687b5300abddf14e78281f2ad63`
- Producing writer: `desktop_implementer` / `019faa4b-ec1b-7403-96b0-d16f31cb38ed`
- Parent reviewer: orchestrator
- Scope: shipping shape-editor workflow only; 4.5C/4.5D and Stage 5 not started

## Corrections

The existing shape-editor algorithms and local state were preserved. The
shipping Blueprint → GResource → `gtk::Builder::from_resource` path was
verified through `AppUi`, not a separately constructed test widget. Two
specific issues were corrected:

- on modal map, the editor canvas could lose focus; the dialog now assigns the
  focusable canvas as its focus child;
- the canvas lacked explicit accessible name, description, and tooltip; these
  now describe anchors, Bézier handles, insertion, deletion, arrow movement,
  and Escape cancellation.

The entry control remains the existing Blueprint `web_edit_shape` button. No
duplicate control or shape algorithm rewrite was introduced. Shape reads use
`pattern_state.shape_settings()` and writes use `set_shape_settings`; the
transient adapter is used only as a contradictory test fixture.

## Realized GTK coverage

`verify_realized_resource_shape_editor_authority_workflow`, run from the
existing realized GTK test, uses the shipping resource and verifies:

- entry control discovery, visibility, sensitivity, focusability, registered
  descriptor label, and production callback activation;
- actual dialog canvas, click controller, focus transfer, and accessibility
  metadata;
- representative production double-click insertion;
- Done commit, authoritative persistence, undo, redo, reopen, and Cancel;
- return to the ordinary Shapes workflow;
- the shipping narrow `OverlaySplitView` branch, collapsed/sidebar visibility,
  and controls-toggle restoration.

## Visual artifacts inspected

- [Shapes entry wide](/home/ricperry1/projects/Toniator/test-artifacts/ton-010-stage-4.5b/shapes-entry-wide.png), 1280×820 — visible Shapes controls, User Defined mark, and existing entry point.
- [Active editor](/home/ricperry1/projects/Toniator/test-artifacts/ton-010-stage-4.5b/active-editor-wide.png), 560×620 — actual editor with anchors, independent handles, instructions, Cancel, and Done.
- [Narrow entry](/home/ricperry1/projects/Toniator/test-artifacts/ton-010-stage-4.5b/shapes-entry-narrow.png), 960×820 — entry and preview at the artifact runner’s clamped width; 760px shipping narrow branch is covered by GTK test.
- [Completed edit preview](/home/ricperry1/projects/Toniator/test-artifacts/ton-010-stage-4.5b/completed-edit-preview.png), 1000×760 — completed User Defined mark reflected in the rendered preview.

The parent inspected all four images. No human GNOME/Wayland click-through is
claimed; physical pointer/keyboard behavior and compositor-resized narrow
layout remain for the user’s manual review.

## Verification

- Parent focused run: `cargo test --locked --bin toniator ui::tests::realized_numeric_controls_leave_continuous_scroll_to_parent -- --exact --nocapture` — passed, including the shipping resource workflow.
- Parent full run: `cargo test --locked` — 138 library, 46 binary/UI, and 0 doc tests failed; all passed.
- Parent checks: Blueprint lint for all three `.blp` resources, all-targets check, strict Clippy, format check, and `git diff --check` — passed.
- Writer generated and inspected the four artifacts above.

## Gate status

Accepted by the parent as complete-for-review. Pause now for user manual
inspection and feedback. Do not begin 4.5C until explicit user direction.

