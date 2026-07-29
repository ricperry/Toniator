# TON-010 Stage 5 Framework Restart — Substage A implementation

- Repository/HEAD: `/home/ricperry1/projects/Toniator` at
  `87b4ce37d633181df485728cb903c4ff15b9470a` (`TON-010-Stage5-Framework-Restart`).
- Working tree before edits: untracked `nextPrompt.md` and the pre-framework
  preservation evidence; neither was modified.
- Changed files: `src/lib.rs`, new `src/site_distribution.rs`, new
  `src/voronoi_geometry.rs`, and
  `evidence/ton-010-stage5-framework-substage-a-87b4ce3-dirty.md`.

## Decision and reuse record

- Added a neutral domain, request metadata, identity, arrangement, source-field,
  ordered-points, fingerprint, limits, and cancellable placement service.
- Uniform mode deliberately ignores a provided field. Weighted mode uses finite
  candidates and exponential-race weighted selection without replacement; this
  gives an exact count with no rejection loop, arbitrary attempt cap, forced
  edge/corner sites, or duplicate collapse.
- Reused only valid archive primitives: exact nearest-site half-plane clipping,
  bounded expanding spatial search, quantized shared-boundary collection, and
  artboard-aware support-line insetting. The new geometry module imports only
  `CancellationToken`, `DomainBounds`, `OrderedPoint`, `anyhow`, and std.
- No semantic channel, pattern state, UI, render, export, or persistence code
  was imported or altered.

## Checks and handoff

- Passed `cargo fmt --check`, `cargo check --locked`, `git diff --check`, and
  the nine selected `site_distribution`/`voronoi_geometry` unit tests.
- No GTK launch, screenshot, preview, or export validation applies to this
  non-integrated foundation. No generated artifacts were produced.
- Known limitation: tolerance/cap defaults are neutral framework defaults and
  should be revisited only when a later substage establishes product-scale
  performance requirements. Review the public request metadata before making it
  persisted adapter state.
- Documentation likely affected later: the durable TON-010 Stage 5 plan and
  eventual pattern API/reference material; intentionally untouched here.
- Invalidate/review this entry if the listed modules, `cancel.rs`, archive
  reference, branch HEAD, or dirty-worktree assumptions change. Safe handoff:
  framework is complete; adapter integration has not started.
