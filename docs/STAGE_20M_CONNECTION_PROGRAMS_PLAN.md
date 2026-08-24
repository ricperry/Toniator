# Stage 20M — Connection Programs and Grid Wall Mazes

## Status and authority

**Complete at implementation checkpoint
`33f1bde3be9afdc3fb88f479c4ee7ec52b80114a`.** The user authorized this bounded
implementation on 2026-08-23 and accepted it on 2026-08-24. The user clarified that `GridMaze` means a conventional wall maze whose
solution traverses cells formed by site-derived walls. This contract is
subordinate to the five protected files under `Project Specification/`, with
`Addendum.md` taking precedence, and to the approved ordering in
`docs/GREENFIELD_REWRITE_PLAN.md`.

Stage 20M adds document-authored deterministic positive connection programs
over the accepted Stage 20L mutual-nearest graph and a distinct conventional
wall-maze program over complete straight-guide arrangement cells. It does not
authorize Stage 20N or later work, publication, or a protected-specification
revision; the implementation checkpoint is recorded above. The bounded
straight-grid face/dual-cell primitive
introduced here is reusable topology authority, not Stage 20O region
realization.

The one-writer implementation, focused verification, intrinsic artifact
inspection, independent read-only reviews, bounded repair re-reviews, and the
final centered-origin review found no material findings. A final requirement audit additionally made the
two-/three-guide geometry witness direct and added public connection/maze
capability-projection coverage; neither repair changed production behavior.
The accepted headless scope retains the centered grid-prototype origin, positive
nearest/random/tree paths, conventional two-/three-guide wall mazes,
intent-only current-v4/preset-v2 persistence, normalized `0.0..=2.0` response,
no GTK work, no renderer topology repair, and no Stage 20N work.

## Bounded scope

Add domain-owned positive connection intent:

- `ConnectionProgram::NearestLinks { adjacency }`;
- `ConnectionProgram::RandomLinks { adjacency, minimum_degree, seed }`;
- `ConnectionProgram::GridSpanningTree { adjacency, algorithm:
  RandomizedPrim, seed }`.

`ConnectionAdjacencyIntent` stores `maximum_degree: u32` in `1..=32` and a
finite positive absolute `maximum_distance: f64`. Random minimum degree is in
`0..=maximum_degree`; seeds are `u32`.

Add domain-owned `MazeProgram { algorithm:
GridMazeAlgorithm::RecursiveBacktracker, seed }` with deterministic automatic
pattern-space solution endpoints. The maze has no
generic nearest-neighbour degree or distance controls because those controls
could delete required walls and invalidate arrangement faces.

Add `PatternOutputLayer::ConnectionPaths { id, site_mechanism_id, program,
style }` for nearest, random, and positive spanning-tree programs, plus
`PatternOutputLayer::MazeWalls { id, site_mechanism_id, program, style }` for
the conventional wall maze. Family-producing mechanisms remain unchanged and
realization intent stays in typed output contracts. Stage 20H capability
projection exposes the applicable positive-program degree/distance and
conditional seed/minimum-degree controls, the maze algorithm and seed, round
style, and thickness response without treating wall topology as a generic link
program.

Persist only authored output-layer intent through current document schema v4
and preset v2. Existing current-format bytes remain unchanged. Do not add a
migration or compatibility adapter, and never serialize graphs, walls, faces,
dual passages, solutions, trails, diagnostics, caches, or resource limits.
The centered local grid-prototype transform changes derived family geometry and
identity only; it does not alter authored v4 document bytes. Random distributions
and parametric structural-source geometry/fingerprints remain unchanged.

Geometry owns `ConnectionPathId`, `ConnectionPath`, `ConnectionPathSet`,
`MazeWallId`, `MazeCellId`, complete ordered grid faces and dual adjacency,
`MazeProgramResult`, stable diagnostics, configurable limits, and cancellable
construction. Generalize canonical stroke identity so structural, positive
connection, and maze-wall paths cannot collide, while preserving the exact
existing structural fingerprint bytes.

## Positive connection geometry contract

Convert every positive program's adjacency intent exactly to
`SiteAdjacencyPolicy::MutualNearest`; reject any supplied graph whose policy
does not match the program.

Eligibility is capability-based:

- nearest and random links accept guide intersections, along-guide sites,
  dispersion/random sites, and parametric equal-arc sites;
- spanning-tree programs accept only the typed `GuideIntersections`
  capability, without family-name or provenance-variant dispatch;
- raw parametric paths fail before coverage, graph, or program allocation.

Program selection is deterministic:

- nearest links retain every Stage 20L edge;
- random links derive an inclusive per-node target in
  `minimum_degree..=maximum_degree`, rank edges with a fixed FNV-1a
  seed/endpoint contract, perform bounded best-effort minimum filling, then
  fill toward targets without exceeding the maximum; under-connected and
  isolated nodes are diagnostics, not failures;
- spanning tree runs seeded randomized Prim with deterministic edge priorities
  independently on every component.

Selected edges sort by canonical endpoint IDs. Program kind and seed remain in
identity even when two small-graph selections happen to match.

