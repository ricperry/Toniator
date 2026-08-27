# Stage 20 Final Architecture and Concurrency

Status: **Ready for user acceptance in the 2026-08-27 working tree.** This
closeout has no commit or checkpoint hash. It is subordinate to the protected
project specifications and the accepted Stage 20 plans. It does not authorize
publication, the GTK4/Blueprint re-baseline, or Stage 21.

## Final authority path

Stage 20 has one headless evaluation authority:

```text
Document plus effective channel intent
    -> typed PatternDefinition and ordered output settings
    -> immutable family and structural products
    -> deterministic output dependency plan and site-use filters
    -> independently keyed output realizations in dependency order
    -> canonical marks, strokes, and positive regions
    -> RenderScene in authored painter order
    -> final-consumer clipping
    -> preview, PNG, or SVG
```

The domain owns authored intent, stable IDs, commands, history, applicability,
and invalidation. Geometry owns canonical curves, offsets, arrangements, and
positive regions. Patterns owns family/mechanism evaluation, site usage,
sampling, and canonical realization. The engine coordinates one complete
evaluation, accepted-cache reuse, latest-only cancellation, and transactional
publication. IO persists only current authored v5 document/v3 preset intent.
Render consumes canonical geometry and never constructs or repairs topology.
CLI and the existing app project the same domain/evaluator authority; neither
selects behavior by recipe name or presentation metadata.

## Integrated stage matrix

| Stage | Result | Final integrated contribution and supersession |
| --- | --- | --- |
| 20A | Repaired | Deterministic bounded random/dispersion sites remain patterns-owned. Visible-mark exclusion again derives separation from twice the active maximum realized support plus authored margin; the retired independent sizing selector remains absent. |
| 20B | Pass | Canonical line/cubic `CurvePath` is the shared construction and consumer boundary. |
| 20C | Pass | Authored open paths and closed shapes remain document-owned, stable-ID resources with validated conversion only. |
| 20D | Repaired | Generalized guide dimensions/products remain typed; guide capability flags now describe only actually applicable repetition/product behavior. |
| 20E1 | Repaired | Normalized mark fill and maximum-support planning remain authoritative; the final model uses output-keyed responses and current visible-margin exclusion. |
| 20E2 | Pass | Authored closed-shape marks transform per site into canonical closed paths with exact resource/history authority. |
| 20F | Pass | Existing GTK private drafts remain command/history adapters only; no Stage 21 workflow was introduced. |
| 20G | Pass | Document base plus channel replacement/deltas resolve through one effective-pattern projection; recipe replacement prunes incompatible deltas and Undo restores exact prior intent. |
| 20H | Repaired | Typed feature flags remain broad support facts while active `PropertyDescriptor` records exclusively own controls, bounds, choices, applicability, commands, and invalidation. |
| 20I | Repaired | Connected responses produce canonical variable-width filled outlines; stroke profile and outline limits are enforced request-wide across outputs. |
| 20J | Repaired | NormalOffset and reusable offset geometry preserve positive parallel-centerline semantics, cusp/reversal gaps, deterministic cleanup, bounded request work, and cancellation. Stage 20S later replaced the historical absolute ConstantGap region UI vocabulary with normalized positive Scale/UniformOffset fill. |
| 20K | Pass | Finite round/square spirals, raw paths, and equal-arc sites remain deterministic, bounded, cancellable, and current-v5 intent only. |
| 20L | Pass | Site adjacency remains deterministic, guard-aware, mechanism-neutral derived state and is neither authored nor persisted. |
| 20M | Repaired | Connection and maze realizations retain accepted identities and global deterministic traversals; cache contracts are output-ID scoped and request-wide stroke budgets are atomic. |
| 20N | Repaired | Ordered keyed output/settings authority and canonical positive regions remain foundational. Stage 20R supersedes its historical one-output gate. |
| 20O | Pass | Ordinary Voronoi uses eligible immutable site sets, guard-inclusive topology, duplicate co-ownership, private Spade types, stable IDs, and final clipping only. |
| 20P | Repaired | Two-/three-guide faces retain deterministic arrangement authority; ordered dimension indices must be unique, increasing, and in bounds. |
| 20Q | Repaired | Final region realization samples complete untreated regions, then applies normalized positive Scale/UniformOffset treatment. AreaAverage uses source-edge clamping, associated RGB/alpha once, request-wide work limits, and deterministic indexed parallel integration. |
| 20R | Repaired | Ordered heterogeneous outputs, `All`/`SitesUsedBy`/`SitesUnusedBy`, dependency order distinct from painter order, keyed cache units, and transactional composite limits remain authoritative. |
| 20S | Repaired | The capability/descriptor surface and strict ordinary recipe reconstruction remain current. The catalog stays exactly 16 cards. `regions-plus-marks` remains retired; mixed region/mark coverage is direct authored test/evidence data only. |

