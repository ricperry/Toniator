# Toniator Progress Tracker

Last updated: **2026-08-30**. The durable execution contract is
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
guide dimensions, analytical off-canvas guard coverage, intersection sites with
stable provenance/fingerprint, and canonical sorted JSON inspection. The
user-authorized 2026-08-24 Stage 20M worktree correction establishes the
current centered local grid-prototype contract and regenerates the current
Stage 3 fixture without rewriting that checkpoint or migrating schema/persistence.
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

**Stage 9B — Complete at commit `fb1b31d`.** Add deterministic linear RGB/normalized CMYK source
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
checkpointed at `ccec466`. The user-authorized 2026-08-24 Stage 20M worktree
correction establishes the current centered local grid-prototype placement for
generalized and authored generic curve-grid guides; random distributions and
parametric structural sources retain their existing geometry/fingerprints, with
no schema/persistence migration or historical-checkpoint rewrite.

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

**Stage 20A — Complete at commit `b7fbd81`.** The headless evaluator now
publishes `FamilySiteSet` as the truthful deterministic derived-site authority
for each typed family result. `TypedFamilyOutput` is opaque, generalized and
random products retain their actual provenance, and a private circle
compatibility adapter preserves the existing canonical circle realization
without publishing fabricated structural facts. Schema, persistence, cache
keys, canonical circle/render/UI behavior, and GTK behavior are unchanged.
Focused natural-resolution PNG and structural/live-text SVG evidence, the
complete-document cache/output checks, and read-only implementation review
passed. This was a headless-only checkpoint, so it has no GTK evidence.

**Stage 20B — Complete in the Stage 20B acceptance checkpoint.** The bounded headless
geometry-only contract for finite line/polyline and cubic Bézier paths,
deterministic evaluation, bounds, arc-length lookup, intersections, and ordered
clipping is implemented. Exactly one `desktop_implementer` owned the
implementation allowlist; focused tests, dependent checks, strict lint,
architecture validation, and independent read-only review pass. The geometry
capability adds no schema, persistence, cache, render/export, CLI, GTK, fixture
semantics, or later-stage authority. The checkpoint additionally tracks the
authorized current-format real-world `.toniator` validation case and this
durable documentation; the fixture is acceptance data, not a Stage 20B geometry
input. The checkpoint is intentionally named rather than self-referenced by hash.

**Stage 20C — Complete in the Stage 20C acceptance checkpoint.** The accepted bounded headless contract adds only
document-owned authored open paths and closed shapes, authoritative commands,
descriptors, history, deterministic current-v2 persistence, and exact
conversion to Stage 20B construction geometry. It adds no consumer, evaluator,
cache, canonical output, renderer/export, CLI, GTK, preset, or later-stage
behavior. The exact focused gate and independent read-only review pass, and the
user accepted the implementation and separately authorized its single local
checkpoint on 2026-08-13. The checkpoint includes the implementation and
synchronized durable documentation and is intentionally named rather than
self-referenced by hash.

**Stage 20D — Complete in the Stage 20D acceptance checkpoint.** The accepted
headless boundary adds authored-open-path and circular-arc guide prototypes,
bounded `Single` and `TransformStack` repetition, baseline/phase/stack
transforms, deterministic conservative coverage, existing guide-site product
consumption, document-aware identity and invalidation, transactional
cancellation/cache behavior, and deterministic current-v2 persistence. The
complete implementation began from planning checkpoint
`453104e39204afc1e10397b9d5bbf551dd85deac`; focused verification and
independent read-only review pass. A narrow post-review `toniator-app`
compilation correction adds presentation labels for the new typed guide fields,
choices, and authored references only; it does not add Stage 20D editing UI or
move authority into the frontend. Private Sway/AT-SPI evidence is automated
only and does not claim manual GNOME/Mutter acceptance. The acceptance
checkpoint is intentionally named rather than self-referenced by hash.

**Stage 20E1 — Complete in the Stage 20E1 acceptance checkpoint.** The accepted
bounded contract in `docs/STAGE_20E1_NORMALIZED_MARK_FILL_PLAN.md` replaces the
temporary absolute mark-size and 4.5 support-capability model with deterministic
per-site nominal cell bases, normalized `0.0..=2.0` fill, derived conservative
coverage, a breaking current-format transition, and synchronized existing
GUI/CLI controls. At that checkpoint, documents used schema v3 only and
presets used format v2 only; obsolete document-v1/v2 and preset-v1 decoders are
not retained. Stage 20N later supersedes those formats with schema v5 and
preset v3.
Focused verification, native PNG/SVG validation, private control evidence, and
the independent repair re-review all pass. The re-review specifically confirms
family-aware support preflight, removal of the unit-diameter fallback, and
deterministic rejection of parallel or near-parallel contributors. No authored
shape or Stage 20E2 implementation was performed.

