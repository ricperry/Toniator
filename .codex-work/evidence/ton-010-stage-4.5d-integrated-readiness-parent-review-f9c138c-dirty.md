# TON-010 Stage 4.5D — integrated readiness review

- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `f9c138c` with the TON-010 worktree dirty
- Date: 2026-07-28
- Parent result: 4.5 complete; Stage 5 remains explicitly gated and was not
  started.

## Reconciled gates

The parent reviewed the evidence for every 4.5 boundary:

- 4.5A historical audit and demonstrability matrix;
- 4.5B restored realized shape-editor workflow and Regular Polygon side count;
- C1 current-format `Polygon Six` and `Motif Ladder` fixtures;
- C2A save/reopen and authoritative undo/redo;
- C2B-1 contradictory Shapes/Curves adapter authority;
- C2B2-A CMYK/RGB inactive-cache authority;
- C2B2-B realized Blueprint/GResource CMYK/RGB workflow;
- C3-A preview/PNG parity artifacts;
- C3-B SVG parity and editable artifacts.

## Integrated readiness findings

`Document.pattern_state` is the sole persisted pattern selector/parameter
authority. Rendering, preview, PNG, SVG, persistence, history, CMYK/RGB cache
transitions, and the shipping AppUi controls project from it. `RenderVariant`,
inactive cache `render` fields, and saved transition facades remain narrowly
bounded derived adapters; deliberately contradictory values are covered by
regression tests and cannot select or replace authoritative pattern state.

Preview Surface remains preview-only. Export Background remains document-wide
export presentation state. Current-format document/preset schemas are strict;
obsolete schemas are rejected and no migration or obsolete-format opening path
was added.

## Artifacts reviewed

- Shape-editor artifacts under `test-artifacts/ton-010-stage-4.5b/`;
- preview/PNG artifacts under `test-artifacts/ton-010-stage-4.5c3a/`;
- SVG artifacts under `test-artifacts/ton-010-stage-4.5c3b/`.

The parent visually inspected the C3-A preview/PNG images and independently
rasterized and visually inspected the C3-B SVG outputs. Polygon Six shows the
distinct six-sided dot treatment; Motif Ladder shows the distinct repeated wave
motif. SVG output retains editable semantic layers, path IDs, curves, and
artboard clipping.

## Final validation

- `cargo test --locked` — passed: 146 library, 48 binary/UI, 0 doc tests;
- focused C3-A preview/PNG parity test — passed;
- focused C3-B SVG parity test — passed;
- `cargo check --locked --all-targets` — passed;
- `cargo clippy --locked --all-targets -- -D warnings` — passed;
- `cargo fmt --all -- --check` — passed;
- `git diff --check` — passed.

## Delegation blocker

The C3-B writer hit an external usage-limit blocker after leaving valid test
and artifact work in the shared worktree. The parent preserved, inspected,
validated, documented, and integrated that work; no overlapping reassignment
or discarded implementation occurred.

## Remaining limitations and Stage 5 gate

No human GNOME/Wayland click-through or screen-reader acceptance is claimed;
realized GTK regression coverage and artifact inspection are complete. This is
recorded as a limitation for the next manual gate, not as a reason to preserve
obsolete compatibility or begin Weighted Voronoi early.

Stage 5 Weighted Voronoi is the next step and remains untouched pending the
user's explicit approval.
