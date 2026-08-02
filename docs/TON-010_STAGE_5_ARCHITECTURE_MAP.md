# TON-010 Stage 5 architecture map

Current preservation-checkpoint snapshot: 2026-08-02. This is not a TON-010
acceptance record.

| Consumer or boundary | Current path | Current status | Closeout boundary |
| --- | --- | --- | --- |
| Weighted Voronoi | `pattern_state` -> bundled `.tnpattern` -> registered Weighted operations -> positive canonical inset regions | Live recipe route; neutral site/Voronoi algorithms preserved | Complete manual reference and SVG-editability acceptance |
| Shapes | `pattern_state` -> bundled or embedded Shapes definition -> registered Shapes operations -> canonical marks/network | Live recipe route, but editor and adapter are Shapes-specific | Replace preset-specific authoring and remove duplicate compatibility dispatch after parity tests |
| Curves | `pattern_state` -> bundled Curves definition -> registered Curves operations -> canonical paths | Live bundled recipe route with typed compatibility adapter | Expose through the common editor/runtime contract and remove duplicate authority after parity tests |
| Preview / PNG / SVG | canonical output -> shared semantic-channel consumers | Shared live route; model-aware RGB/CMYK composition and editable SVG are automated-tested | Finish human GNOME/Krita/Inkscape acceptance and arbitrary-recipe parity |
| Declarative recipes | strict `.tnpattern` v1 parser -> typed DAG -> bounded registered native operations | Bundled Shapes/Curves/Weighted and embedded custom Shapes definitions execute live | Decompose authoring into useful composable operations; no scripts/plugins/native extensions |
| Persistence | `Document.pattern_state`, embedded custom definition/assets, typed instance values | Strict document v9; obsolete definitions rejected | Complete missing/conflicting definition diagnostics and portable recovery |
| Presets | `.tntr` complete/treatment/channel sections | Strict preset v6; bundled fixtures updated | Verify bundled references and custom definition/asset embedding end to end |
| User library | XDG user pattern directory | Save As writes `.tnpattern`; no UI reload/import/library resolution | Add browse/import, duplicate/conflict behavior, stable selection, and layered bundled/user/project resolution |
| Pattern Editor | local draft -> Shapes-compatible graph mutation -> embedded custom definition | Structural dialog exists, but choices/defaults/node IDs are hard-coded and there is no graph editor | Generate controls from recipe/schema metadata and edit the same authoritative graph in Guided and Graph views |
| Channel Settings | selected ink -> channel treatment/distribution values -> `pattern_state` and embedded instance projection | Main-window controls are the intended per-channel authority | Freeze the ownership table and remove legacy naming/synchronization leaks |

## Authority and remaining seams

`Document.pattern_state` is the persisted pattern-selection authority. Pattern
definitions own reusable construction: placement/topology, structural spacing,
curve/math family, dispersion and jitter algorithms, connection/output
structure. Channel instances own ink-specific treatment: enabled state, colour,
opacity, coverage, sampling detail/density response, source weighting and
influence, seed, rotations, mark shape, and scale.

The visible UI partly implements this split. It is not yet structurally
enforced: `PATTERN_PRESET_LABELS`, `apply_named_pattern_preset`, and
`pattern_editor_recipe` in `src/ui.rs` encode named variants and special
defaults, while `shapes.lattice-placement-editor` contains a large native
choice branch. Widget and draft identifiers also retain obsolete
`pattern_editor_*` names for some channel-owned values. Those names must be
reconciled carefully against behavior and tests, not treated as a second state
authority.

`RenderVariant`, NativeBasic, Crosshatch handling, and typed Shapes/Curves
adapters remain compatibility execution seams. `PatternDefinitionRegistry`
validates and resolves definitions in isolation, but it is not yet the
application-wide bundled/user/project resolver. Save As is therefore write-only
from the UI even though embedded definitions reopen with their document.

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

The latest recorded complete automated suite is 261 library tests and 56
binary/UI tests, with formatting, strict Clippy, locked release, doc tests, and
a realized GTK regression previously passing on the dirty checkout. Revalidate
the exact checkpoint before publication.

Human Stage 5 acceptance remains pending for GNOME/Wayland interaction,
Krita-reference RGB/CMYK inspection, and Inkscape Break Apart. Stage 6's
Triangular Dot Grid, Wave Line Field, and Evenly Spaced Pointillism must be
delivered as genuine bundled proof recipes through the same public authoring
surface. The guided/graph editor, library/import/recovery workflow, and
compatibility-adapter removal remain TON-010 blockers.