**Stage 20E2 — Complete at commit
`0c6b6a2e268f9306835038be747352a0cd64044c`.** The accepted bounded
implementation adds explicit document-owned authored-closed-shape mark
references, exact
bounds-center normalization and Fixed/Tangent/Normal realization, truthful
canonical closed paths, even-odd native/preview/SVG consumers, request-wide
transformed-segment and flattened-edge limits, cooperative cancellation, and
complete cache/scene identity. At that checkpoint, document-v3 and preset-v2
persistence remained additive with no obsolete decoder or migration. Stage 20N
later supersedes those formats with schema v5 and preset v3. Focused headless tests,
strict affected-crate lint, architecture validation, both immutable intrinsic
source-artwork PNG/SVG witnesses, and a private Sway app-consumption smoke check
pass. The independent read-only review found three sampled-paint/identity gaps;
the bounded repair and focused re-review close all three without further
findings. A final reviewed engine-to-render zero-alpha witness also proves site
cardinality and hidden-RGB suppression. The automated Sway evidence is not
manual GNOME/Mutter acceptance. The user explicitly accepted Stage 20E2 on
2026-08-14; its local implementation checkpoint includes the reviewed code,
focused tests, and deliberate HolidayMugs fixture/checksum update. Stage 20F+
work remains excluded.

**Stage 20F — Complete at commit
`7117e24b8c9e2e723c3c23e7e9050dc71277d15c`.** The bounded contract in
`docs/STAGE_20F_GUIDE_SHAPE_EDITOR_PLAN.md` now exposes authored open guides
only through provisional **Edit guide paths…** and authored closed marks only
through provisional **Edit mark shapes…**. They are private-draft
authored-resource editors, not the final Pattern Wizard. It includes reusable
path editing, deterministic typed use disclosure, one-step private-history
squash, and accessible main/draft pending-preview feedback while preserving
current schema, preset, evaluator/cache, and canonical-output authority. A real GNOME/Mutter run exposed a
construction-gesture crash, ineffective new-resource application, and an overly
narrow editor; the same implementation writer repaired those failures. Focused
verification and independent test and UX reviews pass. The user explicitly
accepted Stage 20F on 2026-08-21. Publication remains excluded; Stage 20G is
separately gated below.

**Stage 20G — Complete at commit
`de1320ba359beee42223ef994baebfd9ecd94c9c`.** The bounded contract and
accepted implementation are documented in
`docs/STAGE_20G_EFFECTIVE_PATTERN_AUTHORITY_PLAN.md`. The user explicitly
accepted Stage 20G on 2026-08-21; publication remains separate.

**Stage 20H — Complete at commit
`4b1cc08819eee36c2009e2abf5543dcaefe29929`.** The approved bounded capability-projection
contract is recorded in
[`STAGE_20H_CAPABILITY_PROJECTION_PLAN.md`](docs/STAGE_20H_CAPABILITY_PROJECTION_PLAN.md).
It adds only a domain-owned, typed, read-only projection of the document base
or effective channel recipe. It preserves Stage 20G effective authority and
does not alter persistence, commands, invalidation, cache identity, CLI, GTK,
or canonical evaluation. Focused domain, patterns, and engine witnesses pass,
and the independent implementation review passed. The user explicitly
accepted Stage 20H on 2026-08-21; publication remains separate.

**Stage 20I — Complete at commit
`de166f533379dc5b75d5a36e38baf145d0fac6c2`.** The accepted canonical-path/stroke
outline correction is bounded by
`docs/STAGE_20I_CANONICAL_PATHS_STROKES_PLAN.md`. It introduces ordered
guide-path output, effective connected thickness response, reusable
geometry-owned compact filled outlines, and final-consumer clipping only;
offsets, adjacency, regions, composites, presets, GTK feature work, and later
stages remain excluded. The user explicitly accepted Stage 20I on 2026-08-21;
publication remains separate.

**Stage 20J — Complete at commit
`2edbb8659a82106ce8de904ef1ce9155e3b4d777`.** The accepted Path Offset and
Constant Gap implementation is governed by
[`STAGE_20J_PATH_OFFSET_CONSTANT_GAP_PLAN.md`](docs/STAGE_20J_PATH_OFFSET_CONSTANT_GAP_PLAN.md):
persisted absolute-gap `NormalOffset` guide repetition, reusable compact
line/cubic centerline offsets, tangential padded-domain endpoint extension,
deterministic crossing dissolution, Stage 20I outline reuse, and current-v4
persistence. Focused domain, geometry, patterns, engine, IO, render, CLI/SVG,
Holiday, and private Sway/AT-SPI evidence is green; independent re-review has
no remaining finding. This historical absolute-gap wording is superseded for
current terminology: `NormalOffset` now denotes positive parallel-centerline
spacing and never region negative-space or absolute-gap computation. Automated
wlroots evidence is not manual GNOME/Mutter review. The user explicitly
accepted Stage 20J on 2026-08-22; publication,
later Stage 20M+ work, protected specification changes, and Legacy work remain
separate.

**Terminology follow-up — Tracked.** Revisit the public and internal use of
`density`, especially `across_x` and `across_y`: those values currently express
guide resolution/count along the document axes, while density conventionally
means guides per unit distance. Do not rename the fields or change their
behavior as part of Stage 20I; scope the schema, persistence, CLI, and UI impact
in a separately authorized follow-up.

