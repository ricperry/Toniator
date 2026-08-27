# Stage 20S — Capability and Baseline Recipe Completion

Status: **Accepted awaiting checkpoint** (2026-08-26). This document records
the bounded Stage 20S agreement and remains subordinate to the protected
project specifications and `docs/GREENFIELD_REWRITE_PLAN.md`.

## Objective

Stage 20S completes the headless Stage 20 pattern surface. It adds a typed,
derived capability vocabulary and scope-filtered authoritative property
descriptors; ID-free parametric-curve recipes; and a curated, deterministic
16-record bundled catalog. Stage 20R remains authoritative for ordered
outputs, filters, persistence, cache identity, limits, cancellation, canonical
geometry, and rendering.

## Contracts

- `PatternCapabilityProjection` exposes canonical-order feature flags,
  active property descriptors for the requested document-base or effective
  channel scope, and `supports_all`.
- `PropertyDescriptor` remains the only source of value kinds, bounds, units,
  choices, applicability, invalidation, references, and commands. Capability
  records never duplicate those fields.
- Base scope contains base pattern controls and its active structural controls.
  Channel scope contains effective/delta channel controls, output responses,
  and the active effective definition's structural controls. It excludes
  source, paint, mapping, opacity, visibility, translation, and unrelated
  definitions.
- Projection remains derived, non-persisted, non-cached, and identity-neutral.
  Ordered output records retain painter index, effective response, authored
  filter, and compatible dependency targets.
- `PatternStructureRecipe::ParametricCurve` expresses existing schema-v5
  spiral/repetition intent and optional along-curve sites without a new family.
  Materialization allocates mechanisms and output IDs deterministically,
  validates candidate and recipe-local indices before publication, and uses the
  existing recipe-replacement/Undo semantics.
- Region outputs expose only `Scale` and `UniformOffset`, each with a normalized
  `minimum_fill..=maximum_fill` interval in `0.0..=2.0`. Fill is a linear
  geometric-radius multiplier: zero omits the region, one exactly replays its
  untreated positive boundary, and two doubles its equivalent radius. Scale
  uses that fill directly; UniformOffset derives one signed boundary
  displacement per positive untreated region to target base area times fill
  squared, retaining only positive cleaned components. No Full branch,
  authored inter-cell spacing, negative-space complement, or derived
  displacement is persisted or cached.
- Preset v3 gains that tagged recipe variant without a format-version change;
  valid existing variants stay valid and obsolete/malformed inputs stay
  rejected.
- The patterns-owned catalog entry includes a nonserialized `required_features`
  gallery field. Registry version 2 filters availability by clone-applying the
  recipe at the requested scope, normally resolving/projecting it, and omitting
  unsupported entries. Metadata never affects dispatch, coverage, identities,
  cache keys, or renderer behavior. Thumbnails remain `None`.

## Bundled catalog

The version-2 registry contains exactly these 16 lexicographically ordered
records. Purpose text is the implemented registry metadata, not an evaluator
selector.

| ID | Name / category | Implemented purpose |
| --- | --- | --- |
| `clustered-dispersion-random-links` | Clustered Connections / Connections | Clustered sites with deterministic random links. |
| `even-random-circles` | Even Dispersion Marks / Dispersion | Even sites with deterministic circle marks. |
| `grid-voronoi-scale` | Grid Voronoi / Regions | Two-guide intersections with source-driven Scale fill from 0.0 through its natural boundary at 1.0. |
| `one-guide-lines` | One Guide Lines / Guides | One straight guide dimension as structural paths. |
| `residual-sites-along-guide` | Connected and Residual Sites / Composites | Nearest links followed by residual equal-arc marks. |
| `round-spiral-line` | Round Spiral Line / Parametric | One canvas-covering clockwise round spiral path. |
| `round-spiral-marks` | Round Spiral Marks / Parametric | Canvas-covering clockwise round spiral with equal-arc circle marks. |
| `source-weighted-dispersion-voronoi` | Source-Weighted Voronoi / Regions | Luminance-weighted dispersed Voronoi regions. |
| `square-spiral-marks` | Square Spiral Marks / Parametric | Equal-arc circle marks on a square spiral. |
| `straight-grid-circles` | Straight Grid Circles / Marks | Two-guide intersection circle marks with guide-tangent orientation. |
| `three-guide-cells-scale` | Three-Guide Cells / Regions | Phase-aligned equilateral Guide Faces with positive-region Scale resizing. |
| `three-guide-maze` | Three-Guide Maze / Connections | Triangular recursive-backtracker maze. |
| `triagrid-custom-shape-marks` | Triagrid Diamond Marks / Marks | Three-guide intersection diamond marks. |
| `triagrid-spanning-tree` | Triagrid Spanning Tree / Connections | Three-guide randomized-Prim spanning tree. |
| `two-guide-cells-uniform-offset` | Two-Guide Cells / Regions | Rectangular Guide Faces with positive-region uniform offset resizing. |
| `two-guide-maze` | Two-Guide Maze / Connections | Rectangular recursive-backtracker maze. |

