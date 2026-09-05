Update Toniator's normative specification and active Stage 21B documentation to establish a strict product-level distinction between PATTERNS and PRESETS before any Gate 21B-4 implementation begins.

This is a specification/terminology correction only. Do not begin Gate 21B-4 feature implementation, change runtime behavior, migrate persistence formats, or perform broad code renames unless a documentation test or validation mechanism strictly requires a corresponding update.

First inspect the current protected Project Specification, the active Stage 21B plan, ProgressTracker.md, and relevant existing terminology in the repository. Reconcile this change against the actual current architecture rather than mechanically replacing the word "preset."

Establish these normative meanings:

PATTERN

- A Pattern is a reusable structural recipe for generating pattern geometry.
- It describes pattern construction/realization concepts such as family, site strategy, guides, connections, regions, marks, modulation, and the parameters belonging to that recipe.
- A Pattern is not inherently document-wide.
- A Pattern may be applied to the current `All` edit target or to one named channel.
- Different channels in the same document may therefore use different Patterns.
- The gallery/wizard currently built in Stages 19/21B is a Pattern Gallery / Pattern Library.
- Existing built-in resources such as Even Random Circles, Straight Grid Circles, Curve Motif, Voronoi variants, mazes, spirals, etc. are Patterns, not Presets.
- User-created resources of the same semantic kind are personal Patterns.

PRESET

- A Preset is a reusable document-level configuration.
- A Preset may capture document settings and per-channel settings.
- Critically, a Preset may contain heterogeneous Pattern assignments across channels; for example C may use one Pattern while M, Y, and K use different Patterns.
- Applying a Preset is therefore fundamentally different from applying a Pattern.
- Presets and the Pattern Library are separate concepts and must not share user-facing terminology.
- Do not invent a detailed new Preset persistence schema or UI workflow beyond what is required to establish this distinction unless the existing specification already determines those details.

DOCUMENT

- A `.toniator` document/project remains the actual authored project and its source/state.
- A Preset is reusable configuration, not a document and not a structural Pattern recipe.

Required specification corrections:

1. `Project Specification/PatternSchema.md`
   
   - Replace normative language that describes named structural recipes as "presets."
   
   - Named recipes such as rectangular dots, random stippling, maze, curved lines, Voronoi cells, etc. are Patterns constructed from the generic schema.
   
   - Preserve the invariant that names/IDs never select special renderer or evaluator behavior.

2. `Project Specification/ChannelSchema.md`
   
   - Make the Pattern application semantics explicit:
     - document/base Pattern when editing `All`;
     - optional per-channel Pattern replacement for a named channel;
     - heterogeneous channel Patterns are valid.
   
   - Ensure this terminology remains consistent with the existing base-definition/channel-override architecture.

3. `Project Specification/Addendum.md`
   
   - Correct uses of "pattern preset", "general preset", or generic "preset" where the intended object is actually a structural Pattern.
   
   - Update the sections governing bundled resources, reconstruction from exposed controls, gallery/library semantics, ALL/named-channel application, and CLI terminology as necessary.
   
   - The rule formerly expressed as "every preset is reproducible using exposed editor controls" should refer to every bundled Pattern where that is the actual intent.
   
   - Preserve the prohibition on name-specific evaluator/renderer branches.

4. Review `ArchitectureSchema.md` and `ModuleStructure.md`.
   
   - Update only terminology or architectural descriptions that are actually affected.
   
   - Do not make unrelated architecture changes.

5. Update `docs/STAGE_21B_PATTERN_WIZARD_AND_PERSONAL_LIBRARY_PLAN.md`.
   
   - The current Stage 21B gallery/catalog/library resources are Patterns.
   
   - Gate 21B-2 is the Pattern Gallery / wizard shell.
   
   - Gate 21B-3 edits Patterns.
   
   - Gate 21B-4 personal management work must manage personal Patterns where the existing plan currently means structural recipe resources.
   
   - "Saving/applying a Pattern" and "saving/applying a Preset" must be explicitly separate concepts.
   
   - Do not let Gate 21B-4 accidentally implement document Presets merely because the old plan used the word "preset."
   
   - Preserve all already accepted implementation facts and checkpoints; this is a terminology/specification correction, not a rewrite of historical behavior.

6. Update `ProgressTracker.md` only where needed so the durable description of Stages 19/21B uses the corrected terminology without falsifying historical checkpoints.

Repository-wide terminology audit:

- Search for user-facing/specification occurrences of:
  - preset
  - pattern preset
  - personal preset
  - preset library
  - preset gallery
  - general preset
- Classify each occurrence by semantics before editing it.
- If it means a reusable structural recipe, call it Pattern.
- If it genuinely means or should mean a reusable document-level/per-channel configuration, call it Preset.
- Historical/internal implementation identifiers may require special handling; do not blindly rename them.

IMPORTANT COMPATIBILITY BOUNDARY:
Existing implementation and persistence currently contain names such as `preset_format_version`, preset-v4, `presets/`, registry types, IDs, CLI commands, and related Rust symbols. Do NOT mass-rename or migrate these in this task.

Where an existing serialized/internal identifier currently says "preset" but semantically stores what is now called a Pattern:

- preserve the existing identifier/format unless changing it is required for correctness;
- document it as a legacy/internal naming artifact where useful;
- leave any code/persistence migration for a separately planned and authorized change.

Do not bump document schema, preset/resource format versions, or container versions solely for this terminology correction.

The resulting normative product rule should be unambiguous:

"Pattern selection operates on the current Edit channel scope. A Pattern is a reusable structural recipe and may be applied to All or to one named channel. A Preset is a reusable document-level configuration and may contain different Patterns and settings for different channels. Pattern resources and Presets are distinct concepts."

Also ensure the UI vocabulary implied by the specification is consistent with the accepted mockups:

- Pattern Recipe
- Pattern Gallery
- Current Pattern
- Change...
- Edit channel
- All / model-specific named channels
- no "Preset Gallery" terminology for the Pattern Wizard

Before finishing:

- inspect the diff for accidental semantic changes;
- verify that accepted Stage 19/21B behavior and checkpoints were not rewritten;
- verify there is no remaining normative use of "pattern preset" where "Pattern" is intended;
- verify that genuine future document-level Presets remain clearly distinct;
- run applicable documentation/architecture validation;
- report exactly which files changed and any legacy implementation names intentionally left unchanged.

Stop after the specification/documentation correction and verification. Do not begin Gate 21B-4 implementation.
