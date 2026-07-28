# TON-013 Stage 1 documentation reconciliation

- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `546ea4c5eb1fec8e91c2b307545e33e42331e308`
- Timestamp: `2026-07-26`
- Producing agent: `documentation-maintainer`
- Task: reconcile durable documentation for the completed and reviewed TON-013
  Stage 1 GtkBuilder shell milestone.

## Documentation files reviewed

- `docs/UI_ARCHITECTURE.md`
- `ISSUES.md` (TON-013 section only)
- Existing documentation structure: `README.md`, `docs/`, and the named
  architecture document location; no new durable documentation location was
  needed.

## Documentation files changed

- `docs/UI_ARCHITECTURE.md`
- `ISSUES.md` (TON-013 Stage 1 progress and verification only)
- `.codex-work/cache-index.md` (this cache record index)
- `.codex-work/agents/documentation-maintainer/ton-013-stage-1-documentation-reconciliation.md`

## Implementation evidence used

- `resources/ui/Toniator.ui`: the static
  `AdwApplicationWindow -> AdwToolbarView -> AdwHeaderBar ->
  AdwToastOverlay -> GtkStack` shell, 13 stable IDs, initial Controls
  tooltip, and stack transition properties.
- `src/ui.rs`: `TOP_LEVEL_SHELL_UI`, `build_top_level_shell`, `AppUi::new`
  insertion of Rust-owned `start` and `editor` pages, runtime Controls tooltip
  and accessibility updates, and focused Builder tests.
- `.codex-work/agents/desktop-implementer/ton-013-stage-1-gtkbuilder-shell-implementation.md`:
  implementation boundary and verification record.
- `.codex-work/evidence/ton-013-gtkbuilder-migration-seams-546ea4c.md`:
  pre-implementation ownership boundary and migration rationale.
- `.codex-work/evidence/ton-013-stage-1-independent-shell-ux-546ea4c-dirty.md`:
  independent review, documentation finding, verification commands, and
  deferred review limits.
- `test-artifacts/ton-013/shell.png`: inspected 900x680 GTK artifact.

## Stale or contradictory documentation found

- The TON-013 Stage 1 progress text did not explicitly name the
  selector-driven controls as remaining In Progress.
- The architecture wording did not distinguish the XML-declared initial
  Controls tooltip from Rust's runtime tooltip and accessibility ownership.
- Cambalache wording could be read as a completed round-trip claim even though
  the milestone retained a normal editable `.ui` file but did not launch
  Cambalache.
- The issue section lacked a bounded Stage 1 verification record and the
  limitation on `gtk4-builder-tool validate` with libadwaita types.

## Remaining documentation gaps

- TON-013 remains In Progress: the editor hierarchy, selector-driven controls,
  and reusable channel composite are not yet migrated or verified.
- Cambalache round-trip behavior, narrow layout below the default width, and
  assistive-technology behavior remain unverified for this stage.
- A future GResource/build pipeline and reusable component templates remain
  design/workflow decisions, not shipped Stage 1 behavior.

## Git HEAD and working-tree assumptions

- HEAD was `546ea4c5eb1fec8e91c2b307545e33e42331e308`.
- Existing dirty implementation changes in `src/ui.rs`, the new
  `resources/ui/Toniator.ui`, existing `ISSUES.md` and cache-index changes,
  stage evidence, and backup files were preserved. This pass did not edit
  application source or XML, commit, push, publish, deploy, or delete any
  implementation.

## Commands and artifacts

- Read-only inspection used `git status --short`, `git rev-parse HEAD`,
  `find`, `sed`, `rg`, `git diff`, `git diff --check`, and `xmllint --noout`.
- The reviewed evidence records successful `cargo fmt --check`, strict
  Clippy, `cargo test --locked`, release build, and real GTK screenshot
  launch; those checks were not rerun by this documentation-only pass.

## Invalidation conditions

Invalidate this entry after changes to `resources/ui/Toniator.ui`, the named
`src/ui.rs` shell loader/page insertion or runtime Controls ownership, the
TON-013 section, `docs/UI_ARCHITECTURE.md`, the cited Stage 1 evidence or GTK
artifact, GTK/libadwaita versions, Git HEAD, or relevant working-tree state.