**Stage 20K — Complete at commit
`f848ff995c9e30f89a85fbc01b5b8d97cc8de3d5`.** Parametric Curves implementation remains governed by
[`STAGE_20K_PARAMETRIC_CURVES_PLAN.md`](docs/STAGE_20K_PARAMETRIC_CURVES_PLAN.md).
It adds only the common round/square finite spiral family, raw curve paths or
equal-arc curve sites, and reusable accepted repetition. User review correctly
rejected the first bounded Stage 20J cusp/reversal correction governed by
[`STAGE_20J_CUSP_REVERSAL_CORRECTION_PLAN.md`](docs/STAGE_20J_CUSP_REVERSAL_CORRECTION_PLAN.md)
because its cubic artifact retained a crossing lattice. The reopened repair
removed that lattice, and the subsequent one-sided diagnostic finding is now
corrected: its persisted intent requests both sides. User review then found
zero-source-span endpoint extensions from wholly reversed repetitions re-entering
the canvas. The follow-up removed those paths but direct inspection of its
unclipped output found a remaining floating chevron/diamond: cleanup kept
isolated authored fragments after their tangential endpoint extensions had
crossed. The extended envelope now collapses at that crossing. The exact
clipped and clip-released Inkscape outputs were inspected directly: repetition
14 is the final clean cusp pair, repetition 15 and farther are absent, and no
floating or re-entering descendants remain. Focused Stage 20J and directly
relevant Stage 20K verification is green. The user explicitly accepted the
Stage 20J correction on 2026-08-22. Stage 20K's low-detail render claim is now
superseded by intrinsic raster/vector evidence using five full turns with
artboard-derived radial pitch. The correction also replaces pathological
nanounit polygon/chord arc-length refinement with bounded adaptive quadrature
and limits raster winding work to row-active outline edges. All eight native
PNG outputs and all eight Inkscape SVG renderings were inspected directly and
cover their artboards with coherent round/square path and equal-arc-site
geometry. The user accepted Stage 20K on 2026-08-22. The accepted
implementation checkpoint is `f848ff995c9e30f89a85fbc01b5b8d97cc8de3d5`;
publication remains separate.

**Stage 20L — Complete at implementation checkpoint
`b41fa3fcf2e1089ea422ba18524c2c4a26f568e8`.** The user
accepted the deterministic, mechanism-neutral derived site-adjacency contract
on 2026-08-23. It is implemented across geometry, patterns, and engine with
bounded cancellable construction, guard-inclusive family evaluation,
ordinary-output identity preservation, and no document intent, schema, preset,
history, renderer, CLI, GTK, or later connection-program work. Focused and
broad affected-package verification is green, the bounded current-suite
maintenance pass is complete, and the final independent read-only review found
no material issue. The checkpoint includes the user-edited
`.agents/skills/toniator-orchestrator/SKILL.md`; that guidance change is
preserved as user-owned and is not Stage 20L product authority. Publication and
Stage 20M remain separate gates.

**Stage 20M — Complete at implementation checkpoint
`33f1bde3be9afdc3fb88f479c4ee7ec52b80114a`.** The user authorized the bounded headless
connection-program implementation on 2026-08-23 and accepted it on 2026-08-24 under
[`STAGE_20M_CONNECTION_PROGRAMS_PLAN.md`](docs/STAGE_20M_CONNECTION_PROGRAMS_PLAN.md).
The user then clarified that `GridMaze` is a conventional wall maze: every
evaluated grid site on or inside the canvas is valid candidate arrangement and
fingerprint authority, while outside sites are excluded. The emitted maze
connects consecutive actual guide sites, extracts every positively oriented
bounded face, and selects the largest stable connected bounded-face component
only when finite candidates are disconnected. Every inclusive candidate stays
source/fingerprint authority, but only walls bounding selected cells emit; there
is no degree filter, fixed shell, stroke-width inset, or site-clearance rule.
Positive wall width and caps may spill past the canvas and are handled by final
clipping. Selected faces form one connected dual cell graph, recursive
backtracking removes one spanning tree of shared walls, and exactly two existing
perimeter openings bound the one derived cell-traversing solution. A rectangular
final clip may leave disconnected transparent fringe outside bounded maze cells
because canvas bounds do not create closure. Positive
`GridSpanningTree` remains a separate connection-path program. Focused
verification, artifact inspection, independent read-only review, bounded repair
re-review, and the final centered-origin review found no material findings. The
accepted headless scope retains the centered grid-prototype origin, positive
nearest/random/tree paths, normalized `0.0..=2.0` response, no GTK work, and no
renderer topology repair. Its checkpoint-era current-v4/preset-v2 persistence
is superseded by Stage 20N's schema-v5/preset-v3 boundary.
The final boundary repair added square/triangular clip-aligned
regressions and confirmed that all selected maze cells remain reachable in one
dual spanning tree while disconnected candidate components are discarded before
post-selection connectivity is required. A final requirement audit additionally made two-/three-guide geometry
coverage direct and added public connection/maze capability-projection
coverage without changing production behavior. Publication remains separate.
**Stage 20N is Complete at implementation checkpoint
`b8701686042a69fcd1ac68a4038adbad4c0ccdc9`;** the user accepted the
multi-output authority and canonical-region foundation on 2026-08-25 under
[`STAGE_20N_MULTI_OUTPUT_CANONICAL_REGIONS_PLAN.md`](docs/STAGE_20N_MULTI_OUTPUT_CANONICAL_REGIONS_PLAN.md).
The accepted implementation retains the one-output authoring/validation gate;
it adds ordered output settings, independently keyed realization/cache units,
and canonical-region/render foundations without adding a concrete region
source or region treatment. Schema v5 documents and preset v3 recipes persist
authored keyed output settings and channel deltas; derived effective values,
regions, diagnostics, limits, caches, and scheduler state remain absent.