Decompose selected edges into the minimum number of edge-disjoint open trails:

- pair sorted odd-degree vertices using non-emitted virtual edges, then use a
  deterministic Hierholzer traversal;
- split an all-even nonempty component at its smallest real edge so the
  resulting open path has distinct endpoints;
- orient every trail from the smaller endpoint ID;
- sort by component minimum, endpoints, and edge sequence before assigning
  stable ordinals;
- retain output-layer identity, component identity, ordered endpoints, and
  ordinal in each path ID;
- emit line segments only and cover every selected edge exactly once.

## Grid wall-maze geometry contract

Maze eligibility is the typed straight-guide `GuideIntersections` capability.
Reject random, along-guide, parametric, and curved-guide products before
coverage or topology allocation. Family sites and their guide-instance
contributors are the sole source of wall vertices and wall membership; neither
a family name nor a renderer branch selects maze behavior.

Derive the bounded planar maze arrangement from the actual evaluated family:

- retain every intersection site on or inside the document canvas as candidate
  arrangement and fingerprint authority; reject only sites outside those inclusive
  bounds, without a stroke-width or site-clearance inset;
- group those sites by truthful straight-guide instance contributor;
- connect consecutive sites along each guide to form canonical primal wall
  edges, deduplicating triple-intersection edges by endpoint IDs;
- construct an ordered half-edge embedding and retain every positively oriented
  bounded face, never using the canvas to close a face or create an edge;
- if those finite bounded faces form multiple dual components, select the largest
  component with stable ties; discarded components emit no wall fragment, while
  all in-canvas sites remain candidate and fingerprint authority;
- derive dual cell adjacency only across one shared primal wall;
- reject atomically unless all bounded cells are one connected dual component;
- run seeded recursive depth-first backtracking once to select exactly one
  spanning tree over every cell;
- remove the shared primal wall for each selected dual passage and emit the
  complement of retained wall edges;
- identify arrangement-perimeter walls as walls adjacent to exactly one cell;
- choose exactly two distinct perimeter openings, preferring opposite normalized
  canvas sides and otherwise the greatest normalized separation with stable ties;
- remove those two opening walls and derive one solution as the unique tree route
  between their adjacent cells.

The solution and openings are derived topology, not separately rendered geometry or authored
endpoint intent in this stage. A single-face maze still has two distinct perimeter openings and a
one-cell solution. Canvas-side classification is a deterministic ranking hint only; canvas bounds
admit sites inclusively but never create an edge, face, passage, or endpoint. Positive wall width
and caps may extend past the canvas and be clipped by the existing final consumer boundary; that
spill never invalidates an otherwise in-canvas site. Artist-selected pattern-space endpoints and
optional solution styling remain a compatible later extension.

Every emitted path is positive, finite geometry. Positive connection output
contains selected primal edges; maze output contains retained wall edges and
never renders its dual passages or solution as wall geometry. Canvas bounds select the inclusive
candidate-site arrangement and rank existing perimeter openings, but never create nodes, edges,
walls, faces, passages, endpoints, or closure. Use the minimum retained
nominal-cell diameter across each emitted trail as its constant stroke basis.
Positive-graph isolates emit no path or mark. Crossing behavior is the existing
positive `Junction` result; other crossing treatments are deferred.

Fingerprint positive-program contract IDs, source graph fingerprint, complete
authored program, selected edges, ordered path IDs, and vertex sequences.
Fingerprint maze contract IDs, source family fingerprint, authored program,
ordered source walls and faces, selected dual passages, retained wall IDs,
ordered wall-path IDs and vertex sequences, automatic endpoints, and solution
routes. Exclude limits and diagnostics.

Default nonzero connection limits are:

- 1,048,576 selected edges;
- 1,048,576 trails;
- 2,097,152 retained path points;
- 33,554,432 selection/traversal inspections.

Default nonzero maze limits independently bound 1,048,576 source walls,
1,048,576 faces, 1,048,576 dual adjacencies/passages, 1,048,576 emitted wall
trails, 2,097,152 retained wall points, and 33,554,432 arrangement, traversal,
solution, and fingerprint inspections.

Poll cancellation during selection, arrangement/face walking, dual DFS/Prim
growth, complement and solution construction, trail construction,
fingerprinting, and stroke realization. Cancellation returns
`evaluation.cancelled`. Allocation, work-limit, identity, and geometry errors
use stable `connection.*` or `maze.*` diagnostics and expose no partial result.

## Pipeline and cache contract

Patterns owns typed capability validation and guard-inclusive orchestration.
Required positive-connection family support is the existing connected-stroke
base support plus `guard_steps * maximum_distance`. Maze support includes the
connected-stroke base plus enough complete guide intervals for one outer ring
of cells at every active guide dimension. Every active connection or maze
layer requires at least one guard step. A broader cached family envelope is
deterministically subset to the exact requested program envelope before graph
or arrangement construction, so cached guard-only sites cannot perturb visible
seeded topology.

