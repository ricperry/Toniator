# Toniator Progress Tracker

Last updated: **2026-08-05**. The durable execution contract is
[GREENFIELD_REWRITE_PLAN.md](docs/GREENFIELD_REWRITE_PLAN.md). Normative
architecture remains in the five protected [Project Specification files](Project%20Specification/Addendum.md).

## Checkpoints and stage status

### Stage 0 — baseline relocation/spec checkpoint

**Complete at `11c2c8e`.** Greenfield rewrite baseline and protected
specification inputs established.

### Stage 1 — nine-crate foundation

**Complete at `567d307`.** Nine-crate workspace, dependency guard, CLI/app
shells, and architecture guidance are committed. No geometry, rendering,
persistence, source decoding, GTK resources, or exports exist yet.

### Stage 2 — authoritative document and invalidation boundary

**Accepted awaiting checkpoint (not committed).** The shared worktree contains
the reviewed and user-accepted implementation, but `HEAD` remains
`567d307` until the parent creates the explicit checkpoint. Verified summary:

- Authoritative `Document` and `DocumentSession` with stable IDs and discrete
  revisions; immutable validated transitions and stale evaluation-token
  rejection.
- Domain validation with stable schema paths for non-finite/invalid canvas,
  density, transforms, colors, opacity, mark sizes, references, and targets.
- Commands classify `Presentation`, `Realization`, `Family`, and `Source`
  invalidation; source mutation is intentionally deferred.
- Headless `toniator validate` uses the shared domain/engine boundary.
- Nine integration tests (four domain, three engine, two CLI), plus workspace
  format/check/clippy/tests and architecture/CLI validation. No geometry,
  rendering, persistence, source decoding, async evaluation, or GTK.

### Stage 3 — straight-guide family output

**Planned; not authorized or started.** See the bounded contract in
[the Stage 3 plan](docs/GREENFIELD_REWRITE_PLAN.md#stage-3--straight-guide-family-output-next-bounded-stage).

### Stages 4–5 — first complete vertical slice

**Planned.** Stage 4 covers source sampling, circular mark realization, and
canonical marks without a renderer. Stage 5 covers shared RenderScene,
headless raster/SVG consumers, CLI render, final clipping, and artifact/golden
inspection. Details and non-goals are in the plan; neither is started.

### Stages 6–9+

**Planned, high level only.** Async scheduling/cancellation/caches; view-only
GTK preview; command bindings/undo-redo/current persistence/editors; then
newly scoped generalized families, connected/region output, multiframe, and
simple transitions. Each later stage requires explicit approval.

## Maintenance rules

- Use only these status words: Planned, In progress, Implemented awaiting
  review, Accepted awaiting checkpoint, and Complete at commit `<hash>`.
- The parent owns accepted/complete transitions and checkpoint hashes. A writer
  reports proposed status; evidence cannot substitute for user acceptance or a
  commit.
- Update this ledger at every stage transition. Keep implementation evidence
  in `.codex-work/`; keep durable decisions and the approved scope in the plan.
- Preserve dirty worktree files and protected specifications. Do not stage,
  commit, push, or start the next stage from an earlier handoff.
