# TON-010 Stage 4.5C2 — accepted authoring correction

- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `f9c138c` with the TON-010 worktree dirty
- Date: 2026-07-28
- Parent decision: the user accepted the current export-background behavior and
  organization and authorized progression to 4.5C2B.

## Accepted boundary

The saved `Document.appearance.export_background` is authoritative. `None` is
explicit transparent output; `Color` exposes the saved RGBA value and can be
chosen through the shipping Output/Appearance controls. Preview Surface remains
separate from export background. The correction does not claim that the Output
section's organization is final UX; it closes the bounded C2 correction and
does not alter the existing visual/export contract.

## Next handoff

4.5C2B-1 is active. The writer must add deliberately contradictory transient
Shapes/Curves adapter coverage through the production document/render,
persistence/history, and UI paths, then stop for parent review. CMYK/RGB
transition work is a separate follow-up handoff and has not begun.

## Invalidation

Invalidate this acceptance record if appearance authority, export composition,
Preview Surface separation, current-format persistence, or the 4.5C sequence
changes.