`regions-plus-marks` is not a catalog record: the user retired that temporary
maze-debug recipe without a replacement.

All ordinary catalog and evidence documents expose visible R/G/B channels; each
channel samples its corresponding source component/intensity, and every
seed-bearing recipe uses deterministic pairwise-distinct typed seeds across
R/G/B.

## Curved-guide validation fixtures

Validation-only engine fixtures exercise current typed authored curves without
adding catalog cards: `curved-one-stack-paths`, `curved-one-stack-sites`,
`curved-one-normal-offset-paths`, `curved-one-normal-offset-sites`,
`curved-two-stack-paths`, and `curved-two-stack-intersections`. They retain
visible RGB channels, authored cubic authority, and normal engine/PNG/SVG
boundaries. NormalOffset fixtures use constant positive centerline spacing;
they do not calculate negative space.

The stable IDs are lexicographically ordered: `clustered-dispersion-random-links`,
`even-random-circles`, `grid-voronoi-scale`, `one-guide-lines`,
`residual-sites-along-guide`,
`round-spiral-line`, `round-spiral-marks`,
`source-weighted-dispersion-voronoi`, `square-spiral-marks`,
`straight-grid-circles`, `three-guide-cells-scale`, `three-guide-maze`,
`triagrid-custom-shape-marks`, `triagrid-spanning-tree`,
`two-guide-cells-uniform-offset`, and `two-guide-maze`.

The temporary `regions-plus-marks` maze-debug recipe is retired by explicit
user direction and is not replaced; generic `SitesUsedBy` capability and
filter coverage remains part of the public mechanism contract.

All use guard steps 2 and additional margin 0. The three bundled spiral cards
use recipe-local `CoverCanvas` intent: materialization derives fixed clockwise
turns from the receiving canvas corner radius and radial spacing 16, while
manually authored finite spirals retain explicit fixed turns. The
`round-spiral-marks` card uses one source
curve, equal-arc sites at interval 16 and phase zero, and the standard circle
mark response range 0.25 through 0.85. `round-spiral-line` remains a single
raw structural path with path response 0.15 through 0.65. Straight-guide phases are zero
and spacing multipliers one; two-guide angles are 0/90 and three-guide angles
are 0/60/120. Path recipes use round joins/caps. Random recipes retain the
existing 16,000,000 attempt and neighbor-check bounds. The detailed purpose,
seeds, response ranges, region treatment/sampling, spiral, maze, connection,
and composite choices are the approved Stage 20S task contract.

## Boundaries and acceptance

No GTK work, schema migration, preset version bump, new geometry algorithm,
renderer dispatch by metadata, compatibility adapter, or Stage 21 work is
included. Existing typed schema-v5 authored documents continue to project.
Focused domain/pattern/IO/engine/render tests cover capabilities, controls,
filters/cycles, parametric serialization/materialization/history, all catalog
records, renderer parity, cache/replay, cancellation/stale publication, and
metadata identity neutrality. The validation directory holds serialized
presets, manifest evidence, and native PNG/raw SVG/SVG-raster artifacts for
the named eight representatives on immutable raster and SVG inputs. SVG raster
review records its live-text/font caveat. Independent review, its nested
preset-v3 unknown-field correction and re-review, and parent visual inspection
are complete. `semantic-map check` is unavailable and inapplicable because
Toniator has no semantic-map architecture schema; project documentation is the
architecture authority, while `scripts/validate_architecture.sh` is the
mechanical validation check. The user accepted Stage 20S on 2026-08-26 after
independent review and re-review, focused verification, parent intrinsic
RGB/alpha visual inspection, and durable-document reconciliation. The
implementation checkpoint is pending, and this acceptance authorizes its local
creation; push, publication, and later-stage work remain unauthorized and
separately gated.