**Stage 20O — Complete at implementation checkpoint
`7ab97f01ec372ab1e6201b3913742476a1511c02`.** The user accepted the bounded
ordinary Voronoi implementation on 2026-08-25. It retains the Stage 20N
one-output gate, schema-v5/preset-v3 authored persistence, and renderer-owned
final clipping. Eligible `FamilySiteSet` products include along-guide and
`AlongParametricCurveSites`; direct raw `ParametricPaths` remain ineligible.
It adds guard-inclusive realization, exact-duplicate co-ownership, bounded
geometry-private Spade topology, and fixed solid Full regions without a
channel-specific region treatment. Independent re-review and final artifact
inspection passed. At that checkpoint, Stage 20S remained Planned and
separately gated.

**Stage 20P — Complete at implementation checkpoint
`cd531eb65dd2e161e62f355905ad936b8c1ca3c4`.** The user accepted the bounded
headless guide-arrangement-face implementation on 2026-08-25 under
[`STAGE_20P_GUIDE_ARRANGEMENT_FACES_PLAN.md`](docs/STAGE_20P_GUIDE_ARRANGEMENT_FACES_PLAN.md).
It adds typed `GuideFaces` region intent for two or three selected straight or
authored-open guide dimensions, deterministic complete bounded guide
arrangements, canonical regions, v5/v3 authored-intent persistence, and the
existing final-clip renderer path. Region treatments, sampling, composites,
GTK controls, and raw-parametric face sources remain out of scope.
The final 0°/60°/120° witness runs the normal production family evaluator from
the shared centered document origin at equal spacing; each retained pre-clip
face is asserted to be a three-line equilateral triangle with all three guide
sources. The prior canvas-relative renderer-only proof was removed. The
shared Stage 20M grid transform and maze fingerprints remain unchanged; a
Guide Faces relevance correction rejects disjoint off-canvas faces before
corner containment classification. Writer verification, focused replay gates,
intrinsic native/SVG artifact generation, visual inspection, independent
read-only review, and parent verification are complete. Direct raw
`ParametricPaths` remain Guide-Faces-ineligible, while typed parametric
site/Voronoi mechanisms remain valid. Do not push, publish, or begin Stage
20R.

**Stage 20Q — Complete at implementation checkpoint
`071f3604098c0660a876fbe30050a64223fe41b3`.** The user reaccepted the bounded
headless filled-region realization on 2026-08-26 after the repaired
implementation passed independent review and parent verification confirmed the focused
tests, current foundations, strict checks, architecture and protected-input gates,
read-only semantic-map impact/navigation/freshness evidence (not `semantic-map check`
or architecture authority), artifact hashes, RGB/alpha statistics, and intrinsic native/SVG
raster inspections under
[`STAGE_20Q_FILLED_REGION_REALIZATION_PLAN.md`](docs/STAGE_20Q_FILLED_REGION_REALIZATION_PLAN.md).
It realizes only fill-only `Full`, `Scale`, and `ConstantGap` treatments for accepted
ordinary Voronoi and Guide Face regions, with domain-owned reference/area-average
sampling, typed cache-aware realization, sampled region paint, and final-consumer
PNG/SVG rendering. The schema-v5/preset-v3 authored persistence, canonical
untreated-region identity, final canvas clip, and headless-only boundary were
part of the Stage 20Q checkpoint contract; Stage 20R subsequently lifts the
one-output gate. Positive ConstantGap shrinks and negative ConstantGap grows; convex
outward growth uses subdivided smooth cubic round joins, while inward shrink uses tangent
intersection plus crossing/coincident-branch dissolution. The three-guide evidence uses
positive inward gap and triangular line rings. The authored-cubic outward witness uses a
fixed -40 gap (20-unit outward edge growth and 40-unit neighbor overlap) with smooth joins.
Collapse evidence is intentionally transparent; sparse authored-cubic coverage reflects
six complete bounded faces, not raster resolution. This historical Stage 20Q
record is superseded for current region authority by normalized positive-only
Scale/UniformOffset fill; Full, ConstantGap, absolute-gap, and negative-space
region geometry are not current behavior. Publication remains separate; Stage
20R is complete and Stage 20S remains separately gated.

