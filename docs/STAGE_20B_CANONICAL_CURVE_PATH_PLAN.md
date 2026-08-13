# Stage 20B execution contract: Canonical Curve/Path Geometry

## Status, authority and goal

**Complete in the Stage 20B acceptance checkpoint.** The user approved this bounded contract on
2026-08-13 from documentation checkpoint `e7e2dca`, whose direct implementation
parent is the accepted Stage 20A checkpoint `b7fbd81`. The bounded
implementation and focused gate are complete and the independent read-only
review passes. User acceptance is complete. The single acceptance checkpoint
includes the implementation, the authorized current-format real-world
`.toniator` fixture, and durable documentation; this contract intentionally
does not invent a self-referential checkpoint hash. Stage 20C is the next
planning-only boundary and is not started.

Stage 20B adds a finite, deterministic, geometry-owned foundation for connected
line and cubic Bézier paths: explicit line/polyline/cubic construction; open
and closed continuity; segment-local point/tangent/normal evaluation;
analytical or conservative bounds; reusable arc-length measurement and inverse
lookup; line/line, line/cubic and cubic/cubic intersections; and ordered
clipping to finite axis-aligned bounds.

The user rescinded mandatory semantic-map use when approving this contract.
Semantic-map is optional and may be used only when it is actually useful and
more efficient than direct source, `rg`, Cargo, Git, or the repository
architecture validator. No semantic-map cache refresh or query is required by
this checkpoint.

## Non-goals and preserved authority

Stage 20B adds no document state, persisted path/guide schema, IDs, commands,
descriptors, history, migration, presets, invalidation, cache keys, curved
family evaluator, guide repetition/coverage, along-curve sites, strokes,
paint, tessellation, renderer/SVG integration, graph, maze, face, region,
Voronoi, offset, composite output, GTK, or artistic preset. It does not begin
Stage 20C or any later checkpoint and adds no compatibility for superseded
in-development formats.

Preserve Stage 20A `FamilySiteSet`, opaque `TypedFamilyOutput`, truthful site
provenance, the private current-circle adapter, every accepted identity/cache
boundary, canonical circles, render/export, CLI, GTK, protected specifications,
immutable assets, and Legacy. The formerly user-owned
`assets/HolidayMugs_2024_2025.toniator` is tracked only as an authorized
current-format acceptance test case; it is not curve-geometry authority.

Construction geometry and render-canonical output remain distinct. Stage 20B
introduces `CurvePath`, not `CanonicalPath` or a new `GeometryOutput` branch.
The new geometry types acquire no serialization, IDs, provenance, style,
stroke, winding, or renderer semantics.

## Public geometry vocabulary

All struct fields are private. Public access uses validated constructors and
read-only accessors.

```rust
pub enum PathClosure { Open, Closed }

pub struct LineSegment { /* start, end */ }

pub struct CubicBezierSegment {
    /* start, control_1, control_2, end */
}

pub enum CurveSegment {
    Line(LineSegment),
    CubicBezier(CubicBezierSegment),
}

pub struct CurvePath { /* ordered segments, closure */ }
pub struct PathLocation { /* segment_index, parameter */ }
pub struct PathArcLength { /* immutable ordered measurement table */ }

pub enum IntersectionKind { Crossing, Tangent }

pub struct SegmentIntersection {
    /* first_parameter, second_parameter, point, kind */
}

pub struct PathIntersection {
    /* first_location, second_location, point, kind */
}

pub struct CurveError { /* stable path and fixed message */ }
```

Required constructors and operations:

```rust
LineSegment::new(start, end)
CubicBezierSegment::new(start, control_1, control_2, end)

CurvePath::new(segments, closure)
CurvePath::line(start, end)
CurvePath::polyline(vertices, closure)

CurveSegment::{start, end, point_at, unit_tangent_at, unit_normal_at}
CurveSegment::{bounds, arc_length, intersections, transformed}

CurvePath::{segments, closure, start, end}
CurvePath::{point_at, unit_tangent_at, unit_normal_at}
CurvePath::{bounds, measure_arc_length, intersections}
CurvePath::{clip_to_bounds, transformed}

PathArcLength::{total_length, location_at_length}
CurveError::{path, message}
```

## Finite geometry, topology and continuity

- Every coordinate and control point must be finite.
- General path construction requires at least one segment and accepts no more
  than 4,096 segments.
- Adjacent endpoints must be exactly equal under finite `f64` equality.
  Tolerance never silently joins structural endpoints.
- Path continuity is C0 only. C1 continuity, smoothing, joins and corner
  treatment are later-stage behavior.
- `Open` remains open even when its final point equals its start.
- `Closed` requires the final endpoint to equal the initial start.
  `CurvePath::new` never manufactures a closing segment.
- `CurvePath::polyline(..., Closed)` is the one explicit convenience that adds
  the final vertex-to-first line when absent. This is authored closure
  semantics, never canvas-created closure.
- Duplicate vertices, zero-length lines, stationary cubics, two-edge closed
  paths, and fully coincident finite paths are valid geometry.
