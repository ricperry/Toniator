# Stage 20H — Capability Projection Foundation

Status: **Complete at commit `4b1cc08819eee36c2009e2abf5543dcaefe29929`** (user accepted
2026-08-21; publication remains separate). This execution contract is subordinate to
the protected normative specification, the greenfield roadmap, and the Stage
20G effective-pattern authority contract.

## Objective

Add a domain-owned, typed, read-only projection of the active validated
recipe structure. The projection selects either the document base definition
or a channel's Stage 20G effective definition. It provides future workflows
with structural facts without creating a second evaluator or allowing preset
names, thumbnails, metadata, GTK, or CLI code to choose behavior.

## Public contract

`toniator-domain` exposes `PatternCapabilityScope::{DocumentBase,
Channel(ChannelId)}` and `Document::pattern_capabilities(scope)`. The resolver
returns `PatternCapabilityProjection { definition_id, family, outputs }`.

- Family is either `Grid(GridCapabilityProjection)` or
  `Dispersion(DispersionCapabilityProjection)`.
- A grid projection contains density/seed generator flags, guide count,
  spacing/phase/editability facts, ordered active generic guide prototype
  kinds, and its intersection or along-guide site product.
- A dispersion projection contains density/seed flags and active random
  character, density-modulation, and exclusion kinds.
- Ordered outputs currently contain only `Marks`, with active mark prototype,
  orientation, and fill-range support.
- Document-base scope uses `DocumentPatternSettings::definition_id`. Channel
  scope calls `Document::effective_channel_pattern` once, then projects its
  resolved definition. Channel scalar deltas never change capability shape.

Validation and projection share one exhaustive domain helper. Invalid or
missing authority returns the established `ValidationError`; valid but
unsupported future workflow branches are structurally absent. The projection
is ephemeral: it is not serialized, cached, commanded, invalidated, or added
to history. Property descriptors remain the authority for legal edits.

## Accepted current semantics

- Legacy straight guides expose two guides and no spacing, phase, editability,
  or generic prototypes.
- Typed straight dimensions expose their validated one-to-four count plus
  spacing and phase.
- Generic guides expose their validated one-to-four count, spacing and phase,
  stored prototype kinds, and editability only when an authored open path is
  active.
- Current site products are intersections, along-guides, and random sites.
  Current random projections reuse the active typed discriminants.
- Current mark outputs preserve definition order and expose only active circle
  or authored-shape prototypes with fixed, tangent, or normal orientation.
- No path, stroke, region, composite, fixed-triagrid, parametric, adjacency,
  offset, temporal, page, or UI capability is introduced.

`toniator-patterns::PatternPipelinePlan` remains the engine-facing plan and
the domain crate must not depend on `toniator-patterns`. Focused tests prove
that the projection agrees with accepted pattern plans without a production
adapter.

## Verification and stop gate

Focused domain tests cover base/effective scopes, inheritance/overrides,
scalar-delta invariance, errors, legacy/typed/generic guides, random variants,
marks, orientations, deterministic output, and structural omission. Patterns
tests compare current accepted plans, including document-aware generic guides.
An engine test proves querying the projection leaves cache diagnostics and
output identity unchanged, and a v4 Holiday fixture witness verifies divergent
channel definitions resolve independently.

Run focused current tests, affected-package format/check/strict Clippy,
architecture validation, `git diff --check`, protected-path and immutable
asset checks, and semantic-map's read-only worktree review. The reviewed
implementation checkpoint is `4b1cc08819eee36c2009e2abf5543dcaefe29929` and
the user accepted Stage 20H on 2026-08-21. Publication remains separate;
Stage 20I+ remains separately gated. No GTK work, schema/fixture port, or
Stage 20I+ implementation is authorized by this contract.
