# TON-010 Stage 4, Substage 4C2a — parent review

- Recorded: 2026-07-28
- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `f9c138c493a9d687b5300abddf14e78281f2ad63`
- Producing writer: `desktop_implementer` / `019faa4b-ec1b-7403-96b0-d16f31cb38ed`
- Parent reviewer: orchestrator

## Boundary and deliverable

4C2a migrated Curves scalar, layout, color, and visibility synchronization and
callbacks to authoritative `Document.pattern_state.curve_settings()`. Writes
use `DocumentEditor::set_curve_settings`; Crosshatch/output semantics use
`Document.artwork_pipeline`. The writer stopped before direct curve-editor,
motif-overlay, artboard-size, and editing-context reads, which are the explicit
4C2b boundary.

## Parent review

Reviewed the writer report and the current `src/ui.rs` paths. The migrated
callbacks clone and update authoritative Curve settings, while the remaining
`RenderVariant::WebCurveV1` reads are confined to the declared 4C2b helpers.
The realized GTK contradiction test installs different authoritative and
transient Curve values and verifies that control synchronization and scalar
editing follow the authoritative values.

## Verification

- Parent rerun: `cargo test --locked --lib` — 138 passed.
- Writer validation: full `cargo test --locked` — 138 library and 46 binary/UI
  tests passed; all-targets check and clippy, fmt check, and `git diff --check`
  passed.
- No manual GTK/Wayland visual acceptance or screenshot is claimed.

## Safe handoff

Accepted as the 4C2a progress boundary. Continue with the same writer for
4C2b only: migrate the remaining direct Curves editor, motif, context, and
related adapter parameter reads/writes, then stop and report. No Shapes,
schema/preset, custom-pattern ecosystem, or Weighted Voronoi work belongs in
that handoff.