- There is no global normalized path parameter. `PathLocation` is an explicit
  segment index plus `t` in `[0, 1]`.
- Join evaluation is segment-local; tangents are never averaged.

## Fixed numerical policy and limits

These are implementation constants, not authored or caller-adjustable values.

| Policy | Value |
| --- | ---: |
| Absolute geometric tolerance | `1.0e-9` document units |
| Relative tolerance | `64 * f64::EPSILON` (`~1.4210854715202004e-14`) |
| Parameter tolerance | `1.0e-12` |
| Maximum subdivision depth | `48` |
| Maximum subdivision work items per public operation | `262,144` |
| Maximum arc-length leaves | `65,536` |
| Maximum segment pairs per path intersection query | `262,144` |
| Maximum returned intersections | `4,096` |
| Maximum clipping fragments | `4,096` |
| Maximum total clipped output segments | `65,536` |

Geometric tolerance is `absolute + relative * scale`, where scale is at least
one and otherwise derives from the operation's coordinate or geometric extent.
Derivative/stationary classification uses control-polygon length rather than
absolute document position. Every checked arithmetic or intermediate
non-finite result fails. Depth, work, pair, leaf, fragment and result
exhaustion return errors without partial output.

## Evaluation, bounds and arc length

- Point evaluation uses the exact line or cubic Bernstein formula for
  `t` in `[0, 1]`.
- Tangents normalize the analytic derivative. A derivative within the
  stationary threshold returns `curve.path.tangent.stationary`.
- Normals are the left-hand perpendicular `(-tangent.y, tangent.x)`.
- Lines use exact endpoint bounds.
- Cubics solve derivative quadratics for X/Y extrema in `(0, 1)`, evaluate
  those extrema, and conservatively expand for floating-point tolerance.
- Path bounds are the deterministic union in segment order.
- Lines use exact `hypot` length. Cubics use ordered adaptive de Casteljau
  subdivision and converge when control-polygon length minus chord length is
  within geometric tolerance. Accepted leaf length is the chord/control-
  polygon average. Path summation uses compensated accumulation.
- Inverse lookup is monotone over `[0, total]`. Exact joins choose the earliest
  topological location; zero maps to the first segment at `t = 0`, and total
  maps to the last at `t = 1`.
- A fully stationary path has length zero; its sole valid inverse request is
  zero and returns the first location.

## Intersections

- Line/line uses analytical cross products.
- Line/cubic and cubic/cubic use deterministic de Casteljau parameter-box
  subdivision, conservative analytical bounds, fixed split order, and bounded
  refinement.
- Results sort by first path location, then second path location.
- Parameters within parameter tolerance snap to exact zero or one.
- Equivalent adjacent-segment endpoint occurrences canonicalize to the
  lexicographically earliest topological location; the closed seam
  canonicalizes to the first segment at `t = 0`.
- Deduplication requires equivalent locations and geometric coincidence, so
  distinct visits at a self-crossing remain distinct.
- A transverse nonstationary derivative pair is `Crossing`; unique collinear,
  stationary, endpoint, or derivative-parallel contact is `Tangent`.
- A positive-length coincident interval returns
  `curve.path.intersections.overlap`; no partial discrete results survive.
- Zero-length point/point or point/curve coincidence is one tangent, not an
  overlap.

## Clipping and transforms

- `clip_to_bounds` accepts only finite ordered `Bounds`.
- Lines use Liang-Barsky parameter clipping.
- Cubics isolate roots against all four bound coordinates, split with de
  Casteljau, and classify ordered parameter intervals by midpoint.
- Existing boundary geometry is included. An isolated outside tangency at an
  edge or corner produces no artificial point fragment.
- Crossing endpoints snap only the applicable coordinate to the exact bound.
- Output preserves source direction, segment kind and stored segment order.
- Fully contained input returns an exact clone and retains `Closed`.
- Every actually clipped fragment is `Open`.
- A partially clipped closed path is not seam-merged. No rectangle edge,
  connector, closing segment, face, or other topology is created.
- Transformation uses the existing `AffineTransform2D` authority. Stage 20B
  does not alter its accepted rotation-about-center plus document-axis-
  translation behavior or add scale/shear constructors.

## Stable diagnostics

The implementation uses these fixed paths and fixed literal messages:

```text
curve.segment.coordinates
curve.path.segments.empty
curve.path.segments.limit
curve.path.polyline.vertices
curve.path.continuity
curve.path.closure
curve.path.location.segment
curve.path.location.parameter
curve.path.numeric_overflow
curve.path.transform.non_finite
curve.path.tangent.stationary
curve.path.arc_length.distance
curve.path.arc_length.subdivision_limit
curve.path.arc_length.result_limit
curve.path.intersections.overlap
curve.path.intersections.pair_limit
curve.path.intersections.subdivision_limit
curve.path.intersections.result_limit
curve.path.clipping.bounds
curve.path.clipping.subdivision_limit
curve.path.clipping.fragment_limit
curve.path.clipping.segment_limit
```