Engine integration adds no adjacency, connection, arrangement, or maze cache.
Family keys remain based on family mechanisms and required support, so broader
envelopes satisfy narrower requests. Program intent, algorithm contract IDs,
adjacency/connection/maze limits, and stroke limits enter realization cache
keys. Program seed or type changes miss realization while preserving ordinary
family identity and site positions.

All authored program edits conservatively report `Family` invalidation under
the Stage 20+ decomposition. Connected or wall thickness remains
`Realization`, and paint/opacity retain their existing downstream invalidation.
Cancelled or stale scheduler candidates cannot publish paths or alter accepted
caches.

Renderers receive canonical strokes only. They do not inspect graphs or faces,
select passages, infer wall complements, or repair topology.

## Verification and acceptance

Focused geometry coverage includes all three positive programs, policy
mismatch, degree/distance validation, same-seed replay, different-seed
witnesses, best-effort diagnostics, disconnected components, isolates, cycles,
branches, minimum open-trail counts, exact single-edge coverage, path
identity/order, fingerprints, cancellation, allocation, and every resource
limit.

Maze geometry coverage includes two- and three-guide arrangements, canonical
walls and bounded faces, dual adjacency, same-seed replay, different-seed
witnesses, atomic disconnected-dual rejection, inclusive candidate-site authority,
largest stable bounded-face-component selection, square and truthful 0/60/120
boundary-aligned arrangements, exactly two perimeter
openings, wall-complement equality, one dual spanning-tree reachability and
acyclicity, one automatic solution with no unreachable selected cells, and
solution traversal only through removed walls. Rectangular final clipping may
leave transparent fringe outside bounded maze cells because canvas bounds do not
invent closure. Coverage also includes path identity/order, fingerprints,
cancellation, allocation, and every maze resource limit.

Domain and IO coverage includes descriptors, conditional fields, typed variant
replacement, history undo/redo, `Family` invalidation, copy-on-edit,
capability projection, current v4/v2 round trips, unchanged existing
serialization, and proof that derived state and limits are absent from bytes.

Patterns coverage includes every nearest/random-eligible family, tree grid
legality, straight-grid maze legality, non-grid/raw-path rejection before
allocation, mechanism neutrality, exact consumption of evaluated family sites
and guide contributors, guard-inclusive construction, and agreement with
independently broader family envelopes. Three-guide tests prove phase-aligned
0/60/120-degree input, a degree-six interior lattice, only the three source
wall axes, triangular cells, and a cell-traversing solution.

Engine/render coverage proves broader family-envelope reuse, insufficient
envelope misses, topology changes without site movement, unchanged ordinary
guide/parametric/mark identities, canonical PNG/SVG parity, cancellation
atomicity, and stale-publication rejection.

Run focused Stage 20M and directly affected foundational tests only, then
affected-package format/check/strict Clippy, `scripts/validate_architecture.sh`,
protected-path review, immutable-asset hashes, and an authorized full
semantic-map update followed by the final read-only worktree impact review.

Generate raw artifacts under `target/validation/stage20m/` for a two-guide
recursive-backtracker wall maze, a three-guide recursive-backtracker wall maze,
a randomized Prim tree, and connected dispersion. Exercise both immutable
inputs at intrinsic size: PNG at 1024x1024 and SVG at 900x620. Retain native
PNGs, raw SVGs, and rasterized SVG inspection PNGs. Compare representative
output to the focused `maze2.svg`, `maze3.svg`, and
`poisson-disc-connected.svg` design references. Preserve native RGBA bytes and
inspect visible RGB and alpha without substituting a viewer composite for file
content.

No GTK/Wayland run is required because this stage is headless and changes no
frontend behavior.

## Allowlist and exclusions

The single implementation writer may change only the directly affected source
and focused tests under `crates/toniator-domain`, `toniator-geometry`,
`toniator-patterns`, `toniator-io`, `toniator-engine`, and `toniator-render`;
`crates/toniator-cli` only if a bounded headless artifact-generation seam is
strictly necessary; this contract, the roadmap/tracker transition, and
`.codex-work/` implementation evidence. `Cargo.toml` and `Cargo.lock` are in
scope only if the bounded implementation requires a dependency-edge change.

Do not change `toniator-app`, `Project Specification/`, `ToniatorLegacy/`,
`assets/`, unrelated fixtures, gallery/preset entries, Pattern Wizard controls,
or earlier validation directories. Explicit masks and walks, TSP, general
curved-guide arrangement faces, region realization, composites, authored or
wrap-around maze endpoints, motifs, animation, compatibility work, and Stage
20N+ are excluded. The grid-maze wall complement and its bounded straight-grid
face/dual authority are expressly included.

## Gates

Use one writer, then an independent read-only implementation reviewer. During
implementation, stop at
**Implemented awaiting review**, repair only confirmed in-scope findings, then
stop again at **Implemented awaiting review** pending parent and user review.
After explicit user acceptance, record the implementation checkpoint and keep
publication and the next stage separately gated. Do not commit, push, publish,
deploy, or begin Stage 20N without explicit authorization.
