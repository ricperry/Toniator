# Stage 20E2 — User-Shape Mark Realization

## Status and dependency gate

**Complete at commit `0c6b6a2e268f9306835038be747352a0cd64044c`.** The
bounded implementation and focused verification satisfy this contract. An
independent read-only review found three
sampled-paint and identity gaps; the bounded repair, repair re-review, and final
engine-to-render zero-alpha witness close them without confirmed remaining
findings. The user explicitly accepted Stage 20E2 on 2026-08-14, and the local
implementation checkpoint contains the reviewed implementation and deliberate
HolidayMugs fixture/checksum update. No Stage 20F+ work is authorized by this
transition.

## Outcome

Realize one document-owned authored `ClosedShape` as an ordinary filled mark at
each existing `FamilySiteSet` site. The family remains the sole placement and
provenance authority. Preview, PNG, and SVG consume the same canonical mark
geometry; neither renderer resolves authored resources or dispatches on artistic
pattern names.

## Domain and realization contract

- Add `MarkPrototype::AuthoredClosedShape { structure_id }`, its typed kind,
  singular stable-reference descriptor/current value, explicit atomic set and
  retarget commands, history behavior, and active-variant validation. Switching
  from Circle requires an explicit existing `ClosedShape` ID; never choose a
  resource implicitly.
- Permit one shared structure reference per output layer for every current site
  family. Definition duplication retains the reference. Resource duplication
  allocates a new ID without retargeting definitions. Missing, wrong-kind,
  stale, remove-while-referenced, replacement, affected-channel, no-op, undo,
  and redo behavior remains failure-atomic. Closed-shape content/reference
  changes report `Realization` invalidation.
- Convert the stored structure to exact closed `CurvePath`, retain segment order
  and closure, and anchor it at its canonical bounds center. Its conservative
  reference radius is the maximum distance from that anchor to every line
  endpoint and cubic endpoint/control point. Reject a zero-radius prototype;
  retain finite nonzero-extent zero-area paths.
- Uniformly scale the prototype so its reference diameter equals the Stage 20E1
  per-site resolved mark diameter. Apply output Fixed/Tangent/Normal orientation,
  then channel rotation offset, then translate the anchor to the site. Preserve
  authored winding and render with explicit even-odd fill. Self-intersections
  are supported; there are no holes/multiple contours in this single-structure
  stage.
- Generalize canonical output to ordered circle or closed-path marks carrying
  `FamilySiteId`, scope, provenance, finite bounds, and fill semantics. Source
  response changes scale/paint only; it never changes path topology or sites.

## Rendering, limits, identity, and persistence

- Rasterize exact canonical line/cubic paths through deterministic adaptive
  flattening with at most 1/64 output-pixel flatness error, then the accepted
  8x8 coverage contract and final canvas clip. SVG writes editable cubic `<path>`
  geometry with `fill-rule="evenodd"` and the existing canvas clip. Renderer
  output owns final clipping and never rewrites a mark into canvas-created
  topology.
- Add nonzero evaluation limits of 1,048,576 transformed curve-segment instances
  and 4,194,304 flattened raster edges. Preflight checked site/segment products,
  poll cancellation during site, segment, subdivision, and raster work, and
  publish no partial realization, scene, or cache transaction.
- Realization identity includes resource ID plus resolved content, normalization
  and fill contract, per-site nominal diameter, channel response/orientation, and
  source-derived inputs. Family identity remains unchanged by shape content.
  Canonical path bits and fill rule participate in scene identity.
- Extend the Stage 20E1 current document/preset schemas additively for the
  authored-shape variant. Do not restore a superseded decoder or introduce a
  compatibility migration. CLI validate/render/inspect and the app consume
  shape-bearing documents, but authoring remains Stage 20F.

## Verification and boundary

Focused domain, geometry, patterns, render, engine, IO, and CLI tests cover
resource/reference transitions, sharing, exact normalization/transforms,
orientation, self-intersection, zero-radius rejection, zero-area behavior,
limits, cancellation/supersession, cache reuse/misses, deterministic round-trip,
raw native PNG, and editable structural SVG. Exercise both immutable source
artworks at intrinsic dimensions under `target/validation/stage-20e2/`.

The future implementation allowlist is the affected headless crates, CLI,
narrow app compile/consume handling, focused tests, current document fixtures,
durable stage ledgers, and validation/evidence directories. Protected
specifications, immutable source artwork, Legacy, shape-authoring GTK, response
curves/polarity, paths/strokes, regions/topology, composites, and Stage 20F+
implementation remain excluded. One writer stops at `Implemented awaiting
review`; an independent read-only review and explicit user acceptance are
required before any transition.
