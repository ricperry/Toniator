# TON-010 Stage 2 authority pause and sequence revision

- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `f9c138c`
- Checkout: dirty; existing TON-013 Blueprint migration and TON-010 Stage 1
  changes preserved.
- Date: 2026-07-28

## Pause result

The Stage 2 `desktop_implementer` was interrupted before verification. It had
partially added schema metadata in `src/pattern.rs` and registry-selector
scaffolding in `src/ui.rs`. The parent removed the unverified UI synchronization
because it routed selection through `RenderVariant` and therefore extended the
dual-authority boundary. The isolated schema metadata remains unaccepted
groundwork; it is not connected to persistence or UI.

The partial schema additions initially failed `cargo check` because static
metadata was incorrectly given serde derives. The parent removed serialization
from the static metadata descriptor and `cargo check --locked --all-targets`
then passed. No Stage 2 implementation or acceptance evidence exists.

## Corrected architecture decision

Before schema-driven UI, Stage 2 must establish one authoritative persisted
pattern instance containing stable ID, schema/generator versions, and validated
typed parameters. At the Stage 2 implementation cutover, this becomes the only
persisted pattern selector. `RenderVariant` becomes a transient Shapes/Curves
execution adapter/cache only; it cannot own pattern selection, persisted
parameters, or a second undo authority. Current document/preset definitions,
bundled presets, fixtures, and expected artifacts must be updated together, and
obsolete definitions rejected without migration/defaulting code.

## Required output correction

Canonical output must cover marks, paths, filled cells/polygonal regions,
shared boundaries/networks, and negative-space/polarity semantics. Weighted
Voronoi is required in TON-010 and must use seeded source-weighted sites,
intentional edge coverage, optional uniform rendered size, canonical
preview/PNG/SVG geometry, and filled cells separated by subtracted boundaries.
Boundary/network and polarity infrastructure must be reusable by future maze
patterns.

## Scope correction

The full custom-pattern editor, local library, import/export format, project
embedding, and embedded-asset recovery are moved to a separate follow-up issue.
TON-010 retains only the registry/authority/output extension points needed to
make that future work possible.

## Verification

- `cargo check --locked --all-targets` passed after the pause cleanup.
- No Stage 2 formatter, Clippy, full-test, GTK, screenshot, or UX-review gate
  was run or claimed.
- Invalidate after any Stage 2 authority, pattern, UI, persistence, preset,
  renderer, or tracker implementation changes.
