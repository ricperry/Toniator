# TON-010 closeout handoff

Continue from branch `TON-010-Stage5-Framework-Restart`. Start by running
`git status --short --branch` and `git log -1 --oneline`; the latest pushed
commit is a preservation checkpoint, not TON-010 acceptance.

## Read first

1. `ISSUES.md`, especially **Pattern-engine preservation checkpoint audit —
   2026-08-02** under TON-010.
2. `docs/TON-010_STAGE_5_ARCHITECTURE_MAP.md`.
3. `docs/TON-010_STAGE_5_FRAMEWORK_RESTART.md`.
4. `.codex-work/evidence/ton-010-preservation-checkpoint-audit-2026-08-02.md`.
5. `.codex-work/cache-index.md`; treat older 2026-08-01 entries as historical
   implementation slices when they conflict with the checkpoint audit.

Preserve the checkpoint exactly. Do not reset/clean, rewrite history, alter
Weighted Voronoi placement/tessellation without a focused failure, or claim
manual acceptance that the user has not performed.

## Product boundary to enforce

The two concepts must remain separate:

- **Pattern definition:** reusable construction/topology such as grid, curve,
  math family, random dispersion, structural spacing, jitter algorithm,
  connection, and output geometry.
- **Channel instance:** per-ink colour, enabled state, opacity, coverage,
  sampling detail/density response, source weighting/influence, random seed,
  rotations, mark shape, and scale. These controls belong only in the main
  window.

`Document.pattern_state` is the semantic authority. Built-in and user patterns
must use one strict `.tnpattern` loader, typed graph, native-operation registry,
canonical runtime, editor, library, and persistence path.

## Primary architectural defect

Do not add another pattern-specific match arm. The current system is recipe
backed at runtime but still hard-coded at authoring time:

- `src/ui.rs::PATTERN_PRESET_LABELS` fixes the visible catalog by index.
- `src/ui.rs::apply_named_pattern_preset` injects special defaults for each
  option.
- `src/ui.rs::pattern_editor_recipe` mutates a Shapes-compatible graph with
  fixed node IDs, operation IDs, and parameter names.
- `src/shapes_native.rs` implements `shapes.lattice-placement-editor` as one
  large grid/triangular/curve/math/random branch.
- Pattern Save As writes a user file, but the UI cannot browse, import, reload,
  or resolve it later.

The next implementation should expose useful typed placement, path,
dispersion, deformation, modulation, and output operations and generate the
guided controls from recipe/schema metadata. The graph view must edit the same
authoritative graph; switching views cannot rewrite or discard it.

## Disciplined closeout sequence

Use one primary writer and stop at independently verifiable substages. At each
boundary, parent-review the diff, run focused tests, and update the evidence
cache before handing off the next substage.

1. Write and test an explicit ownership table for every current control.
   Remove/rename pattern-editor versus channel-control leaks without changing
   canonical output.
2. Replace numeric preset mutation with immutable bundled `.tnpattern`
   definitions or declarative templates selected by stable ID. Triangular,
   Wave Line Field, and Evenly Spaced Pointillism must be genuine proof recipes,
   not aliases for private Rust branches.
3. Decompose or formally expose the Shapes editor operation as composable,
   typed operations. All meaningful parameters must be user-editable; irrelevant
   controls may be disabled rather than hidden.
4. Implement schema-driven Guided editing and the non-lossy Graph editor over
   the same local draft. Cancel discards; Apply creates one undo step; Save As
   writes and selects a user definition.
5. Wire the XDG user library, import, duplicate/conflict handling, layered
   bundled/user/project resolution, custom definition/asset embedding, and
   actionable missing-definition recovery.
6. Remove `RenderVariant`/NativeBasic/typed compatibility authority only after
   conversion and canonical preview/PNG/SVG equivalence tests pass.
7. Run the full automated matrix and then leave the exact GNOME/Wayland,
   Krita-reference, Inkscape Break Apart, accessibility, and creative-workflow
   checklist for explicit human acceptance.

## Current baseline and open risks

The last recorded complete automated baseline is 261 library tests and 56
binary/UI tests plus format, strict Clippy, locked release, doc tests, and a
realized GTK regression. Re-run it on the checkpoint before editing.

Open risks include hard-coded preset behavior, write-only Save As, absence of
the graph editor and application-level registry, remaining compatibility
dispatch, incomplete Stage 6 proof semantics, and unperformed human acceptance.
TON-010 stays Open until those items are completed and verified.
