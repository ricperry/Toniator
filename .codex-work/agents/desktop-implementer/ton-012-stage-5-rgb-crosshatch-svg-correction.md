# TON-012 Stage 5 RGB Crosshatch SVG correction evidence

- Repository absolute path: `/home/ricperry1/projects/Toniator`
- Git HEAD: `4161635d90ee81421ffa1f2dc52e2a381d18c6d7`
- Producing agent: `desktop_implementer`
- Timestamp: 2026-07-26
- Task: correction-only parity fix for RGB-output Crosshatch SVG compositing.

## Working-tree assumptions

- The existing Stage 5 source edits, modified `.codex-work/cache-index.md`,
  untracked `AGENTS.md`, backups, audit evidence, and prior implementation
  evidence were present before this correction and were preserved.
- This correction changed only `src/svg_export.rs` among product source files.
- No other writer was active.

## Implementation

- `src/svg_export.rs`: derives one authoritative Crosshatch compatibility flag
  from `artwork_pipeline.assignment`. Crosshatch SVG groups now use
  `mix-blend-mode:multiply` even when the overall output model is RGB, matching
  `curve_render.rs` raster compositing. Ordinary RGB Curves retain Screen.
- The existing authoritative Crosshatch label behavior remains intact.
- Added `rgb_output_crosshatch_svg_uses_multiply_and_authoritative_labels`,
  constructing RGB-output Crosshatch with a stale legacy `value_mode` facade
  and asserting four Multiply groups, no Screen groups, and K/C/M/Y labels.

## Existing abstractions reused

- `ChannelAssignment::LegacyCompatibility`,
  `LegacyCompatibilityAssignment::CrosshatchProgressiveKcmyV1`,
  `generate_curve_geometry_for_pipeline`, and existing SVG export fixtures.

## Checks

- `cargo fmt --check` — passed.
- Focused SVG tests — passed: RGB-output Crosshatch Multiply/labels, existing
  authoritative Crosshatch labels, ordinary stale-facade RGB Curves, and
  canonical Curve SVG export.
- `cargo clippy --locked --all-targets -- -D warnings` — passed.
- `git diff --check` — passed.

## Artifacts and limitations

- No screenshots, GTK launch, or manual graphical verification was performed
  or claimed.
- No unrelated issues, architecture, or UI behavior were changed.

## Invalidation conditions

- Revalidate if SVG export compositing, Curve layer generation, authoritative
  assignment semantics, or the recorded Git/worktree assumptions change.