**Stage 20R — Complete at implementation checkpoint
`458c9a981dd349999240a18052e055a71c7b6c3c`.** Authorized implementation began on 2026-08-26
under
[`STAGE_20R_COMPOSITE_OUTPUTS_AND_SITE_FILTERS_PLAN.md`](docs/STAGE_20R_COMPOSITE_OUTPUTS_AND_SITE_FILTERS_PLAN.md).
The bounded headless stage lifts the one-output gate, normalizes ordered output
records, adds compatible site-use filters and derived usage, separates painter
order from dependency evaluation order, and extends current v5/v3 persistence,
commands, per-output cache behavior, request-wide composite limits, and native
PNG/SVG evidence. Writer verification passed focused Stage 20R and directly
affected Stage 20G/I/M/N/O/P/Q targets, affected checks, strict library and
compile-only frontend Clippy, formatting, architecture, protected-input, asset
hash, and persistence gates. The regenerated evidence keeps connection paths,
sampled regions, and maze walls in purpose-specific visual witnesses; filter
DAG/painter-order evidence is recorded without an invalid same-channel
connection/region overlay or site-location circle marks. The solid connection
and maze witnesses retain all modeled RGB channels with distinct deterministic
seeds per channel; only the canonical `SourceColorAlpha` sampled-region witness
is intrinsically single-channel. It adds no GTK workflow, compatibility
adapter, renderer topology repair, publication, or Stage 20S work. An independent read-only audit identified a
missing cross-channel connection/region witness and incomplete sampled-paint
validation; both were repaired, and re-review found no remaining material
issue. Parent verification then reran the complete authorized gate matrix and
directly inspected representative native and SVG-rasterized evidence in RGB
and alpha. The user accepted Stage 20R on 2026-08-26; its implementation
checkpoint is `458c9a981dd349999240a18052e055a71c7b6c3c`. The subsequent
documentation closeout is tracked separately. At that checkpoint, Stage 20S
remained Planned and separately gated.

**Stage 20S — Complete at implementation commit
`55651dee7c744c2aa207924bf0dbb7737609942d`.** The user authorized bounded headless capability
and baseline-recipe completion on 2026-08-26 from Stage 20R documentation
checkpoint `88455d703bab4d0c4932b70ba8a4337c299867da` and implementation
checkpoint `458c9a981dd349999240a18052e055a71c7b6c3c`. The approved contract is
recorded in
[`STAGE_20S_CAPABILITY_AND_BASELINE_RECIPE_COMPLETION_PLAN.md`](docs/STAGE_20S_CAPABILITY_AND_BASELINE_RECIPE_COMPLETION_PLAN.md).
The accepted headless implementation supplies capability projection and
descriptors, strict nested preset-v3 DTO rejection, and a version-2 16-card
registry after retiring `regions-plus-marks`. Regions use normalized
positive-only Scale/UniformOffset fill in `0.0..=2.0`, omit zero fill, and do
not compute negative space or absolute region gaps. Ordinary evidence uses
matching R/G/B source components and pairwise-distinct typed seeds; it also
includes CoverCanvas/variable-width spirals and corrected centered-local curved
guides. `semantic-map check` is unavailable and inapplicable because Toniator
has no semantic-map architecture schema; project documentation is architecture
authority, while `scripts/validate_architecture.sh` is the mechanical check.
GTK work and compatibility adapters remain excluded. The user accepted Stage
20S on 2026-08-26 after independent review and re-review, focused verification,
verified evidence, parent intrinsic RGB/alpha inspection, and durable-document
reconciliation. The integrated Stage 20A–20S scrub is recorded below at
implementation checkpoint `dc7e988200c5be4d22791ca1d231336caac19a24`; push,
publication, and later-stage work remain unauthorized and separately gated.

**Final Stage 20 scrub — Complete at implementation checkpoint
`dc7e988200c5be4d22791ca1d231336caac19a24`.** The user accepted the integrated
Stage 20A–20S scrub on 2026-08-27 after the final architecture, persistence,
cache/invalidation, capability/recipe, rendering, cancellation, scheduler,
performance, native-artifact, and independent-review gates passed. The audit
repaired bounded current-authority,
descriptor, persistence-hardening, cache-key, request-budget, and cancellation
defects; added evaluation-local timing/workload/cache/worker-participation
metrics; and parallelized deterministic per-site marks, per-region sampling
and treatment, and independent pixel composition/quantization through Rayon's
bounded shared pool. Complete-evaluation coordination, global topology,
dependency/budget order, painter order, and transactional cache publication
remain serial authorities. Current persistence is container v1/document v5 and
preset v3; the bundled registry remains exactly 16 cards and the retired
`regions-plus-marks` debug tool was not restored. The durable architecture and
performance record is
[`STAGE_20_FINAL_ARCHITECTURE_AND_CONCURRENCY.md`](docs/STAGE_20_FINAL_ARCHITECTURE_AND_CONCURRENCY.md).
The 1024×1024 release scaling proof is recorded in the same architecture
document. The accepted checkpoint remains headless-only; no push, publication,
GUI re-baseline, or Stage 21 work is authorized by this acceptance.

