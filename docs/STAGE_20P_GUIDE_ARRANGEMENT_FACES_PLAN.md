# Stage 20P — Guide-Arrangement Faces

Status: **Accepted awaiting checkpoint** (user acceptance recorded 2026-08-25;
independent read-only review, parent verification, and final artifact
inspection passed; implementation checkpoint pending).

Stage 20P adds `RegionSourceIntent::GuideFaces { guide_mechanism_id,
dimensions }` for ordered selections of two or three eligible guide
dimensions. It accepts `StraightGuideDimensions` and authored-open-path
`GuideDimensions`; it rejects legacy ID-less guides, circular arcs,
site-only/dispersion mechanisms, and raw parametric paths. Existing raw
parametric paths may still produce sites through their current mechanisms but
cannot directly produce faces.

Guide-face outputs retain the fixed `Regions(Full)` response, no output delta
or property descriptor, the current single-output authoring gate, and final
canvas clipping only. Capability projection distinguishes `OrdinaryVoronoi`
from `GuideFaces { guide_mechanism_id, dimensions }`. Editing selected face
dimensions is a Family-invalidating definition edit with existing stale-base,
copy-on-edit, shared-edit, and exact history semantics. The ID-free recipe
wraps only `GeneralizedStraightGuides` and maps ordered dimension indices to
fresh IDs; generic authored curved definitions persist through v5 only.

Geometry builds complete guard-inclusive selected structural paths, splits
line/cubic paths at deterministic valid crossings, embeds sorted half-edges,
and retains every complete positive bounded face relevant to the canvas. The
canvas never closes topology. It rejects overlaps, tangencies, ambiguous
junctions, invalid pieces, non-simple/holed faces, allocation failures, and
limit exhaustion atomically. Canonical region source identity uses sorted
unique structural boundary provenance; canonical rings preserve the exact face.
Analytic area, bounds, and centroid are deterministic. Cancellation reports
`evaluation.cancelled`; other diagnostics use `region.guide_faces.*`.

The geometry module shares a compatibility arrangement core with Stage 20M,
while preserving Stage 20M result and fingerprint bytes exactly. Per-output
realization/cache keys include the guide-face contract, selected guides,
structural paths, requested coverage, and limits. Renderers consume canonical
closed regions and only fill/clip them.

Default nonzero limits are 1,048,576 source paths and retained faces;
8,388,608 source segments, contacts, split segments, vertices, and retained
ring segments; 16,777,216 directed half-edges; and 67,108,864 inspections.

Verification includes domain/persistence authority and history cases,
arrangement correctness/cancellation/limits, output-cache and stale-publication
tests, Stage 20M/20O replay checks, and canonical PNG/SVG parity. Both a
two-guide rectangular arrangement and a phase-aligned 0/60/120 three-guide
arrangement must produce canonical regions and native PNG/SVG evidence; a
authored curved and curved-off-canvas arrangements add coverage. Native
artifacts use `assets/raster-sample.png` at 1024x1024 and
`assets/vector-sample.svg` at 900x620 under `target/validation/stage20p/`.

The writer stopped first at **Implemented awaiting review**. Independent
read-only review and parent verification passed, including direct inspection
of every native and SVG-rasterized representative. User acceptance on
2026-08-25 moves the stage to **Accepted awaiting checkpoint**. The
implementation checkpoint remains pending; this does not authorize
publication or Stage 20Q, which remains separately gated.
