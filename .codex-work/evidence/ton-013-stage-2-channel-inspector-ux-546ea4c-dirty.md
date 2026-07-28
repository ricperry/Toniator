# TON-013 Stage 2 channel inspector UX decision

- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `546ea4c5eb1fec8e91c2b307545e33e42331e308`
- Producing agent: UX strategist
- Timestamp: 2026-07-26

## Decision

- Put `Source` and `Output` collapsible panels first in the inspector.
- Follow them with `Channel Settings` and an explicit Editing Scope selector.
- Use real semantic channel names such as `Cyan Ink`, `Black Ink`, and
  `Red Channel` for channel instances.
- Use a separate aggregate panel for `All Inks`/`All Channels`; never model it
  as a channel. Crosshatch retains an explicit `All Layers` framing.
- Keep current defaults: CMYK Print, Full Color, Preserve Source Alpha,
  automatic separation, aggregate scope, shared geometry, checkerboard preview,
  and no export background. Source, Output, and Channel Settings begin
  expanded; Advanced and appearance details remain collapsed.
- Prefer `Included Inks`/`Included Channels` when describing export-affecting
  visibility controls.

## Acceptance criteria

- One reusable channel template serves all CMYK and RGB channels.
- Aggregate mode has separate UI/state and no aggregate channel ID.
- Source/output panels are the first inspector sections.
- Aggregate edits declare scope and remain one undo operation.
- Mixed values are explicit rather than silently coerced.
- CMYK/RGB switching preserves semantic identity and stable GTK models.

## Invalidation

Invalidate after changes to source/output/channel UI, semantic pipeline or
model behavior, GTK versions, Git HEAD, or relevant dirty-file assumptions.