**GTK4/Blueprint re-baseline — Complete in checkpoint `ToniatorGUI`.** The user
authorized this separately gated infrastructure migration on 2026-08-27
through `guiRefactorPrompt.md` and accepted it on 2026-08-27 after the bounded
implementation and independent re-review passed. `toniator-app` now uses GTK4
without libadwaita, scoped Blueprint/GResource templates own the primary
static interface composition, `main.rs` retains runtime and dynamic behavior,
and compact controls retain natural vertical sizing. Blueprint compilation,
focused app tests, app check/strict Clippy/format, architecture and diff gates,
fresh raster/SVG private-Sway runs, narrow and normal Pattern Editor flows,
targeted AT-SPI interaction, and independent read-only review passed. Evidence
is recorded in
`.codex-work/evidence/2026-08-27-gtk4-blueprint-rebaseline-parent-review.md`.
Stage 21 pattern-authoring redesign, headless authority changes, publication,
and push remain unauthorized and separately gated.

**Stage 20N+ history and remaining roadmap — Stage 20R Complete; Stage 20S Complete at
`55651dee7c744c2aa207924bf0dbb7737609942d`.** The user accepted the revised remainder
roadmap on 2026-08-24 under
[`STAGE_20N_20S_HEADLESS_PATTERN_COMPLETION_PLAN.md`](docs/STAGE_20N_20S_HEADLESS_PATTERN_COMPLETION_PLAN.md).
The accepted order after completed 20Q is 20R composites and site-use filters;
then 20S headless capability/gallery recipe
completion. The integrated Stage 20 scrub is complete at implementation
checkpoint `dc7e988200c5be4d22791ca1d231336caac19a24`. Stage 21 owns
pattern-authoring GTK, Stage 22 owns all headless
frame/media/sequence/simple-transition work, and Stage 23 owns temporal GTK
with start/end pins. The integrated final scrub is complete at the checkpoint
above. The separately authorized GTK4/Blueprint re-baseline is complete in
checkpoint `ToniatorGUI`; Stage 21A is separately tracked below, while Stage
21B and Stages 22–23 remain Planned and separately gated.

**Stage 21A — Main Window and Still-Image Import — Complete at commit
`3028193b787960fb402b0af6807d6e8e8ab174db`.**
The user authorized the first bounded Stage 21 checkpoint on 2026-08-27. The
user accepted it on 2026-08-28 after implementation, focused verification,
direct artifact and private-harness inspection, and independent re-review.
This stage owns
the density/density-aspect authority and current-only document schema v6,
expanded single-still source decoding/import, explicit ALL-versus-channel
inspector targeting, the compact non-mutating pattern candidate plus atomic
Apply Pattern behavior, capability-filtered common settings, and the private
Advanced Settings modal. Focused tests, system-dav1d checks, Clippy, formatting,
architecture validation, intrinsic native PNG/SVG inspection, and private
Sway/AT-SPI/grim evidence pass. Automated wlroots evidence is not human
GNOME/Mutter or native-portal acceptance. Stage 21B Pattern Wizard and personal
preset-library work remains Planned and separately gated; Stage 21A acceptance
satisfies only its sequencing prerequisite. No push or publication is implied
by this transition.

The subsequent human acceptance pass found duplicate decoration, queued-preview
status, persistent-error, density-control, dense-family, and long-render
feedback defects. The bounded correction registers the Blueprint header as the
native titlebar; makes errors dismissible and clears obsolete preview failures;
projects Density as inverse **Pattern zoom level** with **Pattern aspect**;
removes default application-authored creative work ceilings while retaining
machine representation, fallible incremental allocation, cancellation, and
explicit test limits; and publishes ticketed fixed-weight progress in one
overlay with an overall bar, a current-stage bar, visible percentages, and
stage text through family, ordered-output, scene, raster, and final-publication
work.
The final fresh/direct zoom reference is longest-axis normalized:
`100 × sqrt(min(width, height) / max(width, height))` at aspect `1.0`, yielding
square 100×100, 2:1 100×50, and 1:2 50×100 coverage without rewriting stored
schema-v6 Density. The first source-backed workspace exposes its pending stages
before idle viewport submission; source-less New remains empty. Dispersion now
retains final accepted sites while releasing transient parent/spatial storage,
cache results share immutable scene/raster values, and raster composition uses
only an accumulator plus one layer with fallible allocation rather than an
estimate-based creative refusal. AreaAverage reuses worker-local clip buffers,
reports completed source cells, and duplicate inner-loop progress is coalesced
before frontend delivery instead of accumulating unbounded queued messages.
Its sampling contract now classifies literal decoded-source pixel footprints
by an inclusive 50% exact-coverage threshold: selected pixels contribute their
full mapped values once, fractional coverage never scales a pixel value, and
complete off-source geometry retains repeated unit footprints with nearest-edge
clamping. Indexed Rayon region work preserves deterministic input-order results,
checked request-wide counts, cancellation, and completed-footprint progress.
Guide-path thickness now samples at the nominal pattern interval rather than
half-pixel source supersampling; curved-centerline refinement remains separate
and current retained-sample response is linear. Advanced Settings clarifies that connected
minimum thickness zero is a source-driven lower bound: positive sampled ink
remains visible, while all-zero response produces no contours.
Every bundled pattern now accepts channel layout rotation except
source-weighted site placement, whose capability projection omits Rotation,
whose channel commands reject it, and whose evaluator receives zero rotation.
Named recipe/topology/structural transitions prune only an incompatible channel
rotation delta while shared base rotation remains dormant. Rotated canonical
strokes use a fallible row-active edge schedule and sparse nonzero-winding spans
with bounded cancellation polling rather than rescanning every edge and pixel
inside each widened axis-aligned bounds. The complete sixteen-preset engine
matrix reaches realization, scene construction, and rasterization with nonzero
rotation for every eligible recipe.
Focused parametric, generic-guide, random, scheduler, renderer, app, CLI, and
dense three-guide checks pass. All sixteen bundled recipes evaluate after the
density increase, and the independent correction re-review found no remaining
blocker. Fresh private Sway evidence shows one titlebar, dismissible errors,
inverse zoom controls, and determinate output/raster progress for both immutable
source types. That was the first ready-for-acceptance state; the user then
reopened the performance and memory gate covered below.

