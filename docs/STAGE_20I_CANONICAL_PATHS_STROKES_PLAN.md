# Stage 20I — Canonical Paths and Strokes

Status: **Complete at implementation checkpoint `de166f533379dc5b75d5a36e38baf145d0fac6c2`**; the user accepted Stage 20I on 2026-08-21. Publication remains separate.

Stage 20I adds one ordered guide-path output kind beside existing marks. A
definition remains homogeneous: it produces marks or guide paths, never a
mixed/composite scene. The path branch consumes the resolved Stage 20D
`GuidePathSet` directly, preserving guide identity and order; it does not
create offsets, adjacency, connection programs, regions, or canvas topology.

The document base owns `Connected` minimum/maximum normalized thickness and a
channel can supply matching optional additive deltas. `Document::effective_channel_pattern`
remains the only authority that composes and validates these values. Thickness
is finite in `0.0..=2.0`, uses each guide's nominal spacing basis, and is a
realization concern. The only accepted style is explicit round joins and round
caps. Path output requires solid channel paint; SourceColorAlpha path output
is rejected before evaluation.

Patterns retain exact open or closed `CurvePath` centerlines and realize them
into renderer-independent canonical filled outlines. Source mapping is sampled
along the path with bounded deterministic subdivision; each stroke has a finite
ordered width profile and compact derived contours. The canonical path is never
pre-clipped, split, or rederived. Raster and SVG apply the existing final canvas
clip and paint each nonzero outline once per stroke; separate strokes remain
composited in guide order.

The current document wire vocabulary remains schema v4 additively, and preset
format v2 remains mark-only. Derived paths, profiles, outlines, capabilities,
scenes, and caches are not serialized. Existing v4 fixtures and existing mark
output retain their current representation.

Verification covers authority, inheritance/deltas/reset, path ordering,
variable-width outline bounds and cancellation, raster/SVG parity and final
clipping, cache reuse/invalidation, deterministic v4 save/reopen, and both
immutable source artworks at intrinsic dimensions. The divergent-channel
Holiday document remains a mark-regression witness. The approved mechanical app
adapter and private harness evidence add no dedicated path workflow, GTK feature,
or inspector reorganization. Stage 20J+ remains separately gated.

## Detailed implementation decisions

### Final contract detail

`GuidePaths` is a typed output payload, distinct from every mark prototype. It stores
`PathStrokeStyle { join: StrokeJoin::Round, cap: StrokeCap::Round }` in definition,
IO, pipeline identity, realization identity, and scene identity. A document has one
`PatternGeometryResponse::Connected(ConnectedGeometryResponse { minimum_thickness,
maximum_thickness })`; a channel may store only the matching
`ChannelGeometryResponseDelta::Connected` optional additive members. The effective
resolver remains the only authority and rejects branch mismatch, nonfinite values,
values outside `0.0..=2.0`, inverted ranges, incompatible output topology, source
color paint, and nonzero mark-only shape rotation for a path recipe. The connected
descriptor pair is document/channel current/effective/inherited/reset-capable and
has Realization invalidation.

Family output publishes raw ordered `CurvePath` guide instances and each guide's
resolved nominal spacing. Straight and generic guide families preserve dimension then
index order. Realization samples source response adaptively at exact
`PathLocation`s: De Casteljau control-polygon flatness and width interpolation are
bounded at `1/64` document units; accepted intervals are at most one half of the
smaller source-pixel footprint; recursion depth is at most 48. Cancellation polls
inside subdivision and outline construction. A request accepts at most 262,144 profile
samples and 524,288 derived outline segments across all strokes.

`CanonicalStroke` stores source guide/structure identity, raw centerline/closure,
per-guide basis, explicit style, profile samples, a reusable nonzero filled outline,
and finite outline bounds. The geometry service validates finite ordered exact path
locations and widths, preserves segment boundaries, zero transitions, seams, and
tangent discontinuities, and simplifies center and half-width independently within
one eighth document unit. It emits cubic rails, true cubic round caps, explicit outer
round joins, and two opposite-winding contours for closed positive paths. All-zero
profiles produce an empty outline; self-overlap remains deterministic nonzero winding
without Boolean cleanup. Consumer clipping occurs only at raster/SVG output.
Raster fills one bounded nonzero outline per stroke in one source-over operation;
inter-stroke order remains source-over. SVG serializes one direct filled path per
canvas-intersecting stroke under the one outer canvas clip. Source-color path layers
are rejected; path layers have no sampled mark paint.

Engine realization keys use a tagged Marks/Connected response identity. Connected
keys include the outline contract identifier and profile/outline-segment limits;
EvaluationLimits exposes setters for those request-wide values and passes them to realization.
Connected family support takes the maximum spacing of every emitted guide dimension,
not merely dimensions selected for site products. Definition/layout changes remain Family work;
connected response changes remain Realization work. No effective projection is saved;
schema remains v4 and preset v2 remains mark-only.

## Verified implementation and acceptance record

The implementation is recorded at checkpoint
`de166f533379dc5b75d5a36e38baf145d0fac6c2`, whose parent is the Stage 20H
documentation checkpoint. The user accepted Stage 20I on 2026-08-21. The
checkpoint retains the reusable geometry-owned compact variable-width outline
authority, direct filled SVG paths under the final canvas clip, one raster fill
per stroke in guide order, and current Stage 20G effective-value authority.

Focused domain, geometry, patterns, render, engine, and IO witnesses passed,
along with affected-package check/Clippy, formatting, architecture validation,
and `git diff --check`. Natural and low-resolution raster/vector artifacts are
under `target/validation/stage-20i/`; the natural path outputs are
`path-raster.*` and `path-vector.*`, and `holiday-fresh.*` preserves the
divergent-channel regression witness. SVG parsing and headless Inkscape clip
release/export passed. The private Sway/AT-SPI run exercised the Connected
minimum-thickness edit and preview refresh without diagnostics; it is automated
wlroots evidence, not manual GNOME/Mutter acceptance.

The current document schema remains v4 and preset format remains v2. Derived
profiles, outlines, scenes, capabilities, and caches remain omitted from
persistence. The separate density-versus-resolution terminology follow-up
remains tracked and is not changed by Stage 20I.
