# TON-010 Stage 5 architecture map

Current corrective closeout snapshot: 2026-08-02. Gates 1–5, Gate 6A, Gate
6B1, Gate 6B2A, and Gate 6B2B are parent accepted; this is not a TON-010
closeout record. Gate 7 cross-family proof is next and TON-010 remains Open.

Gate 1 corrective ownership is recorded in
`docs/TON-010_PATTERN_CONTROL_OWNERSHIP.md`. It freezes the current UI/control
boundary without changing canonical output: definitions own reusable
construction/topology/randomness policy/structural orientation; channel
instances own enabled/color/opacity/sampling/coverage-density response/source
weighting/seed/mark primitive/mark scale/mark or treatment rotation/selection
visibility. Corrective Gate 2 is complete in
`docs/TON-010_PATTERN_PARAMETER_SCHEMA.md`: it makes authoring metadata a
strict typed contract, rejects recipe v1 without migration/defaulting, and
does not add a GTK consumer. Gates 3B/3C provide one immutable Parametric
Paths Spiral recipe, generic stable-ID document selection, and canonical
preview/export execution. Gates 4 and 5 are parent accepted: Gate 4 provides
the registry-backed schema-generated Guided draft/catalog, and Gate 5 provides
the shared non-lossy Guided/Graph draft. Gate 6A's definition lifecycle
resolver/model foundation, Gate 6B1 native library/Save As binding, Gate 6B2A
import/project-copy editing, and Gate 6B2B missing-definition recovery are
parent accepted. Gate 7 is the cross-family proof boundary.

| Consumer or boundary | Current path | Current status | Closeout boundary |
| --- | --- | --- | --- |
| Weighted Voronoi | `pattern_state` -> bundled `.tnpattern` -> registered Weighted operations -> positive canonical inset regions | Live recipe route; neutral site/Voronoi algorithms preserved | Complete manual reference and SVG-editability acceptance |
| Shapes | `pattern_state` -> bundled or embedded Shapes definition -> registered Shapes operations -> canonical marks/network | Live recipe route, but editor and adapter are Shapes-specific | Replace preset-specific authoring and remove duplicate compatibility dispatch after parity tests |
| Curves | `pattern_state` -> bundled Curves definition -> registered Curves operations -> canonical paths | Live bundled recipe route with typed compatibility adapter | Expose through the common editor/runtime contract and remove duplicate authority after parity tests |
| Parametric Paths Spiral | `pattern_state.bundled_definition_instances` -> immutable bundled definition -> generic resolved-definition executor -> canonical paths | Gates 4–5, Gate 6A, Gate 6B1, and Gate 6B2A parent accepted; lifecycle/import/project-copy paths preserve bundle/document authority | Gate 6B2B adds missing-definition recovery |
| Preview / PNG / SVG | canonical output -> shared semantic-channel consumers | Shared live route; model-aware RGB/CMYK composition and editable SVG are automated-tested | Finish human GNOME/Krita/Inkscape acceptance and arbitrary-recipe parity |
| Declarative recipes | strict `.tnpattern` v1 parser -> typed DAG -> bounded registered native operations | Bundled Shapes/Curves/Weighted and embedded custom Shapes definitions execute live | Decompose authoring into useful composable operations; no scripts/plugins/native extensions |
| Persistence | `Document.pattern_state`, stable bundled-definition instances, embedded custom definition/assets, typed instance values | Strict document v10 / preset v6; obsolete or incomplete definitions rejected | Complete missing/conflicting definition diagnostics and portable recovery |
| Presets | `.tntr` complete/treatment/channel sections | Strict preset v6; bundled fixtures updated | Verify bundled references and custom definition/asset embedding end to end |
| User library | XDG user pattern directory -> `UserPatternLibrarySnapshot` -> `PatternDefinitionLifecycleResolver` -> `PatternDefinitionRegistry` -> native Pattern Library | Gate 6 complete: strict discovery/reload, parse-before-write import, duplicate-path-safe replacement/new-ID, bundled/project protection, project-copy edit/history/reopen, exact missing-definition recovery, and channel values excluded from `.tnpattern` | Gate 7 proves the same contracts for Structured Fields and Stochastic Distributions |
| Pattern Editor | local immutable Guided draft -> shared non-lossy Guided/Graph draft -> embedded custom definition | Gates 4–5 parent accepted: Spiral metadata Guided controls, bounded layout-only Graph, atomic Apply/Cancel/undo/redo, and current-schema content preservation | Gate 6B adds definition lifecycle and user-library/recovery behavior |
| Channel Settings | selected ink -> channel treatment/distribution values -> `pattern_state` and embedded instance projection | Main-window controls are the intended per-channel authority | Freeze the ownership table and remove legacy naming/synchronization leaks |

## Authority and remaining seams

`Document.pattern_state` is the persisted pattern-selection authority. Pattern
definitions own reusable construction/topology/randomness policy/structural
orientation: placement, structural spacing, curve/math family, dispersion and
jitter algorithms, and connection/output structure. Channel instances own
ink-specific treatment: enabled state, colour, opacity, sampling and
coverage-density response, source weighting/influence, seed, selected mark
primitive, mark scale, mark or treatment rotation, and selection visibility.
Definitions declare supported emitted geometry but do not own the selected
per-ink mark primitive or scale.