The reopened performance and memory gate is complete. Every bundled preset was
run in a separate process against both immutable sources at intrinsic dimensions
with baseline, Pattern zoom `0.80`/`1.40`, Pattern aspect `2.0`/`0.5`, eligible
rotation `17`/`89.5`, response minimum `0`/`0.25`, response maximum `1`/`0.75`,
combined zoom `0.80` plus rotation `17`, and capability-valid shape-rotation and
region-sampling cases. All eligible final cases pass; the Source-Weighted
Dispersion + Voronoi preset reports its exact expected rotation rejection.
Guide-Face peak-RSS reductions range from 59.8% to 84.8% in the recorded
isolated comparisons.
Private Sway evidence covers the asynchronous 128×72 Advanced Settings preview,
two-level progress, one-modal behavior, Apply/Undo, compact normal and narrow
layouts, and Cancel-while-pending followed by immediate reopen without stale
publication. Independent review found no blocking correctness or lifetime
defect. The stress runner accepts one case per invocation, so complete-matrix
orchestration is external; the recorded claims are limited to the mutation list
above and do not include translation. Automated wlroots evidence remains
non-human and non-Mutter. The user accepted Stage 21A on 2026-08-28. No push,
publication, or Stage 21B work has begun.

**Stage 21B prerequisite — Headless Curve Motif — Complete at commit
`b8826c95f3be2abd13d0e007cd59c49c2ce16915`.**
The user authorized this separately accepted headless checkpoint on 2026-08-28.
It adds one authored open motif chained across one-guide Along Guides rows,
including composable alternate-row mirror/phase and existing source-driven
path thickness, before any Pattern Wizard, personal preset library, or GTK
work. The current-only persistence transition is document schema v7 and preset
format v4; the container remains v1 and the bundled registry remains exactly
sixteen entries. Focused and directly relevant foundational verification,
strict Clippy, formatting, architecture validation, native PNG/SVG inspection,
and final independent rereview pass. The user-observed coarse vector needle was
corrected through canonical response refinement and a higher existing Pattern
density evidence recipe without adding motif-size authority. The isolated
release performance/memory gate also passed all 28 intrinsic evaluator-only
cases under timeout/RSS supervision with cold 24-worker evaluation and warm
full cache hits. The user accepted this headless prerequisite on 2026-08-28,
and the local implementation checkpoint is
`b8826c95f3be2abd13d0e007cd59c49c2ce16915`. At that prerequisite checkpoint,
Stage 21B remained Planned and still required separate planning completion and
implementation authorization.