## Exact file allowlist and ownership

Parent-owned contract/status files:

- `docs/STAGE_20B_CANONICAL_CURVE_PATH_PLAN.md`;
- `docs/GREENFIELD_REWRITE_PLAN.md` for Stage 20B status only;
- `ProgressTracker.md` for Stage 20B status only;
- `.codex-work/agents/test-reviewer/2026-08-13-stage20b-readonly-review.md`
  when persisting the read-only review's `CACHE_UPDATE`.

Exactly one `desktop_implementer` is the sole implementation/test/evidence
writer and owns only:

- `crates/toniator-geometry/src/lib.rs` for a module declaration and public
  re-exports, without changing existing function bodies;
- `crates/toniator-geometry/src/curves/mod.rs`;
- `crates/toniator-geometry/src/curves/segment.rs`;
- `crates/toniator-geometry/src/curves/path.rs`;
- `crates/toniator-geometry/src/curves/arc_length.rs`;
- `crates/toniator-geometry/src/curves/intersections.rs`;
- `crates/toniator-geometry/src/curves/clipping.rs`;
- `crates/toniator-geometry/tests/curve_paths.rs`;
- `.codex-work/agents/desktop-implementer/2026-08-13-stage20b-canonical-curve-path-geometry.md`;
- `.codex-work/semantic-map/USAGE_EVALUATION.md` only if semantic-map is
  actually attempted and demonstrates another inefficiency.

No manifest or dependency change is permitted. Every other path is excluded.

## Focused tests and verification

`crates/toniator-geometry/tests/curve_paths.rs` contains these named tests:

```text
line_polyline_and_cubic_paths_preserve_explicit_topology
curve_evaluation_tangent_normal_and_bounds_cover_extrema_and_degeneracy
path_arc_length_and_inverse_lookup_are_deterministic_monotone_and_bounded
path_intersections_order_deduplicate_and_classify_crossings_tangencies_and_overlaps
path_clipping_preserves_ordered_fragments_without_inventing_boundary_topology
curve_operations_remain_consistent_under_existing_affine_transforms
curve_failures_use_stable_paths_and_never_return_partial_results
```

Property-style deterministic loops cover transformed samples, sampled bounds
containment, inverse-length monotonicity, stable ordering, clipped-point
containment, and repeat-run equality. Focused internal tests with reduced
private budgets prove subdivision/leaf exhaustion without expensive public-
limit runs.

Every touched non-trivial named Rust function, method and test receives literal
`///` responsibility documentation describing its present authority,
invariants/bounds, side effects, and applicable Errors/Panics/Safety.

The focused gate is:

```bash
cargo fmt --all -- --check
cargo test -p toniator-geometry --test curve_paths
cargo test -p toniator-geometry --test primitives
cargo check -p toniator-geometry -p toniator-sampling -p toniator-patterns -p toniator-render --all-targets
cargo clippy -p toniator-geometry --all-targets -- -D warnings
bash scripts/validate_architecture.sh
git diff --check
git diff --exit-code -- Cargo.toml Cargo.lock crates/toniator-geometry/Cargo.toml
git diff --exit-code -- ToniatorLegacy 'Project Specification' assets
git status --short --branch --untracked-files=all
sha256sum assets/HolidayMugs_2024_2025.toniator
```

Stage 20B neither loads sources nor renders/exports output, so the immutable
PNG/SVG natural-input gate, validation artifacts, GTK/Sway evidence, visual
review and manual desktop acceptance are excluded.

## Review, acceptance, checkpoint and stop gates (historical execution record)

1. The parent rechecks the approved start gate and records only Stage 20B
   **In progress**.
2. Exactly one `desktop_implementer` implements and verifies the allowlist.
3. One `test_reviewer` performs a read-only review of numerical correctness,
   error atomicity, bounds/limits, topology preservation, documentation,
   dependency direction and scope.
4. Any correction returns to the same `desktop_implementer`; no second
   implementation writer is created.
5. After passing re-review, the parent records only Stage 20B as
   **Implemented awaiting review**, persists checkout-aware evidence, and
   stops uncommitted for user review.
6. Only explicit user acceptance may advance the stage to
   **Accepted awaiting checkpoint**. A local implementation commit,
   documentation closeout, push, or later stage each requires separate
   authorization.

The executed closeout supersedes the intermediate statuses above: Stage 20B is
complete in the single acceptance checkpoint, which contains all tracked Stage
20B implementation, fixture, and durable documentation. Stage 20C is not
started and requires a fresh planning-only contract and explicit user approval
before implementation.

If implementation reveals a material vocabulary/numerical decision, needs a
dependency or excluded file, cannot satisfy the fixed tolerance/limit
contract, or threatens an accepted authority, stop with the worktree
preserved. Record the exact decision, source/test evidence, affected boundary,
and smallest viable choices. Ordinary failures within the allowlist are
corrected by the same writer and do not authorize scope expansion.
