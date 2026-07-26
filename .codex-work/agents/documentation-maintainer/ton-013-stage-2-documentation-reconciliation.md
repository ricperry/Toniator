# TON-013 Stage 2 documentation reconciliation

- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `546ea4c5eb1fec8e91c2b307545e33e42331e308`
- Timestamp: `2026-07-26`
- Producing agent: `documentation-maintainer`
- Task: reconcile durable docs for the completed, reviewed, and corrected
  TON-013 Stage 2 channel-inspector milestone.

## Documentation files reviewed

- `docs/UI_ARCHITECTURE.md`
- `ISSUES.md` (TON-013 section only)
- Existing documentation structure: `README.md`, `docs/`, and the existing
  UI architecture location; no new durable documentation location was needed.

## Documentation files changed

- `docs/UI_ARCHITECTURE.md`
- `ISSUES.md` (TON-013 Stage 2 progress and verification boundary)
- `.codex-work/cache-index.md` (this cache record index)
- This reconciliation record

## Implementation evidence used

- `src/ui.rs`: `build_inspector_hierarchy`, `build_channel_controls`,
  `build_aggregate_channel_controls`, `sync_channel_scope_panels`, the
  `Treatment Editing Scope` callback, and `verify_realized_channel_scope_composites`.
- `resources/ui/ToniatorInspector.ui`,
  `ToniatorChannelControls.ui`, `ToniatorAggregateChannelControls.ui`, and
  `Toniator.cmb`.
- `.codex-work/agents/desktop-implementer/ton-013-stage-2-channel-inspector-implementation.md`
  and `ton-013-stage-2-treatment-scope-correction.md`.
- `.codex-work/evidence/ton-013-stage-2-treatment-scope-final-ux-546ea4c-dirty.md`.
- `test-artifacts/ton-013/stage2-treatment-scope-correction.png`, verified as
  a 1000x760 PNG and inspected for the final hierarchy/scope presentation.

## Verified reconciliation

- The actual top inspector order is Source -> Output -> Channel Settings;
  Source and Output are expanded by default, while Channel Settings,
  Appearance / Canvas & Export, and Treatment Settings are collapsed.
- `ToniatorChannelControls.ui` is a reusable real `OutputChannelId`
  status/context composite cached for seven semantic C/M/Y/K and R/G/B
  channels. The aggregate resource is separate and presents All Inks, All
  Channels, or Crosshatch All Layers status/context.
- `Treatment Editing Scope` is the sole visible treatment-recipient selector
  and synchronizes Shapes and Curves. Output `Channel Assignment` and
  conditional `Active Channel` remain pipeline-authoritative. Hidden legacy
  Adjust Ink/Adjust Channel rows remain compatibility internals.
- TON-013 remains In Progress because substantial treatment-specific layout is
  still Rust-built.
- The durable docs now record the inspected screenshot path, the compositor
  render-node limitation for failed normal/narrow captures, and the missing
  assistive-technology/focus-order capture as follow-up scope.

## Stale or contradictory documentation found

- The historical Stage 2 implementation record still describes the pre-
  correction `Scalar Channel Routing` control and visible Adjust Ink/Adjust
  Channel rows as the treatment authoring locus. It is retained as historical
  evidence; the correction record and current implementation supersede it.
- The earlier Stage 2 UX decision record says Channel Settings begins
  expanded; the corrected resource, realized test, and final UX review establish
  the shipped default as collapsed. No stale claim remains in the durable docs.

## Remaining documentation gaps

- No dedicated assistive-technology tree or focus-order capture exists.
- Narrow-window, screen-reader, and focus-order behavior remain follow-up
  verification; the compositor-limited normal/narrow screenshot attempts did
  not produce artifacts.
- Future treatment-specific layout migration remains outside Stage 2.

## Git HEAD and working-tree assumptions

- HEAD was `546ea4c5eb1fec8e91c2b307545e33e42331e308`.
- The worktree already contained dirty Stage 1/Stage 2 source, resources,
  tracker, docs, evidence, cache, and backup changes. This pass did not modify
  application source or UI resources and did not commit, push, publish, deploy,
  or delete implementations.

## Commands and artifacts

- Read-only inspection used `git status --short`, `git rev-parse HEAD`, `rg`,
  `sed`, `nl`, `git diff`, `git diff --check`, `file`, and `sha256sum`.
- Implementation tests and UX review commands were not rerun by this
  documentation-only pass; their recorded results were checked against the
  current implementation and resources.

## Invalidation conditions

Invalidate after changes to `src/ui.rs`, Stage 2 UI resources or CMB hashes,
the TON-013 section, `docs/UI_ARCHITECTURE.md`, the cited implementation or UX
evidence, the screenshot artifact, GTK/libadwaita versions, Git HEAD, or the
relevant dirty-tree assumptions.
