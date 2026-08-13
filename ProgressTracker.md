# Toniator Progress Tracker

Last updated: **2026-08-12**. The durable execution contract is
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
`target/validation/`. Low-resolution tests may supplement fast coverage, but
future source/sampling/render/preview/export stages must also exercise the PNG
at its natural 1024×1024 size and the SVG at its natural 900×620 size through
the applicable canonical consumer boundary.

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
v1-to-v2 migration gate. Canonical saves remain deterministic two-file Stored
archives; the reader also tolerates Deflated required files and one exact empty
`sources/` directory marker from benign manual repacks. History, dirty state,
and filesystem source paths are not serialized.

**Stage 13A — Complete at commit `36c7b44`.** Add GTK New/Open/Save/Save As/Close, exact content-based
dirty/savepoint tracking, history state, direct-source/container opening, title
identity, and atomic error handling. GTK delegates default document creation
and remains completely ignorant of pattern internals. The separately accepted
app-only reentrancy correction at `02bc2c9` preserves this lifecycle behavior
while preventing nested model-selector and window-close callbacks; it is not
part of the Stage 14 schema checkpoint.

**Stage 13B — Complete at commit `2a773a3`.** Add the dedicated final-consumer
output slice: direct-source CLI intrinsic PNG/SVG sizing with optional explicit
canvas, PNG antialiasing on/off, checked output-target allocation, and matching
GTK PNG/SVG export controls without changing document, preview canvas, family,
realization, or scene authority. PNG backing, dimensions, and antialiasing are
consumer-only choices; containers retain their stored canvas. GTK export uses
immutable workspace snapshots and lifecycle-generation gating, so pending,
failed, cancelled, and stale exports preserve lifecycle and document state.
The accepted app-test outputs were regenerated at native dimensions (raster
1024×1024; vector 900×620); the earlier tiny-output moire was a display-scale
diagnostic, while AA-off hard edges remain intentional. Automated GTK snapshot
coverage and native artifact inspection are recorded without claiming
exhaustive manual GTK dialog/accessibility acceptance.

**Stage 14 — Complete at commit `88fc6dd`.** Add the one-root
mechanism-agnostic typed pattern schema with document-wide stable
definition/mechanism/output-layer IDs and deterministic ordering. Definition
add, duplicate, retarget, unreferenced removal, selected-channel
copy-on-edit, and explicit shared-definition editing remain atomic
`DocumentHistory` commands with exact stale-base, invalidation,
affected-channel, undo, and redo behavior. Immutable private v1 parsing and
container layout v1 remain unchanged; loading migrates deterministically to
typed document schema v2 and current/migrated documents write v2 only with
exact embedded source bytes. The supported typed configuration preserves
accepted RGB/CMYK/SourceColorAlpha geometry, raster, PNG, and editable SVG
parity across frozen v1 and equivalent v2 documents. Native raw-RGBA and
editable-SVG inspection is recorded without claiming exhaustive manual GTK
acceptance. No named artistic pattern branches or GTK controls; Stage 15
generic evaluation is complete below at `711058b`.

**Stage 15 — Complete at commit `711058b`.** Generalize the typed headless
family-to-modulation-to-ordered-realization pipeline without adding mechanism
vocabulary. Typed capability planning and stable definition/mechanism/layer
provenance now cross the existing support-envelope, candidate-limit, and
cancellation boundaries; unsupported compositions fail before source decode or
partial cache publication. Family, realization, scene, and raster identities
reuse only matching authoritative inputs while the current supported
configuration preserves exact canonical geometry and frozen-v1/saved-v2
RGB/CMYK/SourceColorAlpha PNG/SVG parity. DocumentHistory, immutable v1
migration, deterministic v2 persistence, scheduler acceptance, CLI/preview,
renderers, and corrected GTK lifecycle behavior remain unchanged. Stage 17 is
complete below at `e777270`; later GTK editor work remains planned.

**Stage 16A — Complete at commit `ccec466`.** Add generalized one-to-four-dimension straight-guide
mechanisms, independent angles/phase, intersections and along-guide sites,
reusable mark prototypes, and complete transformed guard coverage without named
rectangular/triangular branches. The user provisionally accepted the stage on
2026-08-09 and required natural-resolution PNG/SVG validation in addition to
low-resolution tests for every future applicable stage. The implementation is
checkpointed at `ccec466`.

