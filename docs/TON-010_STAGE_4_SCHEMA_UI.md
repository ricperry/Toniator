# TON-010 Stage 4: schema-driven pattern UI

Stage 4 is complete at the implementation and automated-validation boundary
and is paused before the newly inserted Stage 4.5 baseline gate. The
schema-driven inspector now
projects only from `Document.pattern_state`; transient Shapes/Curves
`RenderVariant` adapters are not UI authority.

## Authority contract

`PatternDocumentState` is the sole persisted pattern selector and parameter
store. The UI reads the selected identifier, metadata, and typed Shapes/Curves
settings through authority-only accessors. Selector and inspector-panel
decisions come from the selected registry entry. UI edits call
`DocumentEditor::select_pattern`, `set_shape_settings`, or
`set_curve_settings`, preserving validation, undo, autosave, preview refresh,
and current inactive-pattern state.

`RenderVariant` is not consulted by production `src/ui.rs`: the only remaining
references there are test-only fixtures that deliberately install contradictory
adapters. Rendering may still receive a derived adapter from the authoritative
state for the current legacy Shapes/Curves execution paths, but the adapter
cannot select, overwrite, serialize, or independently undo pattern state.

## Schema and UX binding

`PatternRegistry` supplies stable selector metadata and control descriptors.
Shapes and Curves controls use those descriptors for labels, help, visibility
and accessibility metadata while retaining the current mature editor layout.
The selector keeps the live GTK models and deferred synchronization behavior;
invalid transient dropdown positions do not become an all-channel fallback.
Narrow-layout, keyboard/focus, and resource-level behavior remain covered by
the existing GTK tests.

## Remaining narrowly bounded adapters

These are the remaining execution or transition projections, not persisted UI
authority:

* `Document.render`: transient derived Shapes/Curves execution adapter used by
  the current renderer and export paths. Replace its consumers as Stages 5–6
  move generators to canonical output; it remains `serde(skip)` and must not be
  read by the schema-driven UI.
* `OutputTreatmentCache.render` and the inactive RGB/CMYK treatment caches:
  transient per-output execution/cache state needed to preserve current
  CMYK/RGB transitions and preview behavior. Remove or reduce these fields
  when the canonical pattern generators and output cache replace the legacy
  branches in Stages 5–6; their embedded `pattern_state` remains authoritative.
* `saved_web_shape`, `saved_web_curve`, and their pipeline snapshots:
  in-memory Crosshatch exit and output-mode restoration snapshots. Remove them
  when canonical Crosshatch/transition handling replaces that legacy escape
  path in the later pattern stages; they are skipped from persistence and do
  not select or own pattern parameters.
* `src/preset.rs` channel extraction and renderer-facing export helpers:
  narrow projections needed by current Shapes/Curves preset application and
  channel exports. Retain until those consumers use canonical pattern output,
  then delete the projection helpers; they read authoritative state or a
  derived adapter and never write selection authority.
* Registry legacy adapter metadata and test-only adapter setters:
  retained to route current Shapes/Curves through the declared compatibility
  contract and to prove contradictory-adapter rejection. Remove after all
  current and new generators consume canonical output at the Stage 8 review.

No production UI adapter reads remain. This inventory is the removal boundary;
it does not authorize starting Stage 4.5 or the later stages.

## Validation and limitations

The current worktree passes 138 library tests and 46 binary/UI tests,
`cargo check --locked --all-targets`, strict Clippy, formatting, and diff
checks. Realized GTK tests cover authoritative selector/panel state,
contradictory Shapes and Curves adapter values, scalar and editor edits,
artboard/path/color/motif state, descriptor help, Crosshatch pipeline context,
and deferred dropdown synchronization. Existing save/reopen, preset, undo/
redo, CMYK/RGB, Shapes/Curves transition, current-schema rejection, and
preview/export tests remain green.

No manual Fedora GNOME/Wayland click-through, screenshot, or accessibility
acceptance is claimed. That remains a human validation limitation, not a
second authority model. Stage 4.5 has not started, and Stage 5 Weighted
Voronoi remains blocked until Stage 4.5D is explicitly accepted.
