# TON-010 Stage 5 framework restart

Recorded 2026-07-29 on `TON-010-Stage5-Framework-Restart`.

## Why Stage 5 was restarted

The previous `TON-010-Stage5-Voronoi` tip remains preserved as
`archive/TON-010-Stage5-Voronoi-pre-framework` and tag
`TON-010-stage5-voronoi-pre-framework` at `e37eeb2`. Its useful clipped-cell
and boundary mathematics is a reference, but its generation layer placed site
distribution, source response, Voronoi construction, canonical output, and
pattern settings in one Weighted Voronoi module. That ownership would make
Shapes, Curves, Pointillism, maze, spiral, grid, stacked-curve, and
traced-curve additions repeat the same services.

The restart is based directly on the accepted Stage 4 checkpoint `87b4ce3`.
Stage 4 authority, strict validation, cancellation, canonical output, and
shared preview/PNG/SVG routes remain in force.

## Generation pipeline and ownership

```text
Prepared artwork / resolved channel field
    -> site_distribution (neutral guide/site basis)
    -> voronoi_geometry (clipped cells and boundaries)
    -> weighted_voronoi (source response and canonical regions)
    -> CanonicalPatternOutput
    -> preview / PNG / SVG
```

`src/site_distribution.rs` owns bounded deterministic candidate generation,
uniform and source-weighted selection, polarity, strength, arrangement policy,
semantic identity input, stable ordering, fingerprints, and cancellation.
Uniform placement uses jittered stratified candidates; weighted placement uses
finite exponential-priority selection without rejection-loop fallback.

`src/voronoi_geometry.rs` owns pure half-plane clipping, bounded cell
construction, explicit artboard versus shared interior boundaries, and
boundary-derived insets. It does not know about channels, artwork, pattern
settings, UI, or rendering.

`src/weighted_voronoi.rs` is an adapter. It validates persisted settings,
requests each enabled channel's resolved field, maps settings to neutral
requests, applies response insets, and emits each final boundary-derived inset
polygon as one positive canonical region. Raw clipped cells and raw-to-inset
boundary rings are construction data only; they are not final Weighted
Voronoi artwork. `WeightedVoronoiCellRegion` preserves semantic channel/site
identity without claiming a cell-sizing subtraction. General canonical
subtraction remains available for genuine holes, knockouts, and other
semantics that cannot be represented as final positive geometry.

`Document.pattern_state` remains the only persisted pattern authority.
`RenderVariant::WeightedVoronoiCanonicalV1` is a derived dispatch marker. The
registry supplies stable identity, metadata, specialized control descriptors,
schema version 3, and generator version 2. The document and preset formats
were not bumped because their persisted envelopes are unchanged; generator
version 1 is rejected explicitly.

## Declarative recipe contract boundary — 2026-08-01

The accepted recipe-contract milestone adds strict `.tnpattern` v1 data types
and validation in `src/pattern_definition.rs`, deterministic layered
resolution and provenance diagnostics in `src/pattern_definition_registry.rs`,
and a bounded cancellation-aware native-operation executor. It reuses
`SiteDistribution`, `DistributionField`, `VoronoiDiagram`, and
`CanonicalPatternOutput`; recipe data cannot load scripts, plugins, native
libraries, or arbitrary code.

The original contract-only status is now superseded by the 2026-08-02
preservation checkpoint. Bundled Shapes, Curves, and Weighted Voronoi resources
have production native operation bodies and execute through the same strict
loader and bounded DAG runtime. Embedded custom Shapes definitions are
persisted and dispatched through the canonical preview/PNG/SVG route. Current
documents are v10 and `.tntr` presets are v6; obsolete or incomplete
definitions are rejected without migration or defaulting.

The integration remains incomplete. The user Pattern Editor constructs a
Shapes-compatible definition by mutating fixed graph nodes and parameters;
the remaining authoring choices are still hard-coded and the native placement
operation retains its legacy branch. Gate 3C removed the named Spiral
preset/default mutation and corrected the remaining preset actions to one typed
13-entry inventory after a preset-index regression was found. Gate 4 is now
parent accepted: a registry-backed stable-ID Guided catalog creates an
immutable definition-local metadata draft, renders its canonical local preview,
and excludes channel-instance fields. It removes the legacy Shapes Math
Function Spiral choice/branch. Gate 5 now adds the shared non-lossy Guided/
Graph draft and document-local duplicate/Apply/Cancel/undo/redo lifecycle.
Gate 6A adds a presentation-neutral XDG user-library snapshot and layered
bundled/user/project resolver, including malformed-file diagnostics, explicit
reload inputs, missing-ID recovery candidates, and immutable-bundle conflict
enforcement; it is parent accepted. Save As now shares that XDG directory
policy, but no UI library/import/open or selection flow can select a saved file
again. The full user-facing portability/recovery workflow and
compatibility-dispatch removal remain TON-010 work. See `ISSUES.md` and
`.codex-work/evidence/ton-010-gate-3c-generic-selected-spiral-parity-2026-08-02.md`.

