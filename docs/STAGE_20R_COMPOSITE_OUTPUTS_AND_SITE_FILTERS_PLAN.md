# Stage 20R — Ordered Composite Outputs and Site-Use Filters

Status: **Accepted awaiting checkpoint**

## Scope

Stage 20R lifts the one-output gate and supports ordered heterogeneous marks,
structural paths, connection and maze paths, and treated regions over one
shared family. Stored output order is painter order. A separate deterministic
dependency DAG controls evaluation order.

This is a headless-only stage. `toniator-cli` and `toniator-app` receive only
mechanical compile adaptations. Stage 20R adds no flags, GTK workflow,
frontend-owned model, compatibility adapter, renderer topology repair, wall
complement, fallback mark, outline, temporal behavior, or Stage 20S work.

## Domain authority and persistence

- Normalize each output into `PatternOutputLayer { id, source_filter,
  realization }`, where `PatternOutputRealization` owns the existing output
  variants.
- Add `SiteUseFilter::{All, SitesUsedBy { output_layer_id }, SitesUnusedBy {
  output_layer_id }}` and derived `SiteUsageSet` authority with optional exact
  site-mechanism identity, sorted unique site members, and a stable
  fingerprint.
- Site-backed marks, connections, MazeWalls, and Voronoi regions may
  participate in compatible filters. Guide/parametric paths and Guide Faces
  publish empty non-site usage and accept only `All`.
- Reject missing, self, non-site, incompatible-mechanism, and cyclic filter
  references. Derive evaluation order by deterministic topological sorting
  with output ID as the tie-breaker; retain stored order exclusively as painter
  order.
- Normalize preset-v3 recipes into one family recipe plus ordered output
  recipes/settings. Preset references use validated recipe-local output
  indices and materialize to fresh document IDs.
- Keep schema v5 and preset v3. Current records must contain explicit filters;
  reject missing or malformed filters and obsolete versions without adapters.
  Persist only authored layers, order, filters, settings, and deltas.
- Add bundle edits for output settings, site-use filter changes, and output
  movement. A move reorders output settings and each linked channel's keyed
  deltas in lockstep. Typed bundle or recipe replacement remains the only
  composite add/remove/replacement authority.
- Selected copy-on-edit clones and remaps mechanism/output/filter references
  and compatible deltas. Unshared edits retain IDs. Shared edits retain IDs and
  affect linked channels in document order. Undo/Redo restores exact bundles,
  painter order, filters, and deltas.
- Filter edits invalidate `Realization`; painter-only moves invalidate
  `Presentation`; complete output-set replacement and selected copy-on-edit
  invalidate `Family`. Existing treatment, response, mapping, paint, opacity,
  and visibility invalidation contracts remain unchanged.
- Project filter-kind and filter-reference descriptors, painter index,
  compatible filter targets, realization capability, and base/effective
  response. Do not expose usage sets or derived evaluation order.
- Retain one channel-wide paint and opacity. Sampled composites are valid only
  when every output supports sampled paint; a sampled composite containing a
  stroke fails atomically.

## Usage, evaluation, cache, and rendering

- Apply filters to the complete guard-inclusive family site set before output
  realization and preserve family order. `All` selects the complete family,
  `SitesUsedBy` intersects referenced usage, and `SitesUnusedBy` takes the
  complement relative to the complete compatible family.
- Derive usage before final clipping: positive-radius marks whose sampled alpha
  is nonzero; endpoints of selected connection edges; endpoints of retained
  positive maze walls; every co-owner of retained treated Voronoi regions after
  collapse and alpha suppression. Structural paths and Guide Faces publish no
  site usage.
- Empty usage is valid. A used-by filter can produce no sites and an unused-by
  filter can produce the complete compatible family.
- Realizers receive one explicit capability, matching response, and filtered
  site view. Renderers receive completed canonical geometry only.