## Confirmed cross-stage repairs

- Restored the Stage 20E1 visible-mark margin contract without resurrecting
  its retired sizing-policy selector. Support planning, fingerprints,
  descriptors, persistence, commands, and separation tests use one current
  margin-only intent.
- Corrected descriptor applicability for output filters, connected responses,
  generic guides, and Guide Faces. The app regression now dispatches connected
  thickness through its current `ChannelOutput` delta authority.
- Removed remaining current-gate dependence on superseded pre-release document
  APIs and deleted `*-v1.toniator` fixture names. Current tests exercise
  container v1/document schema v5 only; obsolete document schemas reject.
- Hardened immutable container-v1 topology before the ZIP reader can collapse
  or ignore raw records. The loader validates the EOCD-declared central span,
  every raw record boundary, and scanned-versus-declared cardinality; a hostile
  duplicate record whose EOCD count remains two now rejects transactionally.
- Corrected cache identity so remaining request budget is not semantic identity,
  connection/maze contracts are output scoped, accepted warm family reuse
  reports `Hit`, and performance facts never enter cache units.
- Enforced transformed-segment, stroke-profile, stroke-outline, and composite
  budgets across the complete request rather than independently per output.
- Added cancellation polling to scaled region segments and full-frame raster
  composition/background/quantization. Parallel failures or cancellation
  return no partial geometry, raster, cache transaction, or accepted result.
- Kept the 16-card recipe registry unchanged. The natural mixed-output witness
  is an ordinary serialized v5 document assembled directly in engine evidence,
  not a recipe or wizard card.
- Reconciled obsolete integrated-gate expectations with final Stage 20
  authority: centered-local grid/shape placement, output-keyed realization
  identities, current normalized-fill fixtures, and final artifact-density
  policy now have exact current regression assertions rather than historical
  pre-release fingerprints.

## Concurrency model

The scheduler still coordinates one authoritative complete-document evaluation
at a time and preserves latest-only acceptance. CPU-heavy independent work
inside that evaluation uses Rayon's bounded shared pool. The default pool
tracks available hardware; `RAYON_NUM_THREADS` or an explicitly installed test
pool provides a deterministic diagnostic worker count. No site, region, path,
or pixel creates its own OS thread.

Indexed parallel work is used for:

- per-site circle and typed mark realization, including authored shapes and
  source-colored marks;
- reference-point sampling and prepared AreaAverage integration per untreated
  region;
- independent Scale/identity region treatment components;
- per-pixel model composition, ordinary source-over composition, background
  application, and final sRGBA quantization.

Each parallel iterator collects in input index order. Fingerprints, stable IDs,
paint correspondence, canonical ordering, SVG element order, and painter order
therefore do not depend on worker completion order. Request-wide work
allocation, unordered-error selection, canonical grouping/reduction, cache
mutation, and publication remain serial and deterministic.

The following stay intentionally serial:

- random/exclusion generation because the stable PRNG and accepted-neighbor set
  evolve in deterministic order;
- one global Voronoi triangulation or guide planar arrangement;
- maze/tree traversal and connection selection where the accepted topology
  evolves globally;