### Corrective Gate 1 — control ownership, 2026-08-02

The current controls are frozen in
`docs/TON-010_PATTERN_CONTROL_OWNERSHIP.md`. `Document.pattern_state` remains
the semantic authority. Definitions own reusable construction, topology,
randomness policy, and structural orientation; channel instances own
enabled/color/opacity/sampling/coverage-density response/source weighting/seed/
mark primitive/mark scale/mark or treatment rotation/selection visibility. Gate
1 removed the only proven draft leak: Shape Size Response is now named
`channel_random_size_response` in
Channel Settings and is no longer captured by `PatternEditorDraft`. A focused
recipe test verifies structural-draft changes cannot replace channel treatment
values and preserves grid rotation as a retained structural compatibility
projection. No geometry, canonical output, or parameter-schema work is part of
this gate.

### Corrective Gate 2 — parameter-schema contract, 2026-08-02

`docs/TON-010_PATTERN_PARAMETER_SCHEMA.md` records the completed typed
parameter-authoring contract. Every parameter has explicit creator/internal
authoring metadata, layout placement, ownership, applicability, serialization,
and invalidation semantics. Numeric category/unit pairs now distinguish integer
counts from unitless integer values such as seeds, normalized stored percentages
from their percent display precision, genuine `[0, 1]` influence from unbounded
unitless response exponents, and canonical document/artboard distance from
pixels. Current `.tnpattern` format v1 and recipe v2 remain strict; recipe v1
is rejected and no migration/defaulting path exists.

This gate adds neither GTK controls nor recipe operations or output semantics.
Schema-generated Spiral Guided controls remain Gate 4; the shared Guided/Graph
draft and actual Graph view are separate Gate 5 work.
Gate 3B added the first immutable Parametric Paths Spiral definition and a
generic typed-centerline-to-canonical-path operation through the strict
declarative/native boundary. Gate 3C resolves its stable `PatternId` from the
required strict Document v10 `bundled_definition_instances` map and runs a
generic resolved-definition executor before compatibility `RenderVariant`
dispatch. Its value-only channel inputs preserve existing channel-instance
authority. Preview, PNG, and editable SVG consume the same canonical Paths;
the old named Spiral preset mutation/default injection is gone. Removing that
entry exposed a stale numeric action mapping, so the correction replaced the
split lists with one typed 13-entry preset inventory and added deterministic
alignment coverage. The executor is intentionally Paths-only/provider-limited
at this gate, does not add a selector UI, and has no family/display/index
branch. Gate 3 is parent accepted; these artifacts remain implementation
evidence rather than human visual acceptance.

### Corrective Gate 4 — schema-generated Guided Spiral editor, 2026-08-02

Gate 4 is parent accepted. The registry-backed `GuidedDefinitionCatalog` uses
generic default policy: the current editable stable document selection,
otherwise the first editable registry entry; only the artifact fixture
explicitly requests Spiral. `GuidedDefinitionDraft` consumes strict
definition-owned metadata for controls, help, units, order, accessibility,
and applicability, validates typed edits, and refreshes a local canonical
preview through the generic executor/renderer. Channel-instance values remain
outside the Guided draft. Bundled definitions are immutable and the dialog
requires explicit duplication to a document-local copy before Apply.

The visible Shapes-compatible Math Function `Spiral` choice, obsolete value,
and preview/recipe branch are removed; obsolete values are rejected rather
than remapped or defaulted. The non-interactive artifact path now quits the
application generically after capture, including when a modal remains open.
The locked Gate 4 suite passed 285 library and 60 binary/UI tests. The settled
GTK artifact is implementation evidence only; human GNOME/Wayland,
screen-reader, and creative acceptance remain open. Gate 4 does not add
document lifecycle, Apply/Cancel, undo/redo, or a Graph view. Gate 5 is now
parent accepted with one local non-lossy Guided/Graph draft, explicit
document-local duplication, atomic Apply/undo/redo, non-mutating Cancel,
rejected-Apply preservation and corrected-ID retry, accessibility relations,
and bounded authoring-layout Graph positions. Graph is not a topology,
argument, asset, or schema editor; losslessness covers valid current-schema
content not exposed by the bounded UI. The full Gate 5 matrix passed 292
library and 61 binary/UI tests; Gate 6A, Gate 6B1, Gate 6B2A, and Gate 6B2B
are now parent accepted. Gate 7 cross-family proof is next.