Gate 3C removed the named Spiral preset mutation/default route and replaced the
split label/action lists with one typed 13-entry preset inventory. The accepted
Gate 4 Guided editor removes the Shapes-compatible Math Function `Spiral`
choice and its preview/recipe branch; the immutable Parametric Paths definition
is its sole current authoring route. Other Shapes-compatible Math Function
choices remain current separate definitions until a later scoped removal. Gate 1 renamed the proven Shape Size
Response ownership leak to `channel_random_size_response` and removed its
inert `PatternEditorDraft` copy. The remaining compatibility storage is
documented in the ownership table and is not a second state authority.

`RenderVariant`, NativeBasic, Crosshatch handling, and typed Shapes/Curves
adapters remain compatibility execution seams. Gate 3C's resolved bundled
definition executor is a generic canonical-path seam; its current registered
output algebra intentionally covers canonical paths only. The selected
immutable Spiral is therefore a Paths-only proof, not a complete output-kind
provider. `PatternDefinitionRegistry` now has a Gate 6A presentation-neutral
lifecycle consumer: `PatternDefinitionLifecycleResolver` loads direct
`.tnpattern` files from the XDG user directory in stable path order, retains
malformed-file diagnostics, and builds the bundled/user/project registry
without changing render authority. Project content remains authoritative over
differing user content with the registry diagnostic; immutable bundled content
remains a hard conflict. Missing stable IDs yield candidate recovery inputs
rather than automatic fallback. Save As shares this XDG directory policy but
now has a parent-accepted Gate 6B1 native consumer: Pattern Library shows provenance and
malformed-file diagnostics, reloads explicitly, and embeds a user definition
with validated defaults for active output channels as one undoable project
edit. Shared drafts can atomically save their complete definition with current
definition-owned structural values promoted to defaults; per-channel values
remain project-local. Discovery verification does not change selection. Gate
 Gate 6B2A is parent accepted for strict external import/open, explicit
duplicate/conflict choices, and generic project-definition reopen/edit. Gate
6B2B is parent accepted for missing-definition recovery; no automatic fallback
is introduced. Gate 7 must prove the same public contracts for one Structured
Fields and one Stochastic Distributions proof, sequentially, without
family-specific GTK branches.

The Gate 2 schema is intentionally an authority/serialization boundary rather
than a widget system. `IntegerValue` with unit `None` identifies non-count
integers such as seeds. Percentages are stored as normalized fractions and
displayed as percent values with a validated displayed increment/precision.
Genuine normalized influence remains `[0, 1]`; response exponents are unitless
and may use a wider declared domain. Document-relative distance always means
canonical document/artboard units, never device pixels. See the schema contract
for the complete category/unit table and current bundled examples.

## Stable Stage 5 geometry and composition boundaries

`src/site_distribution.rs` remains authoritative for bounded deterministic
site generation, source weighting, arrangement policy, semantic identity,
fingerprints, and cancellation. `src/voronoi_geometry.rs` remains authoritative
for pure clipped-cell, shared-boundary, artboard, and response-inset geometry.
Do not alter placement or tessellation without a demonstrated focused failure.

Weighted Voronoi canonical output contains final boundary-derived inset
polygons as positive regions. Raw cells and cell-sizing rings do not survive
the producer boundary. Preview and PNG render isolated semantic-channel
coverage before RGB additive or CMYK multiplicative composition. SVG keeps
named channel layers and compound positive paths; genuine subtractive regions
remain channel-local.

## Acceptance state

Gate 3 is parent accepted after correcting the preset-index regression exposed
by removing the named Spiral entry. The typed preset inventory now keeps labels,
actions, selected-state lookup, and bounds checks aligned. The current-format
Document v10 `bundled_definition_instances` map is the strict stable-ID
authority for the immutable Spiral; the generic resolved-definition executor
feeds canonical Paths shared by preview, PNG, and editable SVG. The 160 by 120
PNG, editable SVG, and GTK launch screenshot in the Gate 3C evidence are
implementation artifacts, not human creative acceptance. The executor is
intentionally Paths-only at this gate and there is no selector UI.

Gate 4 is parent accepted for the registry-backed generic catalog/default
policy, metadata-generated definition-owned controls/help/units/order,
accessibility/applicability, local canonical preview, channel exclusion,
immutable-bundle messaging with explicit document-local duplication, and old
visible Shapes Spiral removal with obsolete-value rejection. The corrected
non-interactive artifact capture exits generically through application quit even
when a modal remains open. The full locked suite passed 285 library and 60
binary/UI tests; the settled GTK screenshot
`test-artifacts/ton-010-gate-4/guided-spiral-editor-settled.png` is
implementation evidence only.
Gate 4 does not mutate the document, provide Apply/Cancel, undo/redo, or a
Graph editor. Gate 5 is now parent accepted for one shared non-lossy current-
schema draft: Guided stable-ID bindings and a bounded Graph that edits stored
layout coordinates only; atomic Apply, non-mutating Cancel, undo/redo,
invalid-Apply preservation and corrected-ID retry; draft/model bundled-ID
collision rejection; and accessible Guided/node-qualified Graph controls.
The full locked suite passed 292 library and 61 binary/UI tests. Inspected
Wayland artifacts are
`test-artifacts/ton-010-gate-5/shared-recipe-editor-guided.png` and
`test-artifacts/ton-010-gate-5/shared-recipe-editor-graph.png`; these are
implementation evidence only.

