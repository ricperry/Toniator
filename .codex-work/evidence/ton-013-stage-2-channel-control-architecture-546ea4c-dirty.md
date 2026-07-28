# TON-013 Stage 2 channel-control architecture

- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `546ea4c5eb1fec8e91c2b307545e33e42331e308`
- Working tree: `src/ui.rs`, `ISSUES.md`, `.codex-work/`, and untracked
  `resources/`/docs files are dirty; `resources/ui/Toniator.cmb` hashes the
  current `Toniator.ui`.
- Producing agent: codebase explorer
- Timestamp: 2026-07-26

## Verified current architecture

- `EditorWidgets` and `build_editor_view` remain flat and Rust-owned.
- The current Document expander combines source, output, assignment, active
  channel, Crosshatch, and appearance controls.
- Treatment callbacks live in `AppUi::connect_actions`; synchronization lives
  in `sync_controls` and deferred `sync_controls_when_idle`.
- `OutputChannelId` already supplies stable CMYK/RGB semantic identities.
- Current channel choice is represented by `active_channel`, treatment target
  dropdowns, and fixed visibility arrays; no reusable channel composite exists.
- `sync_dropdown_strings` preserves `GtkStringList` identity and existing
  realized tests cover invalid positions and model transitions.

## Settled architecture recommendation

- `ToniatorChannelControls.ui` defines the reusable visual composite only.
  Rust instantiates/caches one widget per real `OutputChannelId` and binds the
  semantic ID; callbacks never capture transient dropdown indexes.
- Aggregate `All Inks`/`All Channels` controls use a separate aggregate panel,
  never an aggregate channel ID or a fake channel instance.
- Source and Output become separate collapsible panels at the top of the
  inspector. Channel Settings follows and contains explicit Editing Scope.
- Rust retains dynamic models, visibility/sensitivity, semantic callbacks,
  synchronization, drawing, and runtime content. UI files own static structure,
  labels, stable IDs, and placeholders.

## Open implementation boundary

The current UI has selector-driven treatment controls rather than a native
per-channel row model. Stage 2 should establish the reusable template and
semantic ownership without silently changing render semantics; aggregate edits
must remain clearly scoped and one undoable operation. Preserve existing GTK
model identity, deferred synchronization, and CMYK/RGB transition protections.

## Commands and invalidation

Read-only inspection used `sed`, `nl`, `rg`, `find`, `git status`,
`git rev-parse`, `git diff --check`, and `sha256sum`. Invalidate after changes
to `src/ui.rs`, `src/artwork_pipeline.rs`, `resources/ui/*`, relevant GTK
tests, GTK/libadwaita versions, Git HEAD, or dirty-file assumptions.