### Corrective Gate 5 — shared non-lossy Guided/Graph draft, 2026-08-02

Gate 5 is parent accepted. `SharedRecipeEditorDraft` is one complete
strict-schema draft shared by Guided stable-ID bindings and the actual bounded
Graph view. Graph lists stored nodes/edges and edits layout X/Y only. Apply is
one atomic document edit, Cancel is non-mutating, rejected Apply preserves the
exact draft for corrected-ID retry, and undo/redo restore the complete
relationship. Draft and model boundaries reject built-in and immutable bundled
ID collisions. Accessible Guided/node-qualified Graph controls, visible local
identity, strict current-schema unexposed-content preservation, and refreshed
Wayland artifacts passed review. At the Gate 5 boundary, Gate 6 lifecycle
work was still open; subsequent Gate 6A/6B1/6B2A/6B2B substages are now parent
accepted. Human GNOME/Wayland, keyboard, screen-reader, save/reload, and
creative acceptance remain open.

### Corrective Gate 6A — definition-lifecycle resolver/model foundation, 2026-08-02

Gate 6A is parent accepted. `PatternDefinitionLifecycleResolver` provides a
presentation-neutral bundled/user/project resolution surface with stable XDG
direct-file discovery, malformed-file diagnostics, immutable bundled-ID
conflicts, project-over-user diagnostics, typed missing-ID recovery candidates,
and atomic reload. A failed strict-conflict reload preserves the prior
snapshot, selected identity, resolved definition/fingerprint/source, and
diagnostics. Save As shares the XDG policy; at the Gate 6A boundary it was
write-only, and Gate 6B1 now provides the accepted native consumer. No UI
artifact applies because this substage adds no UI; the locked suite passed 301
library and 61 binary/UI tests. Gate 6B1, Gate 6B2A, and Gate 6B2B are now
accepted native lifecycle consumers; Gate 7 cross-family proof is next.

### Corrective Gate 6B1 — native library, embedding, and shared-draft Save As, 2026-08-02

Gate 6B1 is parent accepted. The native Pattern
Library lists bundled, user-library, and project definitions by stable ID with
provenance, malformed-file diagnostics, and explicit Reload. Only an
authoritative user-library entry can be used in a project; it becomes a complete
embedded definition plus validated defaults for the active output channels in
one undoable edit, so reopen is portable. The shared Guided/Graph draft now
atomically saves its complete definition with current definition-owned
structural values promoted to `.tnpattern` defaults; per-channel instance values remain
project-local. It verifies discovery without applying the draft or changing
selection. Bundled definitions remain duplicate-before-edit. The realized
1280x820 Wayland library capture and locked 305 library / 61 binary-UI matrix
are implementation evidence. Gate 6B2A is parent accepted for external
import/open, explicit duplicate/conflict choices, and project-definition
reopen/edit. Gate 6B2A and Gate 6B2B are both now parent accepted; Gate 7 is
next, and no automatic fallback was added. The isolated-XDG
1280x820 artifact SHA-256 is
`74f31c07df6e80dced0b2780bea3b161b5c8238ac5a614b20241cff8f8a34dc9`; it is
implementation evidence only. The locked Gate 6B1 suite passed 305 library and
61 binary/UI tests. Human GNOME/Wayland, keyboard, screen-reader, save/reload,
and creative acceptance remain open.

### Corrective Gate 6B2A — strict external import and project-copy editing, 2026-08-02