- dependency-DAG construction, request-wide budget accounting, output/channel
  coordinator traversal, fingerprints, and cache publication;
- painter-order primitive rasterization inside a layer and SVG serialization.

Independent per-output/channel work is not launched as competing complete
evaluations. Current profiles show that the safe inner region/mark/pixel work
already consumes the shared pool, while connection/global topology and ordered
raster costs do not benefit from naive output fan-out. This retains bounded
memory and exact error/cancellation order.

## Diagnostic performance surface

`evaluate_profiled_with_limits` returns the ordinary result plus evaluation-
local records for Preflight, SourceDecode, Family, DependencyFilter, typed
Mark/StructuralPath/Connection/Maze/Voronoi/GuideFace realization,
RegionSampling, RegionTreatment, Scene, Raster, and Total. The stateful
`evaluate_profiled_cached_with_limits` seam accepts cache transactions only
after complete success, so cold, warm, and targeted-edit profiles use the
ordinary cache authority. Records distinguish computed work, accepted-cache
hits, and within-request family reuse, and include inexpensive deterministic
counts for source/raster pixels, outputs, sites, paths, marks, strokes,
profile/outline work, regions, boundary segments, and AreaAverage
flattening/cell intersections. Metrics also report configured pool width,
distinct Rayon workers observed through existing cancellation polls, and one
cheap registration per participating worker. Fine-grained polls do not
perform per-poll atomics.
Typed output counts distinguish marks, structural paths, connections, maze,
Voronoi, and Guide Faces. Timings, worker participation, and
workloads are diagnostic only: they are absent from persistence, cache keys,
cache transactions, identities, and ordinary `EvaluationResult`.

The ignored release diagnostic
`stage20_closeout_release_profile` selects an ordinary catalog ID and canvas
size with environment variables and prints cold and accepted-warm architectural records,
including observed worker participation.
Exact test assertions compare semantic output and stable workload/cache facts,
not wall-clock duration.

Representative same-code one-worker versus eight-worker release results:

| Workload | Stable work | 1 worker | 8 workers | Interpretation |
| --- | --- | --- | --- | --- |
| Source-weighted dispersion Voronoi, 512 square | 3 outputs; 2,695/2,692/2,633 regions; 16,164/16,153/15,819 boundary segments; 1,904,481/1,896,113/1,888,755 AreaAverage cell intersections | Total 3.985 s; three sampling spans 1.083/1.092/1.063 s; raster 0.619 s | Total 1.210 s; sampling 0.152/0.162/0.152 s; raster 0.612 s; all 8 workers observed | AreaAverage was the material avoidable serial bottleneck: 3.29x total and about 6.9x sampling speedup. Ordered rasterization is now dominant. |
| Straight-grid circles, 512 square | 3,469 sites and marks plus 122 structural paths per channel | Total 216.7 ms; mark spans 3.19/2.75/2.79 ms; raster 169.3 ms | Total 196.3 ms; mark spans 1.59/1.34/1.33 ms; raster 156.3 ms | Mark realization scales, but raster dominates this ordinary card. |
| Even-random circles, 512 square | 3,298 ordered random sites and marks per channel | Total 308.6 ms; family 30.3/29.8/29.7 ms; marks 3.41/2.19/2.35 ms; raster 183.1 ms | Total 288.8 ms; family 30.4/32.7/31.6 ms; marks 1.99/1.33/1.31 ms; raster 160.7 ms | Ordered PRNG/exclusion is intentionally serial; per-site realization scales but is a small share. |
| Authored diamond marks, 256 square | 965 marks and 133 structural paths per channel | Total 123.6 ms; realization 1.51/1.32/1.39 ms; raster 88.8 ms | Total 124.3 ms; realization 1.04/0.89/0.76 ms; raster 88.6 ms | Authored-shape transformation is parallel; total remains raster/source-decode dominated. |
| Clustered random links, 256 square | 2,325 sites/channel; 783/794/787 strokes and identical profile/outline work | Total 290.8 ms; connection realization 44.5/46.4/49.6 ms; raster 112.2 ms | Total 282.5 ms; connection realization 47.9/44.1/44.3 ms; raster 108.5 ms | Global connection selection and ordered stroke/raster work remain serial; thread count alone correctly gives no meaningful scaling. |

