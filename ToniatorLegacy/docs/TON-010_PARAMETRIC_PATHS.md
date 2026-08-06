# TON-010 Parametric Paths contract

Recorded 2026-08-02. Gate 3A established the native typed-generator
foundation; Gate 3B adds its first immutable bundled recipe and a generic
typed-centerline-to-canonical-path execution seam. Gate 3C makes that same
immutable definition selectable by its stable `PatternId` through the strict
Document v10 `bundled_definition_instances` authority and generic
document/runtime seam. Gate 3 is parent accepted after correcting the preset
index regression caused by removing the named Spiral entry. The correction
uses one typed 13-entry preset inventory. It adds neither a selector UI nor
GTK controls.

## Quadratic-radial Spiral v1

The registered native operation is `parametric-paths.quadratic-radial-spiral`
v1. It
has no runtime inputs and returns the native-only `ParametricPath` port. A
later bounded recipe stage may consume that typed centerline; it is not
canonical document output and cannot itself be serialized by a recipe.

For revolutions `u`, the generalized quadratic-radial model is:

```text
r(u)     = r0 + s*u + 0.5*g*u^2
theta(u) = theta0 + d*2*pi*u
```

`r0` is Starting Radius, `s` is Radial Growth per Revolution, and `g` is
Spacing Growth per Revolution. `d` is +1 for clockwise and -1 for
counterclockwise in Toniator’s canonical artboard coordinates (positive Y
points down). Center X/Y are offsets from the execution-context artboard
center, in canonical document units. The generator rejects a model whose
radial derivative `s + g*u` would become negative across the declared turns,
so a turn cannot fold back over an earlier radius.

When `g = 0`, this is the Archimedean specialization. A nonzero `g` changes
radial spacing quadratically, so the stable operation/type contract does not
mislabel every supported path as strictly Archimedean. Gate 3A exposes this
one generalized family only; it does not add a family selector.

The default is 20 turns, zero starting radius, 20 document units of radial
growth per revolution, zero spacing growth, a zero-degree clockwise start, and
a centered origin. On the preservation checkpoint's common 640 by 480
artboard, that approximates the prior Shapes Spiral's 20-unit pitch and
400-unit corner-radius construction. It deliberately does not retune other
patterns or claim parity for other artboard sizes.

## Creator contract and sampling

`quadratic_radial_spiral_parameter_definitions()` supplies strict Gate 2 creator
metadata for the future recipe: all values are Pattern Definition-owned,
geometry-invalidating structural values. The contract exposes turns, starting
radius, radial growth, spacing growth, starting angle, direction, center X/Y,
maximum sample distance, edge extension, and edge overscan. Center X/Y are a
declared two-dimensional relation; edge overscan is applicable only when edge
extension is true.

Maximum Sample Distance is an explicit document-unit quality control. The
generator derives a conservative speed bound from the largest radius and
radial derivative, then samples sufficiently densely that every generated
centerline chord is no longer than that distance. Work is bounded at one
million samples and cooperatively cancellable.

When enabled, Edge Extension computes the farthest distance from the offset
center to the four execution-artboard corners, then appends just enough
additional revolution extent to reach at least the larger of the base radius
and that corner radius plus Edge Overscan. It therefore remains truthful for
wide, tall, and off-center artboards. The generator rejects an extension that
would carry a negative radial derivative. This preserves centerline coverage
outside the artboard for a future clipped consumer; this generator performs no
clipping itself.

## Bundled recipe and canonical emission

`assets/patterns/quadratic-radial-spiral.v1.tnpattern` is immutable bundled
definition `parametric-paths.quadratic-radial-spiral.v1`. Its two-node graph
uses the typed generator followed by generic
`parametric-paths.emit-paths` v1. The emitter consumes only its
`ParametricPath` input plus already-declared channel-instance `enabled`,
`color`, and `opacity` values; it does not create a second channel authority
or mutate those values. It uses the established canonical `CurveGeometry` /
`CurveInkLayer` representation and a one-document-unit centerline width.