**Stage 21B — Pattern Wizard and Personal Library — In progress.**
The user authorized the decision-complete four-gate Stage 21B plan on
2026-08-28 and accepted Gate 21B-1 on 2026-08-28. Gate 21B-1 is **Complete at
commit `f77998c`** and covers the headless Storage and Registry Foundation
only: current-only reusable-resource v1 and library-config v1 storage, safe
personal-library operations, a layered immutable-built-in/personal catalog,
bundled-registry version 3, and the seventeenth `curve-motif-rows` preset.
At that checkpoint, `Edit Pattern` remained disabled, and GTK, Blueprint,
pending-icon adoption, complete wizard pages, and personal-library UI remained
in later gates.
The accepted filesystem implementation uses single-open no-follow reads,
actual byte limits, exact-byte preset parsing and fingerprints,
prepublication stale checks, candidate-root validation, type-qualified
thumbnails, rollback-capable paired trash/undo moves, and an explicit
single-process-writer boundary. Focused domain, registry, current preset-v4,
personal-library, Curve Motif persistence/evaluation, check, strict Clippy,
formatting, architecture, diff, rotated-engine, and private-Sway/apply/
Advanced Settings verification pass; independent rereviews pass. Gate 21B-2
(Wizard Shell and Gallery) is **Complete at commit `63fd9fb`** and was
user-accepted on 2026-08-28. Its corrected card uses a stored density-10
canonical SVG through the same GResource and black-backed presentation as the
other 16 built-ins; the exact 17-icon test, raw SVG/Inkscape contact evidence,
and focused private-Sway gallery inspection pass. The gate implements the
modal transient private Pattern Wizard shell/gallery, shared layered catalog,
all 17 **Use as is** cards, initial Edit support for exactly Curve Motif,
One Guide Lines, Even Random Circles, and Round Spiral Line, 256-longest-edge
source proxy to 512×512 preview, latest-ticket/last-success semantics, one
Apply publication, specified ALL delta reset/named behavior, and wide/narrow
layout. All first-party crates are version `0.2.0`; `toniator --version`
reports `toniator 0.2.0`. Gates
21B-3 (Complete Editing and Nested Editors) and 21B-4 (Personal Management and
Final Verification) are Planned, not begun, and separately gated. The exact contract is
[`STAGE_21B_PATTERN_WIZARD_AND_PERSONAL_LIBRARY_PLAN.md`](docs/STAGE_21B_PATTERN_WIZARD_AND_PERSONAL_LIBRARY_PLAN.md).
The user then accepted a Gate 21B-2 CMYK/SourceColorAlpha stabilization at
commit `b67a3483d2a1670e01b29716a1a33df51e10f7a1`. Ordinary development builds
now use optimization level 2 while retaining development assertions and debug
information, so the measured intrinsic Three-Guide Cells CMYK case fell from
69.22 seconds to 5.50 seconds without changing release semantics. The existing
stress runner now covers authoritative RGB, CMYK, and SourceColorAlpha topology,
and focused app regressions cover ALL/named private materialization, output
deltas, inspector projection, exact Undo/Redo, and stable SourceColorAlpha path
rejection. Both immutable sources and a fresh private-Sway CMYK-to-source-alpha
workflow pass with clean app logs; automated wlroots evidence is not manual
GNOME/Mutter acceptance. Gate 21B-3 remains not begun. No push or publication
is implied by this transition.

**CMYK source-fidelity correction — Complete at commit `a2633e1`.** The user
accepted the correction on 2026-08-30 after selecting it from the
validation-only color-model audit:
linear-light `(1-K)`-normalized C/M/Y fields with the current pure process
paints and fixed subtractive compositor. The correction must preserve
straight-sRGBA alpha association, RGB and SourceColorAlpha behavior,
model-sensitive backgrounds, canonical topology, and pattern geometry. It
includes the required protected Addendum and durable Stage 9 authority
revisions plus focused source-reconstruction and intrinsic raster/SVG visual
evidence. Focused reconstruction tests and both immutable source fixtures prove
that the corrected source separation restores chromatic-channel support and
improves spatially averaged fidelity. Editable CMYK SVG now preserves every
live channel group at its canonical coordinates, moves only each rendered
layer result into a disjoint nested-filter atlas slot, and recombines those
same-document `SourceGraphic` inputs with the protected linear-light
transmittance and coverage-union equations. This removes CSS multiply without
`feImage`, duplicate proxy geometry, or raster embedding. Checked-in opaque and
fractional overlap tests agree with native output within one byte of renderer
quantization; all 36 full-resolution fixture variants render through resvg and
Inkscape, and native/SVG visible-color error improves materially in all six
primary cases. The intermittent RGB-edit-to-CMYK crash investigation is
deliberately deferred until the user can provide a reliable reproducer; it is
not a Gate 21B-3 blocker and must not be mixed into that gate. Gate 21B-3
remains planned, separately gated, and not begun. No push or publication is
implied by this acceptance transition.

## Maintenance rules

- Use only these status words: Planned, In progress, Implemented awaiting
  review, Ready for user acceptance, Accepted awaiting checkpoint, and
  Complete at commit `<hash>`.
- For this closeout, Stage 20B is **Complete in the Stage 20B acceptance
  checkpoint**. That single local checkpoint contains all tracked Stage 20B
  implementation, fixture, and durable documentation; it is not split into a
  separate implementation and documentation checkpoint and is not named by a
  self-referential hash.
- For this closeout, Stage 20C is **Complete in the Stage 20C acceptance
  checkpoint**. That single local checkpoint contains all tracked Stage 20C
  implementation and synchronized durable documentation and is intentionally
  named rather than self-referenced by hash.
- The GTK4/Blueprint re-baseline is **Complete in checkpoint `ToniatorGUI`**.
  The user explicitly named this single local code-and-documentation checkpoint,
  so the ledger names it rather than attempting to self-reference its hash.
- Stage 20D is **Complete in the Stage 20D acceptance checkpoint**, whose
  direct planning parent is `453104e39204afc1e10397b9d5bbf551dd85deac`; the
  acceptance checkpoint is intentionally named rather than self-referenced by
  hash. Stage 20E1 is **Complete in the Stage 20E1 acceptance checkpoint**;
  Stage 20E2 is **Complete at commit
  `0c6b6a2e268f9306835038be747352a0cd64044c`** under its approved contract.
- The parent owns accepted/complete transitions and checkpoint hashes. A writer
  reports proposed status; evidence cannot substitute for user acceptance or a
  commit.
- Update this ledger at every stage transition. Keep implementation evidence
  in `.codex-work/`; keep durable decisions and the approved scope in the plan.
- Preserve dirty worktree files and protected specifications. Do not stage,
  commit, push, or start the next stage from an earlier handoff.