The final source-weighted profile's accepted-warm invocation was 19.2 ms with
one worker configured and 21.6 ms with eight: source, family, Voronoi, scene,
and raster records were accepted cache hits and no worker registered. Native
output encoding for the identical 598,531-byte PNG and 4,417,918-byte SVG took
2.5 ms and 22.0-22.9 ms respectively. PNG and SVG are caller-selected outputs,
so there is no production invocation that waits for both; parallelizing their
serialization would add a second authority path without accelerating a real
consumer.

An initial diagnostic implementation atomically counted every worker-side
cancellation poll. The same eight-worker workload regressed to 4.882 s while
recording 136,318,694 polls. Replacing that counter with one thread-local-gated
registration per participating worker restored the 1.210 s result while still
observing all eight workers. This keeps the opt-in instrumentation lightweight
at the actual fine-grained sampling boundary.

Earlier whole-command pre-instrumentation readings were 2.28 s for selected
straight-grid circles, 1.40 s for source-weighted dispersion Voronoi, and
2.07 s for a derived round-spiral line. Those values include artifact/export
overhead and are retained only as coarse provenance, not a like-for-like
benchmark. Large generic one-guide/spiral profile requests correctly stop at
the request-wide 262,144 stroke-profile limit; validation uses documented
bounded derivatives rather than raising the product limit.

## Persistence and recipes

The immutable container layout remains version 1, the only current document
schema is v5, and presets are v3. Current saves are deterministic and strict;
obsolete document schemas reject instead of migrating. Authored structures,
stable output IDs/order, filters, responses, deltas, region source/treatment/
sampling intent, and embedded source bytes round-trip. Evaluated sites,
adjacency, regions, usage sets, effective projections, caches, diagnostics,
timings, worker state, scheduler state, and drafts do not persist.

The registry remains version 2 with exactly 16 ordinary v3 records. Capability
classes cover guides, circular/authored marks, parametric paths/sites,
connections, maze, Voronoi, Guide Faces, normalized Scale/UniformOffset,
ReferencePoint/AreaAverage sampling, and residual-site filters/composition.
Presentation permutations and the retired `regions-plus-marks` debug tool are
not catalog records.

## Verification and evidence

The final integrated gate includes formatting, all-target check/Clippy/tests,
architecture validation, diff checks, current persistence/capability/recipe/
geometry/sampling/composite/cache/scheduler suites, exact one-worker versus
four-worker semantic equivalence, release profiles, semantic-map refresh, and
protected/immutable-input hashes. Native evidence is under
`target/validation/stage20r/`:

- `connection-paths-1024x1024.png` and raw SVG verify clipped variable-width
  connection outlines and visible independent RGB layers;
- `area-average-regions-900x620.png` and raw SVG verify clean sampled positive
  regions, edge coverage, and transparent gaps;
- `authored-regions-and-marks-900x620.{toniator,png,svg}` verifies ordinary
  direct-authored mixed outputs and region-below-mark painter order without a
  catalog recipe;
- separately rasterized SVGs verify vector correspondence; native RGB and
  alpha inspections distinguish hidden RGB, coverage, and viewer background.

The app production workflow was not changed. Current app test fixtures were
ported mechanically to final headless authority, so a private Wayland GTK run
is not an applicable acceptance boundary for this headless closeout.

## Deferred boundaries

The GTK4-only/Blueprint re-baseline, Pattern Wizard/gallery UI, Stage 21
pattern authoring, all media/sequence/temporal stages, GPU work, new topology
families, wall complements, and compatibility/import remain outside Stage 20.
The final Stage 20 headless surface is ready for the separately authorized GUI
re-baseline and later Stage 21 only after user acceptance of this working tree.