`execute_parametric_paths_definition_cancellable` is graph/registry-driven:
it accepts a definition, value-only instance, and execution context without
matching a bundle name, display label, selector index, or family branch. Gate
3C's `execute_resolved_definition_cancellable` is the generic resolved-definition
seam used by the accepted Gate 3C route: it
resolves the selected definition from `Document.pattern_state`, runs its
registered native operations for each declared semantic channel, and combines
the resulting canonical paths. It does not inspect a parametric-family or
Spiral ID. `RenderVariant` stays a derived compatibility boundary, not the
selection authority. The bundle has strict parse/schema validation, stable
identity, deterministic canonical output, cooperative cancellation, and
bounded sampling coverage. This is intentionally a Paths-only/provider-limited
proof; other canonical output kinds require a later bounded output-algebra
stage. The generic serializer preserves valid graph layout records it does not
assign current editor meaning to.

The current document v10 format has the required
`bundled_definition_instances` map beside compatibility instances and embedded
definitions. It stores strict `PatternInstanceParameters` by stable ID; a
missing field, unknown definition, or conflicting duplicate authority is
rejected rather than migrated or defaulted. Current `.tntr` v6 fixture
definitions explicitly carry the required empty map when they do not select a
generic bundled definition.

## Gate 3 acceptance and boundary

The selected immutable Spiral is now the strict Document v10 bundled-instance
authority. The resolved-definition executor runs the same canonical Paths for
preview, PNG, and editable SVG, with no bundle-label, selector-index, family,
or display-name branch. The old named Spiral preset mutation/default injection
is removed. The Gate 3C deterministic PNG, SVG, metrics, and GTK launch
screenshot are implementation evidence only, and no human creative acceptance
is claimed.

Gate 3C proves that a stable selected Spiral definition produces the same
canonical result for preview, PNG, and editable SVG. It also removes the old
named `Spiral` preset mutation/default injection. Gate 4 is specifically the
schema-generated Spiral Guided editor: metadata-driven controls, help, units,
validation, focus/accessibility, conditional enablement, local preview, and
removal of the superseded Shapes-compatible Spiral authoring path. Gate 5 is
separate: it binds Guided and the actual Graph view to one non-lossy draft and
proves round trips, unfamiliar valid-content preservation, Cancel, Apply,
undo/redo, and invalid-draft rejection. User-library resolution, arbitrary
canonical output algebra, and compatibility-adapter removal remain later
parent-approved work.

## Gate 4 implementation — schema-generated local Guided draft

Gate 4 is parent accepted. It adds `GuidedDefinitionCatalog` and
`GuidedDefinitionDraft` as a
definition/instance-local editing boundary. The catalog enumerates the current
bundled registry by stable `PatternId` and follows generic default policy: the
current editable stable document selection, otherwise the first editable
registry entry; only the artifact fixture explicitly requests Spiral. It never
selects a definition by its display name, a numeric position, or a family
match. A registry entry whose strict creator metadata is not yet renderable
remains discoverable with its exact unavailable reason rather than receiving
guessed controls.

The Guided draft consumes only declared Pattern Definition-owned Creator
parameters present in the definition layout. It derives section order, labels,
help, units, numeric bounds/steps/precision, choice values, declared
two-dimensional relationships, and applicability from the strict metadata.
Channel-instance `enabled`, `color`, and `opacity` are deliberately excluded:
they remain Channel Settings authority. Unsupported metadata categories return
a clear error; no control is inferred from a parameter name.

The GTK dialog is an immutable bundled-definition inspector with one local
draft. Every widget validates a proposed typed value through the current
definition, refreshes its declared conditional availability, and invokes the
same generic resolved-definition executor and canonical renderer used by the
runtime route. Every control has an accessible label and description, metadata
ordering provides keyboard focus order, and the dialog states that a duplicate
to a document-local copy is required before Apply. There is intentionally no
Apply, Cancel lifecycle, undo, or Graph affordance in Gate 4.