- Cache each output independently. Filtered keys include filter kind,
  referenced output ID and usage fingerprint, base-family identity, and all
  existing response/source/mapping/paint/limit inputs. Hits replay geometry,
  usage, and diagnostics.
- Keep `All` out of legacy one-output fingerprint branches so accepted Stage
  20N–20Q geometry, realization, scene, PNG, and SVG identities replay
  byte-for-byte.
- Aggregate scene identity, renderer payloads, and cache diagnostics in painter
  order after dependency-ordered evaluation.
- Enforce request-wide `CompositeOutputLimits`: 4,096 output units, 8,388,608
  accumulated usage memberships, and 16,777,216 dependency/selection
  inspections. Count hits and misses across the complete document request.
- Use stable diagnostics under `pattern.output_layers.filter.*`,
  `pattern.output_layers.dependency.*`, `realization.site_filter.*`,
  `realization.site_usage.*`, and `realization.composite.{limits,allocation}.*`.
  Cancellation remains exactly `evaluation.cancelled`.
- Failure, cancellation, stale completion, or limit exhaustion publishes no
  partial output, usage table, family, scene, raster, SVG, or cache candidate.
- PNG and SVG consumers retain painter order, primitive-paint alignment,
  nonzero fills, channel opacity, and final clipping only. They perform no
  filtering, topology construction, splitting, offsetting, closure, or repair.

## Verification and evidence

- Add focused `stage20r_composites` tests across domain, patterns, engine, IO,
  and render for validation, ordering, exact history, persistence,
  complete-family filtering, usage derivation, cache replay/invalidation,
  atomic limits/cancellation, sampled-paint compatibility, and painter-order
  rendering.
- Run Stage 20R plus directly affected current Stage 20G, 20I, 20M, 20N, 20O,
  20P, and 20Q targets; affected-package checks; compile-only CLI/app checks;
  formatting; strict affected-library Clippy; diff/architecture/protected-path
  checks; immutable-asset hashes; and semantic-map reconciliation.
- Generate `target/validation/stage20r/` with current source documents, native
  PNG, raw SVG, SVG rasterizations, hashes, RGB/alpha statistics, painter/DAG
  records, usage identities, and a manifest. Exercise the intrinsic
  1024×1024 raster and 900×620 vector inputs. Visual witnesses keep connection
  paths, sampled regions, and maze walls isolated by channel/output purpose;
  they do not overlay connection and region realizations in one channel or add
  circle marks solely to expose site locations. Solid connection and maze
  witnesses retain every modeled RGB channel and use a distinct deterministic
  random seed for each channel; the sampled region witness remains the
  canonical single-channel `SourceColorAlpha` model. Deliberate painter/DAG order,
  `SitesUnusedBy`, usage identities, and painter swaps remain in a nonvisual
  semantics record and focused tests. Duplicate owners, empty/collapse,
  off-canvas support, opacity, and final clipping remain directly recorded.
- One writer owns all changes. After the writer stops at **Implemented awaiting
  review**, an independent read-only reviewer audits the complete diff. The
  parent repairs findings, reruns every gate, directly inspects native PNG and
  SVG raster evidence in RGB and alpha, then stops at **Ready for user
  acceptance**.

The independent read-only audit found two actionable omissions: the focused
engine suite lacked a cross-channel connection/region witness, and sampled
composites did not require paint for every ordered output. Both were repaired,
the reviewer found no remaining material issue on re-review, and the parent
reran the complete authorized gate matrix. Direct inspection confirmed that
the 1024×1024 connection witness contains connection strokes only, the
900×620 sampled witness contains regions only, and native/SVG rasterizations
agree in visible RGB and alpha coverage. The unchanged historical engine
catch-all remains excluded because it already targets superseded APIs and is
outside the current-stage test boundary.

No commit, push, publication, accepted/complete transition, protected
specification change, immutable-asset change, or `ToniatorLegacy/` work is
authorized.
