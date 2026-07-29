# CACHE_UPDATE — TON-010 Stage 5 Framework Restart / Substage A

- Repository: `/home/ricperry1/projects/Toniator`
- Timestamp: 2026-07-29
- Git HEAD: `87b4ce37d633181df485728cb903c4ff15b9470a` on `TON-010-Stage5-Framework-Restart`
- Working-tree assumption: `nextPrompt.md` and
  `ton-010-stage5-pre-framework-preservation-2026-07-29.md` were pre-existing
  untracked files. This substage adds `src/site_distribution.rs`,
  `src/voronoi_geometry.rs`, and this report, and modifies `src/lib.rs`.

## Implemented boundary

- `site_distribution::{DomainBounds, OrderedPoint, DistributionField,
  DistributionRequestMetadata, DistributionRequest, SiteDistribution}` provides
  neutral, deterministic finite-candidate placement; it has no model, artwork,
  UI, pattern, render, or export dependency.
- Uniform placement uses jittered stratified candidates and never reads source
  values. Source-weighted placement uses a positive normalized field, polarity,
  strength, shared/independent identity-aware candidate arrangements, and an
  exponential-race selection without replacement. It produces exact counts,
  avoids rejection/attempt fallback, and records a fingerprint of metadata and
  ordered output.
- `DistributionLimits` centralizes the 8,192-site and 65,536-candidate caps.
  `GeometryLimits` centralizes the 8,192-site geometry cap.
- `voronoi_geometry::{build_voronoi_diagram_cancellable, VoronoiDiagram,
  inset_clipped_cell}` is pure clipped Voronoi construction over neutral ordered
  points. It adapts the archived half-plane clipping, bounded spatial search,
  topology quantization, and artboard-aware inset math without importing any
  prior pattern-specific ownership.
- Diagram boundaries explicitly distinguish artboard clipping segments from
  shared interior segments. Insets leave artboard supports unchanged and only
  offset actual interior supports.

## Verification

- `cargo fmt --check` — passed.
- `cargo check --locked` — passed.
- `cargo test --locked site_distribution` — 5 selected library tests passed.
- `cargo test --locked voronoi_geometry` — 4 selected library tests passed.
- `git diff --check` — passed.

The focused tests cover deterministic ordering and seed changes, source-free
uniform output, spatial spread, source weighting/polarity/strength, exact and
distinct sites, shared versus independent arrangements, cancellation and
limits; plus cell bounds, shared and artboard boundary classification,
artboard-preserving inset behavior, controlled degenerate/cancelled errors,
and clustered bounded construction.

## Artifacts, limits, and follow-up

- No screenshot, GTK launch, export, or preview artifact: this substage has no
  UI, renderer, persistence, or export integration.
- No durable Stage 5 plan/documentation was changed. The next review should
  inspect public neutral API names, numeric tolerance/cap values, and whether a
  later adapter needs a separate persisted representation for request metadata.
- Revalidate this entry if `src/site_distribution.rs`, `src/voronoi_geometry.rs`,
  `src/lib.rs`, `src/cancel.rs`, the archived source at
  `archive/TON-010-Stage5-Voronoi-pre-framework:src/weighted_voronoi.rs`, or the
  current HEAD/dirty-file assumptions change.