The former Shapes-compatible Math Function `Spiral` choice, its fixed
definition choice value, and its preview branch are removed. The remaining
Math Function choices (`Sine`, `Square Wave`, and `Sawtooth Wave`) remain the
current Shapes-compatible generator surface. The removed choice is superseded
by the immutable Parametric Paths definition; it must not be restored as a
second authoring route. Gate 5 may retain the other Math Function generator
only if its current strict definition remains supported; any later removal must
reject obsolete recipe values rather than map or default them.

Focused coverage proves that all eleven exposed Spiral structural parameters
change the local canonical preview, including explicit edge-extension coverage
behavior. The corrected non-interactive artifact path exits generically through
application quit even when a modal remains open. The settled artifact is
`test-artifacts/ton-010-gate-4/guided-spiral-editor-settled.png`; the locked
suite passed 285 library and 60 binary/UI tests. These are implementation
evidence only, not human GNOME/Wayland, screen-reader, creative, or external-
application acceptance.

## Gate 5 acceptance — shared non-lossy Guided/Graph draft

Gate 5 is parent accepted. Guided and the actual bounded Graph view share one
complete `SharedRecipeEditorDraft`; Guided binds creator parameters by stable
metadata IDs, while Graph lists stored nodes/edges and edits layout X/Y only.
The draft preserves valid current-schema content not exposed by the bounded UI,
including SVG assets, Quick Controls, layout anchors, and operation arguments,
through Guided edits, Graph layout edits, duplication, and JSON round trips.

Apply validates and installs the complete document-local definition/instance as
one atomic document edit. Cancel is non-mutating; rejected Apply preserves the
exact draft for correction and corrected-ID retry; undo/redo restore the
complete relationship. Draft and final model boundaries reject collisions with
built-in or immutable bundled IDs. Guided and node-qualified Graph controls
have accessible labels/relations, and the local ID/provisional inherited name
remain visible. Graph is not a topology, argument, asset, or schema editor;
unknown operation IDs/fields remain rejected by the strict current schema.

The locked Gate 5 suite passed 292 library and 61 binary/UI tests. Inspected
Wayland artifacts are
`test-artifacts/ton-010-gate-5/shared-recipe-editor-guided.png` and
`test-artifacts/ton-010-gate-5/shared-recipe-editor-graph.png`; they are
implementation evidence only. Gate 6A's definition-lifecycle resolver/model
foundation is parent accepted. Gate 6B — Definition Lifecycle UI binding is
next: user-library discovery and resolution, Save As/reload/reopen, project
embedding and recovery, missing-definition diagnostics, and
duplicate/conflicting-ID handling remain open. Human acceptance remains
outstanding.

Gate 6B1 is parent accepted: the generic
native Pattern Library exposes stable-ID provenance, malformed-file diagnostics,
and explicit reload; a user-library Spiral can be embedded and selected with
validated active-channel defaults as one undoable portable project edit. Shared
local drafts atomically save their complete definition with current structural
values promoted to `.tnpattern` defaults, while per-channel values stay project-local, and
verify discovery without selection mutation. The locked Gate 6B1 suite passed
305 library and 61 binary/UI tests; the isolated-XDG library capture is
implementation evidence only. Gate 6B2A is parent accepted: strict external
import is parse-before-write with stable-ID classifications for ready,
identical, user conflict, immutable bundled conflict, and project-protected
conflict. Identical content is no-write dedupe; destructive replacement is
restricted to one matching user file, while duplicate files allow only
Cancel/new-ID. New-ID payloads preserve graph, schema, layout, assets, metadata,
and definition defaults. Project copies reopen through the shared Guided/Graph
draft with one-edit Apply and exact undo/redo/save/reopen history. Channel
values remain project-instance data. The locked Gate 6B2A suite passed 309
library and 61 binary/UI tests. Four isolated-XDG artifacts are implementation
evidence only. Gate 6B2B implementation now awaits parent review: a strictly
decoded document with only a missing selected stable ID becomes a typed recovery
candidate, never a live editor. Exact matching recovery validates preserved
values and embeds portable authority; explicit replacement uses sorted resolver
IDs and fresh defaults without mapping old structural values. The locked matrix
passed 312 library and 61 binary/UI tests.