Gate 6A is parent accepted for the presentation-neutral
`PatternDefinitionLifecycleResolver`: stable XDG discovery, malformed-file
diagnostics, bundled/user/project precedence, immutable bundled-ID conflicts,
typed missing-ID recovery candidates, and atomic reload. A failed strict
conflict reload preserves the prior snapshot, selection, resolved
definition/fingerprint/source, and diagnostics. Save As shared the XDG policy
and was write-only at the Gate 6A boundary; Gate 6B1 now provides the native
consumer. No UI artifact applies because Gate 6A adds no UI.
The locked Gate 6A suite passed 301 library and 61 binary/UI tests.

Gate 6B1 is parent accepted. The native Pattern Library resolves bundled,
user-library, and project-embedded definitions by stable ID with provenance,
malformed diagnostics, explicit reload, and one undoable user-definition
embedding that preserves save/redo/reopen. Shared Guided/Graph Save Draft writes
the complete definition after promoting definition-owned structural values to
defaults; channel/per-ink values remain excluded and project-local. The locked
Gate 6B1 suite passed 305 library and 61 binary/UI tests. The isolated-XDG
1280x820 artifact hash is
`74f31c07df6e80dced0b2780bea3b161b5c8238ac5a614b20241cff8f8a34dc9`; it is
implementation evidence only.

Gate 6B2A is parent accepted. External `.tnpattern` imports strictly parse
current-schema content before mutation. Stable-ID planning distinguishes ready,
identical, user-library conflict, immutable bundled conflict, and
project-protected conflict without label/index/family inference. Identical
content is no-write dedupe; destructive replacement is atomic and restricted
to exactly one matching user file; duplicate user files permit only Cancel or
new custom ID. New-ID imports preserve graph, schema, layout, assets, metadata,
and definition defaults exactly. Generic project-copy editing reopens the
complete embedded definition and current instance in the shared Guided/Graph
draft; Apply, undo/redo, save, and reopen preserve structural and channel state.
The locked Gate 6B2A suite passed 309 library and 61 binary/UI tests. Four
isolated-XDG Wayland captures are implementation evidence only: import-library
`e40b502cd0190a24aa6bbd2c1deb405b191c3077c7ad468bd50189596b365b49`,
project-copy-library `4ff46d97848fafd3d80ee5c9d343df53b924c234052c9175dbbc08583cf79d26`,
user-replace-conflict `489e07755f7ee69be29a1cfc12318c68897a6fab40fd6b31c86a2ffb751396c3`,
and project-protected-conflict
`8203611b69646ab7da7c88311497add15e2c098a130e09a5d41a2a5b1e1a6bea`.

Human Stage 5 acceptance remains pending for GNOME/Wayland interaction,
Krita-reference RGB/CMYK inspection, and Inkscape Break Apart. Stage 6's
Triangular Dot Grid, Wave Line Field, and Evenly Spaced Pointillism must be
delivered as genuine bundled proof recipes through the same public authoring
surface. Gate 5 parent-acceptance evidence covers one non-lossy draft, strict
current-schema unexposed-content preservation, Cancel, Apply as one edit,
undo/redo, invalid rejection/retry, and explicit document-local duplication.
Its bounded Graph edits authoring layout positions only, not topology,
arguments, assets, or schema. Gate 6B2B is parent accepted: current-schema
document open retains a typed candidate only when the selected stable ID is the
sole missing authority after strict decode and all unrelated validation holds.
The invalid value cannot enter the editor. Native recovery is explicit: Cancel;
strict exact-ID `.tnpattern` lookup that validates retained values and embeds
portable authority; or a sorted stable-ID replacement with fresh defaults and
no structural-value mapping. Replacement is disabled when resolver conflicts
make it unsafe. The final locked matrix passed 314 library and 63 binary/UI
tests. A transient parent D-Bus `NoReply` was followed by the exact isolated
test and later full passes without a code workaround. The final isolated-XDG
Wayland recovery artifact hash is
`5ce73287a14dd726ae33d255c1db4321f7dad177911a05fb29acea432f87def4`; it is
implementation evidence only. Gate 7 is the exact next gate: sequentially
prove one Structured Fields and one Stochastic Distributions pattern through
the same public schema, shared draft, Guided editor, lifecycle, and canonical
runtime. Do not add family-specific GTK branches or alter protected Weighted
Voronoi placement/tessellation. Compatibility-adapter removal and human
acceptance remain open.
