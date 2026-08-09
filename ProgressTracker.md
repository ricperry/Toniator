# Toniator Progress Tracker

Last updated: **2026-08-08**. The durable execution contract is
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

**Complete at `8f3b759`, extended at `8f4925d`.**
`assets/raster-sample.png` is the canonical RGBA/alpha source fixture,
`assets/vector-sample.svg` is the canonical SVG fixture with live text, and
`assets/video-sample0001-0010.mp4` is reserved for future multiframe and
animation validation. Relevant approved stages must use their applicable
baselines without modifying them; derived artifacts belong under
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

### Stages 6+

**Stage 6 — Complete at commit `d8d1dc3`.** Authoritative document evaluation
and the bounded Stage 3 family-output correction passed parent review, user
visual acceptance, and the implementation checkpoint gate.

**Stage 7 — Complete at commit `ed2183f`.** The bounded engine-only scheduler
uses one standard-library worker, checked monotonic tickets, queued-work
coalescing, pipeline-boundary cancellation, latest-only completion polling,
and clean shutdown/Drop joining. Scheduled results for both immutable sources
match synchronous Stage 6 identities and pixels.

**Stage 8 — Complete at commit `67503ae`.** Add bounded, invalidation-aware
last-successful caches for decoded source, family output, realization, scene,
and raster preview.
The approved contract includes transactional acceptance by the current
scheduler ticket/document token, immutable hit/miss diagnostics, declared
pattern support capability instead of the temporary global 2.0–9.0 diameter
restriction, and a configurable checked family-candidate safety limit.
Implementation, the automated final gate, user review, and the implementation
checkpoint are complete. Authoritative multi-channel document evaluation,
then view-only GTK preview, undo/redo, portable persistence, command-bound
editors, generalized families, connected/region output, multiframe, and simple
transitions remain planned.

**Stage 9 — Complete at commit `67e831a`.** The complete bounded headless authoritative
multi-channel document-evaluation path is split into five separately reviewed
and locally checkpointed substages. Every substage starts only after the
previous substage is user-accepted and its implementation plus tracker closeout
are committed locally. Push remains optional and requires a separate user
decision.

**Stage 9A — Complete at commit `c821568`.** Add authoritative channel models, canonical and
explicit ordered topologies, stable IDs, source mappings, paint compatibility,
atomic model/topology replacement, revision behavior, affected-channel
reporting, and `ChannelTopology` invalidation in the domain layer.

**Stage 9A corrective checkpoint — Complete at commit `2320feb`.** Add the domain-owned
complete-document evaluation snapshot/token boundary required by Stage 9D
without changing the accepted single-channel evaluation APIs.

**Stage 9B — Complete at commit `fb1b31d`.** Add deterministic linear RGB/full-UCR CMYK source
fields and SourceColorAlpha sampled-paint realization, including alpha
association exactly once and zero-alpha paint suppression.

**Stage 9C — Complete at commit `d37469a`.** Add the fixed additive RGB, idealized subtractive CMYK,
and SourceColorAlpha raster/SVG compositors while preserving single-layer
behavior, transparent straight-sRGBA, consumer-only PNG backing, and ordinary
editable per-channel SVG geometry with documented raster/SVG semantic
correspondence.

**Stage 9D entry seam correction — Complete at commit `cad3705`.** Rename the retained
single-channel engine and CLI path to explicit channel-diagnostic APIs so the
unprefixed engine namespace can become complete-document authority in Stage 9D.

**Stage 9D — Complete at commit `0d73d88`.** Add complete-document ordered multi-channel engine
evaluation, strict aggregate/per-channel identities, accepted-cache reuse,
transactional scheduler behavior, and ordered diagnostics.

**Stage 9E — Complete at commit `67e831a`.** Migrate `toniator render` to the
authoritative document path, add channel-model and export-background CLI
semantics, generate the native review artifacts, and run the complete Stage 9
integration gate. The accepted Stage 9E CLI still requires explicit `--canvas`
for direct-source rendering and does not yet expose PNG antialiasing control.
Its accepted artifact template uses channel opacity `1.0`; SourceColorAlpha
source alpha changes mark size only, leaving positive-alpha SVG marks opaque
and PNG mark interiors at alpha `1.0` except for antialiased edge coverage.

