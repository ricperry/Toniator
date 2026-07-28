# TON-013 Stage 2 independent channel inspector UX review

- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `546ea4c5eb1fec8e91c2b307545e33e42331e308`
- Working tree: modified `src/ui.rs`, `ISSUES.md`, `.codex-work/`, and
  untracked `resources/`/docs files; preserve all existing changes
- Producing agent: UX reviewer
- Timestamp: 2026-07-26
- Verdict: correction required

## Findings

- High: `ToniatorInspector.ui` orders Source, Output, and Channel Settings
  internally, but `build_editor_view` appends that hierarchy after the Halftone
  selector, presets, and full treatment stack. The requested top-of-inspector
  hierarchy is therefore not topmost in the actual UI.
- High: the new Editing Scope callback changes pipeline scalar routing and
  `active_channel`, while Shapes and Curves retain independent Adjust Ink/
  Adjust Channel selectors that choose treatment-edit recipients. A user can
  select one scope above and edit another scope below.
- Medium: real and aggregate resources are structurally separate and semantic
  widgets are cached correctly, but their current panels are heading/status
  shells with empty hosts while actual editable controls remain under the old
  treatment selectors. The user-facing editing locus is incomplete.
- Medium: Source, Output, Channel Settings, and Treatment Settings all start
  expanded; once moved to the top this is likely scan-heavy. Source and Output
  can remain expanded, but Channel Settings or Treatment Settings should be
  collapsed by default unless the actual editing controls make expansion
  necessary.

## Passing evidence

- Seven cached real `OutputChannelId` instances and one separate aggregate root
  preserve semantic identity across CMYK/RGB transitions.
- Resource IDs, labels, basic accessibility naming, `.cmb` hashes, XML parse,
  and diff checks are coherent.
- No Stage 2 screenshot or assistive-technology inspection was available;
  rendered spacing, narrow layout, focus order, and spoken context remain open.

## Exact review surface

Inspected `resources/ui/ToniatorInspector.ui`,
`ToniatorChannelControls.ui`, `ToniatorAggregateChannelControls.ui`,
`Toniator.cmb`, `src/ui.rs` hierarchy/scope/target synchronization and tests,
`src/artwork_pipeline.rs::OutputChannelId`, `ISSUES.md`, and available Stage 1
artifacts. Commands included `git status`, `git diff`, `git diff --check`,
`rg`, `sed`, `nl`, `sha256sum`, and `xmllint`.

## Invalidation

Invalidate after changes to Stage 2 UI resources, hierarchy placement,
scope/target synchronization, GTK accessibility behavior, screenshots, GTK
versions, Git HEAD, or relevant dirty-tree state.
