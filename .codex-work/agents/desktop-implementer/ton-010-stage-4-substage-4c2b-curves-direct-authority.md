# TON-010 Stage 4, Substage 4C2b — Remaining Curves UI authority migration

- Recorded: 2026-07-28
- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `f9c138c493a9d687b5300abddf14e78281f2ad63`
- Producing agent: desktop-implementer
- Scope: finish the remaining runtime Curves UI adapter reads after accepted
  4C2a; stop before 4D.

## Working-tree basis

The worktree was intentionally dirty at handoff with completed TON-010 stages
1–4C2a, TON-013 work, presets, fixtures, resources, evidence, and
documentation.  It was preserved.  This substage changed `src/ui.rs` and added
this evidence entry only; no commit, push, reset, cleanup, schema, persistence,
preset, Shapes, custom-pattern, or Weighted Voronoi work occurred.

## Inspected symbols and verified implementation

- `document_artboard_size` now selects dimensions from authoritative Shapes or
  Curves pattern state and no longer reads an execution adapter.
- Direct Curves UI readers `current_motif_arrangement`,
  `motif_overlay_geometry`, `sync_motif_overlay`, `current_curve_path`, and
  `current_curve_color` read typed copies from
  `Document.pattern_state.curve_settings()`.  The copies are taken inside
  bounded state borrows before target-selection helpers run, preserving GTK/
  `RefCell` safety.
- Direct curve-editor and motif writes already flow through
  `change_curve_treatment`; its 4C2a authority implementation writes with
  `DocumentEditor::set_curve_settings`, retaining undo, autosave, preview, and
  coalescing behavior.
- `update_editing_context` uses selected pattern state plus
  `ArtworkPipelineSettings` for Curves/Crosshatch context.  It no longer
  inspects `RenderVariant` or `value_mode`.
- Curves schema binding now also covers the curve editor and motif-controls
  container via `PATTERN_REGISTRY.parameter_for_control`, applying descriptor
  help as tooltip/accessibility description without adding controls.
- Runtime `src/ui.rs` has no remaining `RenderVariant::WebCurveV1` references;
  its test-only import remains solely for intentional contradictory-adapter
  tests and legacy-projection assertions.

## Contradictory-adapter coverage

The established realized GTK single-initialization regression now installs
authoritative Curves values for artboard dimensions, line path, channel color,
motif layout/arrangement, and scalar settings while deliberately replacing the
transient Curve adapter with conflicting defaults.  It verifies:

- authoritative artboard dimensions;
- path and color helper values;
- motif controls, overlay visibility, and overlay geometry;
- profile edit persistence through authoritative settings;
- descriptor-backed curve editor help;
- Curves/Crosshatch editing context and target mapping derived from the
  authoritative artwork pipeline despite a contradictory non-Crosshatch
  adapter; and
- retained selector/deferred-sync/live-dropdown-model behavior.

## Verification

Passed:

```text
cargo test --locked --bin toniator ui::tests::realized_numeric_controls_leave_continuous_scroll_to_parent -- --exact
cargo test --locked --lib model::tests::authority_read_accessors_ignore_a_contradictory_transient_adapter -- --exact
cargo test --locked --bin toniator ui::tests::editor_controls_resource_exposes_static_editor_structure_without_display -- --exact
cargo test --locked
cargo check --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --all -- --check
git diff --check
```

Results: all passed.  The full suite had 138 library and 46 binary/UI tests.
No manual GTK/Wayland interaction, screenshot, export artifact, desktop
validation, or core inspection was performed or claimed; realized GTK/resource
coverage is automated evidence only.

## Unresolved uncertainty / follow-up

- This completes the current runtime Curves adapter-read migration in
  `src/ui.rs`; Stage 4 parent review must decide any 4D scope.  No 4D work was
  begun.
- A later authorized Fedora GNOME/Wayland manual interaction pass remains the
  appropriate visual/accessibility confirmation for the completed Stage 4 UI
  migration.
- Durable documentation remains a milestone-review decision, not a change in
  this bounded substage.

## Invalidation conditions

Reinspect/retest if pattern-state accessors, Curves descriptor IDs, artwork
pipeline Crosshatch semantics, GTK resource/model synchronization,
`change_curve_treatment`, or any Curves direct editor/motif/context helper
changes; or if the dirty worktree/HEAD is replaced.