**Stage 16B — Complete at commit `77bad7c`.** The typed `RandomSites` family
adds an ordered raw-uniform/even/clustered base process, uniform or
artwork-weighted density modulation, minimum-center or visible-mark exclusion,
and the existing circular output product. Deterministic xorshift32 `u32` seeds,
stable accepted-site provenance, bounded candidate/neighbor work, cancellation,
and requested-versus-achieved plus rejection/scope diagnostics are verified.
Only artwork-weighted structure includes decoded content/pixel identity; source-
independent families remain source-free and logical references remain at decode
lookup. Additive current-v2 persistence preserves the immutable v1 parser,
migration, and existing v2 forms. The shared canonical geometry/clipping,
preview, PNG, and SVG path is exercised at natural 1024×1024 PNG and 900×620
SVG resolution with high-density raw/native artifacts and save/reopen parity.
Automated/native evidence and bounded app liveness are recorded without claiming
separate manual visual, interactive, or accessibility acceptance.

**Stage 17 — Complete at commit `e777270`.** Typed headless pattern/channel
commands and schema-derived property/capability descriptors provide exact
invalidation, stable-ID allocation, copy/shared semantics, undo/redo, and
rendered restoration before any GTK inspector controls. The deterministic CLI
`capabilities [--input PATH]` surface exposes the same descriptor contracts.
Validation covers atomic command rejection, persistence/CLI parity, cache and
scheduler reuse, and natural-resolution raster/vector output evidence; it does
not claim separate manual GTK visual, interactive, or accessibility acceptance.

**Stage 17A — Complete at commit `2a85252`.** Immutable transient compound-variant
drafts derive complete typed payloads for random-character, density-modulation,
exclusion-policy, and guided-output-orientation changes, require explicit
confirmation, and finalize only existing typed edits. Drafts remain outside
descriptors, document/history state, persistence, cache/evaluator inputs, and
frontend state; domain validation, no-op/stale rejection, and history semantics
remain authoritative.

**Stage 18 — Complete at commit `2a85252`.** Descriptor-driven selected-channel
GTK inspection uses the immutable typed current-value reader, stable-ID
selection/fallback, generic descriptor controls, and progressive disclosure.
Selected copy-on-edit and explicit shared editing dispatch through
`DocumentHistory`; visible domain transition drafts require confirmation, while
invalid/no-op drafts preserve document/history and preview state. Selection,
drafts, disclosure, status, and focus are runtime-only; guarded asynchronous
updates retain the last-successful raw-RGBA preview. Lifecycle, persistence, and
canonical preview/PNG/SVG output boundaries remain unchanged. Automated/static
and native artifact evidence is not manual GNOME/Wayland visual, keyboard,
focus, or assistive-technology acceptance.

**Stage 19A — Complete at commit `9919d85`.** The version-1 headless
preset registry uses standalone `preset_format_version: 1` records in stable
order: `even-random-circles`, then `straight-grid-circles`. Preset IDs, names,
categories, descriptions, and thumbnails are metadata only; ordinary typed
recipes reconstruct document-owned definitions through the typed command and
Stage 17A draft authority, never through evaluator/cache/renderer name
branches. Selected application creates an independent definition. Explicit
shared replacement discloses the ordered affected channels and confirms that
scope before mutation. Every bundled record serializes/reloads and
reconstructs independently with canonical PNG/SVG parity at 1024×1024 PNG and
900×620 SVG natural resolution. The accepted RGB-independence evidence proves
that an isolated red edit leaves green/blue definitions, identities, isolated
PNG bytes, and visible geometry unchanged, with the modeled SVG identity
caveat. Schema
v2, container v1, and the immutable v1 parser/migration remain unchanged.
The accepted persistent-`StringList`/`splice`/deferred-rebuild/
invalid-position selector correction is included in this checkpoint as a
bounded GTK correction, not GTK preset UI.

**Stage 19B — Complete at commit `b0b84e4`.** The first descriptor-driven GTK
Pattern Editor failed artist-usability review because raw artwork could not
apply the bundled Random or Grid patterns and the interface exposed
engine-oriented controls instead of a coherent creative workflow. That attempt
is superseded by the accepted GTK application remediation: an adaptive canvas
and persistent artist-facing channel editor, immediate bundled-pattern
application, visible Undo/Redo, and a deliberately opened private-draft Pattern
Editor, all over the accepted headless command, history, scheduler, persistence,
and canonical-output authorities. The user accepted this implementation
milestone at the local `b0b84e4` checkpoint; that acceptance does not claim
manual GNOME Shell/Mutter or exhaustive usability acceptance.

The stage-owned report under `target/validation/stage-19b-gui-remediation/`
records private Sway/AT-SPI evidence, direct persistence and canonical export
witnesses for both immutable inputs, and focused private-draft transition
tests. Its limits remain explicit: portal dialogs are external surfaces,
UI-driven export encountered a private-session keyring surface, and injected
WayVNC keyboard/pointer actions did not reach GTK; direct boundary tests cover
canonical output instead. The report was generated from the implementation
tree immediately before the local checkpoint, whose exact accepted SHA is
`b0b84e4`.

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
