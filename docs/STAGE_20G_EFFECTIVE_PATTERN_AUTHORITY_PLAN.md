# Stage 20G — Effective Pattern Authority

Status: **Implemented awaiting review** (implementation verified, 2026-08-21)

Stage 20G moves the shared recipe, density, layout rotation, shape rotation and
mark response into `DocumentPatternSettings`. Each ordered channel retains its
appearance, source mapping and absolute translation together with an optional
recipe replacement and typed additive deltas. `Document::effective_channel_pattern`
is the only resolver used by domain consumers, engine, CLI and GTK.

## Public authority

The current domain model is exactly `DocumentPatternSettings { definition_id,
density, pattern_rotation_degrees, shape_rotation_degrees, geometry_response }`,
`ChannelPatternInstance { definition_override, layout_delta,
shape_rotation_delta_degrees, geometry_response_delta }`,
`ChannelPatternLayoutDelta { density: Option<DensityMetricDelta2D>,
rotation_degrees: Option<f64>, translation_x, translation_y }`, and
`DensityMetricDelta2D { across_x_delta, across_y_delta }`.
`PatternGeometryResponse::Marks(MarkGeometryResponse { minimum_fill,
maximum_fill })` and `ChannelGeometryResponseDelta::Marks(
MarkGeometryResponseDelta { minimum_fill_delta: Option<f64>,
maximum_fill_delta: Option<f64> })` are the only response variants. No dormant
path or region response type is introduced. The derived projection is
`EffectiveChannelPatternInstance { definition_id, density,
pattern_rotation_degrees, translation_x, translation_y,
shape_rotation_degrees, geometry_response }`; it is never persisted.

Definition resolution is `definition_override.unwrap_or(base.definition_id)`.
All continuous composition is finite addition; authored rotations are neither
clamped nor normalized. The document owns `aspect_locked`; a channel cannot
override it. Domain builders accept desired effective density/rotation/response
values and derive/store typed deltas, including both density axes when locked.

## Commands, transaction rules, and descriptors

Every Stage 20G command carries the expected `DocumentPatternSettings` base:
`SetDocumentPatternSettings`, `ReplaceDocumentPatternDefinitionRecipe`,
`SetChannelPatternDefinitionOverride`, `ResetChannelPatternDefinitionOverride`,
`ReplaceChannelPatternDefinitionOverrideRecipe`, `SetChannelDensityDelta`,
`ResetChannelDensityDelta`, `SetChannelPatternRotationDelta`,
`ResetChannelPatternRotationDelta`, `SetChannelShapeRotationDelta`,
`ResetChannelShapeRotationDelta`, `SetChannelGeometryResponseDelta`, and
`ResetChannelGeometryResponseDelta`. Channel set commands store concrete typed
deltas; reset is the only operation that removes an option. Commands reject a
stale base, bad finite/range state, missing definition, incompatible response,
nonpositive effective density, invalid fill bounds, or a base edit that would
invalidate another channel. Validation, history, revisions, undo/redo, stale
rejection, and private-draft squash are failure-atomic.

Reset deletes stored intent; it never copies an effective value. Recipe reset
retains valid remaining deltas and otherwise fails atomically. Only identical
authoritative before/after state is a no-op: an explicit zero delta or override
that currently resolves equally remains meaningful intent. `CommandResult`
has `Option<InvalidationLevel>` so authority-only changes may report `None`.
Affected channels compare before/after effective values in document order:
definition, density, pattern rotation, translation are Family; shape rotation
and mark response are Realization; mapping and presentation retain their
existing levels. Descriptors expose document-base and channel-delta scope,
effective current values, inherited/optional state, reset capability,
applicability, and their existing invalidation metadata.

## Consumers and file boundary

Engine resolves an effective instance before capability checks and cache lookup,
then consumes only effective definition/layout/shape rotation/response. Cache
witnesses cover document-wide versus selected-channel family work, realization
changes, reset reuse, cancellation, stale publication, and both retained
channel configurations. Selected presets materialize fresh override definitions;
document recipe materialization installs a fresh base definition. Preset format
remains v2 and its bytes/reconstruction stay unchanged.

CLI and GTK make mechanical effective-value display/construction adaptations
only. They do not calculate inheritance, add inheritance UI, reorganize the
inspector, expose capability projection, or add a Pattern Wizard.

Container version remains 1. Document schema is v4 only: schemas 1–3 are
rejected, not migrated. v4 serializes exactly one tagged mark-only document
response and per-channel optional replacement/deltas plus absolute translation,
mapping and appearance. It never serializes effective state. The three current
fixtures are ported once using their first ordered channel as base and exact
later overrides/deltas, retaining current rendering; documented fixture hashes
are updated without changing the immutable PNG/SVG source bytes.

## Verification and gate

Focused domain coverage includes inheritance/replacement/addition/aspect lock,
reset then base edits, incompatible deltas/stale bases, authority-only changes,
ordered affected channels, optional invalidation, history/undo/redo/no-op and
Stage 20F draft squash. Engine coverage includes effective-only consumption,
family/realization cache witnesses, reset, cancellation/stale publication and
both configurations. IO/preset coverage includes deterministic v4 round-trip,
omitted derived state, preserved deltas, v1–v3 rejection and preset-v2 bytes.
Both immutable sources are exercised at intrinsic dimensions through ordinary
v4 evaluation/render under `target/validation/stage-20g/`, preserving native
RGBA and exercising the live-text SVG source under its documented font caveat.
Existing GTK controls receive private Wayland/AT-SPI,
focus, preview, screenshot and log evidence; Sway/wlroots evidence is not human
GNOME/Mutter acceptance. Required final checks are focused tests, affected
format/check/strict Clippy, architecture validation, diff/protected-path and
asset hashes, and read-only semantic-map worktree diff/impact/check.
These checks passed for the implementation; the verified evidence is recorded
in `.codex-work/evidence/stage-20g-implemented-awaiting-review.md`.

Non-goals: capability projection, inheritance UI, Pattern Wizard, paths,
regions, new response branches, compatibility adapters, changes to protected
specifications, and Stage 20H+ work. The stage stops at **Implemented awaiting
review**; review, acceptance, checkpointing and publication are separate gates.