**Stage 10 — Complete at commit `980af50`.** The accepted implementation keeps
intrinsic PNG/SVG document dimensions and native export raster bytes intact,
and adds a renderer/engine-derived fitted preview raster target for the GTK
viewport. It rerasterizes canonical scene geometry at output-pixel resolution,
preserves cache authority and stale-ticket rejection, and does not move
geometry/composition into GTK. The app still accepts only optional `PATH`; it
has no document/canvas override. Parent review, automated verification, user
visual acceptance, and the local implementation checkpoint are complete.

The preview final consumer clips supersamples to the fitted authoritative canvas
rectangle, so guard geometry cannot paint letterbox margins; the tracked splash
regression verifies 1280×640 into 960×720 rows 120..600.

**Stage 11 — Complete at commit `341ad8e`.** Add unbounded session-lifetime headless undo and redo
around `DocumentSession`, storing exact validated before/after documents and
the original invalidation result for every successful command. Apply, undo,
and redo each advance authority monotonically; failures and empty operations
are atomic no-ops, branching clears redo, and GTK/persistence/coalescing remain
out of scope.

**Stage 12 — Complete at commit `dd7ca56`.** Add immutable `.toniator` container/document version 1,
IO-owned version-specific DTOs, exact embedded source bundles, deterministic
version dispatch with no transforming migration yet, atomic save, CLI
create/validate/render, and frozen PNG/SVG-backed v1 fixtures for the later
v1-to-v2 migration gate. History, dirty state, and filesystem source paths are
not serialized.

**Stage 13A — Planned.** Add GTK New/Open/Save/Save As/Close, exact content-based
dirty/savepoint tracking, history state, direct-source/container opening, title
identity, and atomic error handling. GTK delegates default document creation
and remains completely ignorant of pattern internals.

**Stage 13B — Planned.** Add the dedicated final-consumer output slice: native
direct-source CLI sizing with optional explicit canvas, PNG antialiasing on/off,
and matching GTK PNG/SVG export controls without changing document, preview
canvas, family, realization, or scene authority.

**Stage 14 — Planned.** Add the one-root mechanism-agnostic typed pattern schema,
stable document/mechanism IDs, atomic definition CRUD, selected-channel
copy-on-edit, explicit shared-definition editing, immutable-v1 document-schema
migration to v2, and exact accepted-output parity. No named artistic pattern
branches or GTK controls.

**Stage 15 — Planned.** Generalize the headless family-to-modulation-to-ordered-
output-to-canonical-geometry pipeline, cache identity, support/coverage,
provenance, limits, and cancellation before adding new mechanisms.

**Stage 16A — Planned.** Add generalized one-to-four-dimension straight-guide
mechanisms, independent angles/phase, intersections and along-guide sites,
reusable mark prototypes, and complete transformed guard coverage without named
rectangular/triangular branches.

**Stage 16B — Planned.** Immediately prove the framework is not grid-shaped by
adding deterministic raw/even/exclusion, clustered, and source-weighted site
distributions with `u32` seeds, spacing guarantees, achieved-density
diagnostics, and the same downstream output machinery.

**Stage 17 — Planned.** Complete typed headless pattern/channel commands and
schema-derived property/capability descriptors, exact invalidation, stable-ID
allocation, copy/shared semantics, undo/redo, and rendered restoration before
any GTK inspector controls.

**Stage 18 — Planned.** Add the descriptor-driven GTK channel inspector over
`DocumentHistory`, with progressive disclosure and no hidden values, ID
allocation, pattern mathematics, validation, or alternate schema in GTK.

**Stage 19A — Planned.** Add the versioned headless pure-schema preset registry
and reconstruction gates; preset names remain metadata and never evaluator
branches.

**Stage 19B — Planned.** Add the separate descriptor-driven structural Pattern
Editor, defaulting shared definitions to atomic copy-on-edit and exposing
deliberate affected-channel **Edit Shared Definition** behavior.

**Stage 20+ — Planned.** Add curved/procedural guides, connected/network output,
regions/Voronoi and reusable offsetting, composite output mechanisms,
user-authored structures, multiframe sources, and transitions only through the
typed schema, generic evaluator, headless commands/descriptors, canonical
geometry, and final-consumer clipping.

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
