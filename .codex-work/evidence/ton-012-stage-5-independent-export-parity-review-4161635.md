# TON-012 Stage 5 independent export/parity review

- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `4161635d90ee81421ffa1f2dc52e2a381d18c6d7`
- Scope: read-only review of the Stage 5 implementation diff and focused
  export/transition tests.
- Reviewer: `test_reviewer`

## Major finding

RGB-output Crosshatch has a preview/PNG versus SVG compositing mismatch.
`render_curve_geometry_output_cancellable` uses Multiply for its K/C/M/Y
compatibility layers because they are non-RGB `Ink` values, while
`export_curve_svg` selects SVG Screen from the overall RGB output model. The
SVG blend rule must follow the Crosshatch compatibility assignment so all
three paths agree.

## Verified passing

- Per-output Preview Surface defaults, cache restoration, and one-step
  undo/redo.
- Preview Surface is excluded from PNG/SVG; Export Background remains export
  only.
- Semantic document Shapes/Curves paths and authoritative Crosshatch labels.
- Stale-facade RGB SVG channel selection contains only Red/Green/Blue.
- No GtkBuilder/Cambalache, TON-009, TON-010, or TON-011 work was introduced.

## Minor follow-up gaps

- No direct RGB Curves preview/PNG no-Black assertion.
- Persistence does not behaviorally switch after reload in the focused test.
- Legacy-v6 fallback covers missing CMYK but not missing RGB snapshots.
- No mid-generation Curves cancellation integration test.

## Review commands/evidence

- Focused output-mode, preview/export, persistence, PNG, Crosshatch-label, and
  RGB-Curves SVG tests passed.
- `git diff --check` passed.
- No graphical/manual verification was performed.

Invalidate after changes to rendering, SVG/PNG export, Crosshatch assignment,
output caches, persistence, cancellation, or relevant tests.
