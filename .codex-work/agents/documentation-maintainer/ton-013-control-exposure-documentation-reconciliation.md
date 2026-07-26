# TON-013 control-exposure documentation reconciliation

- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `546ea4c5eb1fec8e91c2b307545e33e42331e308`
- Timestamp: `2026-07-26`
- Producing agent: `documentation-maintainer`
- Task: reconcile durable documentation after the verified TON-013
  control-exposure correction without changing application behavior or UI
  resources.

## Documentation files reviewed

- `docs/UI_ARCHITECTURE.md`
- `ISSUES.md` (TON-013 section)
- `.codex-work/project-rehydration.md`
- `.codex-work/cache-index.md`
- Existing Stage 1/Stage 2 documentation-maintainer records and the current
  documentation structure (`README.md`, `docs/`); no new durable product-doc
  location was needed.

## Documentation files changed

- `docs/UI_ARCHITECTURE.md`
- `ISSUES.md` (TON-013 control-exposure progress and verification)
- `.codex-work/project-rehydration.md`
- `.codex-work/cache-index.md`
- This evidence record

## Implementation evidence used

- `src/ui.rs`: `EDITOR_CONTROLS_UI`, `build_editor_view`, Builder scale
  retrieval/configuration, five dropdown accessibility bindings, live models,
  callbacks, visibility/sensitivity, dynamic Shapes/Curves/Motif rows,
  mixed/help content, custom drawing/dialogs, and synchronization.
- `resources/ui/ToniatorEditorControls.ui`: Builder-owned Source, Output,
  Appearance, treatment chrome, and Basic/native Sampling Detail, Coverage,
  Contrast, and Screen Angle row/scale shells.
- `resources/ui/Toniator.cmb`: Cambalache 1.0.3 project metadata and current
  UI hashes; no round-trip was performed.
- `.codex-work/agents/desktop-implementer/ton-013-control-exposure-stage-implementation.md`
  and `ton-013-control-exposure-stage-correction.md`.
- `.codex-work/evidence/ton-013-control-exposure-inventory-546ea4c-dirty.md`
  and `ton-013-control-exposure-independent-ux-546ea4c-dirty.md`.
- `test-artifacts/ton-013/control-exposure-stage-corrected.png`, verified as a
  1000x980 RGBA PNG and recorded as visually inspected by the correction
  evidence.

## Verified reconciliation

- Builder owns Source, Output, Appearance, treatment chrome, and the
  Basic/native Sampling Detail, Coverage, Contrast, and Screen Angle row/scale
  shells.
- Rust owns live models, callbacks, visibility/sensitivity, dynamic
  Shapes/Curves/Motif detail rows, mixed/help content, custom drawing/dialogs,
  and synchronization.
- `Treatment Editing Scope` remains the treatment-recipient selector, while
  Output routing remains authoritative through `Channel Assignment` and
  conditional `Active Channel`; aggregate All Inks/All Channels context remains
  distinct from real semantic channels.
- Current recorded checks passed: `cargo fmt --check`, `cargo test --locked`
  (117 library and 46 binary/UI tests), strict Clippy, locked release build,
  XML/Cambalache-file parsing, `git diff --check`, focused realized GTK
  coverage, and the 1000x980 screenshot run/inspection.
- TON-013 remains `In Progress`. Cambalache 1.0.3 is installed, but no
  round-trip edit was performed.

## Stale or contradictory documentation found

- `docs/UI_ARCHITECTURE.md` treated the earlier 1000x760 Stage 2 artifact as
  the only final bounded artifact and grouped all treatment-specific rows as
  Rust-built. The current correction was separately recorded and the wording
  was narrowed to the dynamic Shapes/Curves/Motif rows.
- The durable tracker lacked the current correction's test counts, 1000x980
  artifact path, and explicit Cambalache round-trip limitation.
- Historical Stage 2 evidence still records its 1000x760 artifact; it remains
  valid historical evidence and was not rewritten as current correction
  evidence.

## Remaining documentation gaps

- No dedicated assistive-technology tree or focus-order capture exists.
- Narrow-window behavior remains follow-up verification because the separate
  normal/narrow capture attempts were compositor-limited.
- Future migration of dynamic Shapes/Curves/Motif detail rows remains open;
  TON-013 is not complete.

## Git HEAD and working-tree assumptions

- HEAD was `546ea4c5eb1fec8e91c2b307545e33e42331e308`.
- The worktree already contained dirty `src/ui.rs`, tracker, project
  rehydration, cache, resources, docs, evidence, and backup changes. This pass
  changed only documentation/tracker/rehydration/cache/evidence files and did
  not modify application source or UI resources, commit, push, publish,
  deploy, or delete implementations.

## Commands and artifacts

- Read-only inspection used `git status --short`, `git log -1`, `rg`, `sed`,
  `nl`, `sha256sum`, and `file`.
- Implementation and review commands were not rerun by this documentation-only
  pass; their recorded results were checked against current source/resources
  and the current artifact dimensions.

## Invalidation conditions

Invalidate after changes to `src/ui.rs`, any `resources/ui/*` file or CMB hash,
the cited implementation/review evidence or screenshot, TON-013 documentation,
GTK/libadwaita behavior, Git HEAD, or the preserved dirty-worktree assumptions.
