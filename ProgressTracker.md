# Toniator Progress Tracker

Last updated: **2026-08-06**. The durable execution contract is
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

**Complete at `e842a8a`.** The reviewed and user-accepted implementation is
recorded in the Stage 2 checkpoint. Verified summary:

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

### Cross-stage baseline artwork fixtures

**Complete at `8f3b759`.** `assets/raster-sample.png` is the canonical
RGBA/alpha source fixture and `assets/vector-sample.svg` is the canonical SVG
fixture with live text. Relevant source, sampling, render, preview, and export
stages must use both without modifying them; derived artifacts belong under
`target/validation/`.

### Stage 3 — straight-guide family output

**Complete at commit `f60eb65`.** The accepted bounded implementation provides
deterministic headless straight-guide family output: two rotated/translated
guide dimensions, analytical off-canvas guard coverage, intersection sites
with stable provenance/fingerprint, and canonical sorted JSON inspection.
Focused crate and workspace tests, strict Clippy/checks, architecture
validation, and the canonical JSON comparison passed. During Stage 3,
point-site correctness was not visually confirmable on a plotted canvas because
no visible-output path existed; that deferred coordinate-level visual
verification was later resolved through the user-accepted Stage 5 artifacts.
Marks, rendering, source sampling, and GTK remained unimplemented in Stage 3.
See the contract in
[the Stage 3 plan](docs/GREENFIELD_REWRITE_PLAN.md#stage-3--straight-guide-family-output).

### Stages 4–5 — first complete vertical slice

**Stage 4 — Complete at commit `31f4cc9` (alpha-associated correction accepted).** It adds byte-boundary PNG/SVG source
decoding, deterministic straight-sRGB source fields with independent alpha and
linear-light Rec.709 luminance, clamped `StretchToCanvas` sampling, canonical
circular-mark realization from immutable Stage 3 sites, and headless compact
`inspect marks` JSON summaries. Both baseline assets, their documented hashes,
SVG live-text/font-policy diagnostics, guard-mark preservation, realization
reuse, and presentation independence are covered by focused and workspace
validation. The accepted alpha-associated correction was discovered during
Stage 5 visual validation. No renderer or clipping is present in Stage 4.
**Stage 5 — Complete at commit `31f4cc9` at the RenderScene/renderer boundary.** It now provides one immutable renderer-owned `RenderScene`
from the Stage 4 realization, a headless straight-sRGBA `RasterSurface` and
PNG encoder, deterministic SVG circles with a canvas clip path, and headless
`toniator render` extension selection. Both immutable sources have inspectable
PNG/SVG artifacts under `target/validation/stage-5/`, which received user visual
acceptance. The alpha-aware carried condition is satisfied, and deferred Stage
3/4 coordinate-level visual verification is resolved. No binary goldens were
committed or accepted. Stage 6 is not started and remains planned; GTK remains
out of scope. Details and non-goals are in the plan.

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