Gate 6B2A is parent accepted. External
`.tnpattern` input is parsed and validated before a user-library mutation.
Identical content is explicitly deduplicated; a differing user-library stable
ID presents destructive replacement only when exactly one valid matching file
exists, otherwise duplicate paths force Cancel/new-ID. Bundled content is
immutable; project authority cannot be replaced by import and directs creators
to Edit Project Copy or new-ID. A
successful write alone triggers resolver refresh; a failed refresh preserves
the previous resolver surface. Project-embedded rows expose generic Edit
Project Copy, reopening their complete stored definition and current instance
in the shared Guided/Graph draft; Apply is one undoable edit, Cancel is inert,
and undo/redo/save/reopen preserve structural values, graph layout, and channel
values. New-ID imports change only the stable ID and preserve graph, schema,
layout, assets, metadata, and definition defaults. The locked Gate 6B2A suite
passed 309 library and 61 binary/UI tests. Four 1280x820 isolated-XDG Wayland
captures are implementation evidence only: `import-library.png`
(`e40b502cd0190a24aa6bbd2c1deb405b191c3077c7ad468bd50189596b365b49`),
`project-copy-library.png`
(`4ff46d97848fafd3d80ee5c9d343df53b924c234052c9175dbbc08583cf79d26`),
`user-replace-conflict.png`
(`489e07755f7ee69be29a1cfc12318c68897a6fab40fd6b31c86a2ffb751396c3`), and
`project-protected-conflict.png`
(`8203611b69646ab7da7c88311497add15e2c098a130e09a5d41a2a5b1e1a6bea`).
Gate 6B2B missing-definition recovery is parent accepted.
Strict current-schema decode retains only the selected-ID-missing candidate;
all unrelated validation failures remain hard and the invalid value never
reaches an editor. Native recovery is Cancel-default, strict exact-ID lookup
with preserved-instance validation, or explicit sorted-ID replacement with
fresh defaults rather than structural mapping. Recovered custom authority is
embedded and opens dirty for explicit save. The final locked matrix passed 314
library and 63 binary/UI tests. A transient parent D-Bus `NoReply` was followed
by the exact isolated test and two later full passes without a code workaround.
The final isolated-XDG artifact hash is
`5ce73287a14dd726ae33d255c1db4321f7dad177911a05fb29acea432f87def4`.
Human chooser, keyboard, screen-reader, import/edit, save/reopen, and creative
acceptance remain open.

## Cache boundaries

Resolved channel fields use the existing bounded request-local cache keyed by
source generation, field bounds, pipeline identity, output model, assignment,
active channel, and enabled semantic channels. Weighted output metadata keeps
source generation, resolved-field generation, distribution fingerprint,
geometry fingerprint, channel identity, and view key separate. Distribution
settings (seed, count, arrangement, mode, polarity, strength) are therefore
distinct from geometry settings (boundary gap and response/inset controls),
and view-only preview presentation remains downstream of canonical output.
There is no process-global or unbounded pattern cache.

## Future consumers and intentional deferrals

The neutral services are deliberately small. Weighted Voronoi retains
`site_distribution.rs` and `voronoi_geometry.rs` as its algorithm authorities.
Shapes, Curves, and Weighted Voronoi now have bundled recipe adapters and
registered native operations, while typed compatibility adapters and
`RenderVariant` branches remain as production seams. The custom editor exposes
grid, triangular, curve, math, and random variants through a monolithic
Shapes-specific placement operation. Its former Math Function `Spiral` choice
and branch are removed: the immutable Parametric Paths definition is the sole
current Spiral authoring route. The remaining Math Function choices are kept
only while their strict current definition remains supported; no obsolete
choice may be defaulted or remapped. These variants are not substitutes for a general composable
operation/editor surface. The accepted user-library resolver now leaves the
selector UI and cross-family proof catalog open.

Pointillism shared/independent arrangements, recipe library/import/export,
layered resolution, and project embedding/recovery remain open. Gate 6B —
Definition Lifecycle UI binding is next; no global cache is introduced.

## Validation evidence

Focused distribution tests cover deterministic ordering, seed changes,
source-independent uniform placement, spatial spread, weighted polarity,
exact counts, distinct sites, shared/independent arrangements, cancellation,
and centralized limits. Geometry tests cover clipped bounds, shared interior
boundaries, artboard exclusion, insets, degenerate input, cancellation, and
clustered input. Weighted tests cover semantic fields, uniform independence,
arrangement policy, strict generator rejection, persistence, undo/redo,
preset behavior, canonical preview/PNG/SVG parity, and perimeter omission.
The realized GTK selector/control regression is also covered; human GNOME/
Wayland pointer and screen-reader acceptance remains unclaimed.
The remaining Stage 5 manual gate also includes Krita-reference CMYK/RGB
inspection and Inkscape **Break Apart** inspection of editable SVG output.
Preserved reference images are evidence inputs only, not human acceptance.

## Correction pass

The 2026-08-01 correctness pass preserves the framework and changes only its
canonical consumers: semantic region rasterization now renders isolated
per-channel coverage before deterministic RGB additive or CMYK multiplicative
composition, so genuine subtraction cannot erase sibling channels. Direct
Weighted Voronoi inset regions therefore have no cell-sizing subtraction path,
and semantic SVG exports use one editable compound positive path per channel.
The artboard clip remains a page/domain constraint because canonical geometry
may be out of bounds; genuine subtraction masks remain only where genuine
subtractive regions exist. Preview Surface remains preview-only and
Export Background is applied at the export presentation stage.
