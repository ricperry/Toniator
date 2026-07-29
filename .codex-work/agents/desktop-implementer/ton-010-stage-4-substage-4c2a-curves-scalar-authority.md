# TON-010 Stage 4, Substage 4C2a — Curves scalar UI authority migration

- Recorded: 2026-07-28
- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `f9c138c493a9d687b5300abddf14e78281f2ad63`
- Producing agent: desktop-implementer
- Task: migrate Curves parameter synchronization and scalar/layout/color/visibility
  callbacks to authoritative `Document.pattern_state`; stop before 4C2b direct
  curve-editor, motif-overlay, and editing-context work.

## Working-tree basis

The checkout began intentionally dirty with TON-010 stages 1–4C1, TON-013,
presets, resources, fixtures, evidence, and documentation.  It remains
preserved.  This substage modified `src/ui.rs` and added this evidence file;
there was no commit, push, reset, cleanup, persistence/schema/preset change,
or work outside the approved boundary.

## Inspected subsystems and symbols

- `src/ui.rs`: `sync_controls`, `selected_curve_inks`,
  `change_curve_treatment`, Curves scalar/layout/color/visibility callbacks,
  `sync_curves_schema_metadata`, the realized GTK selector/control test, and
  the intentionally deferred direct editor/motif/context helpers.
- `src/pattern.rs`: Curves descriptor control IDs and metadata from Stage 4A.
- 4C1 and 4B implementation evidence plus the current dirty worktree and HEAD.

## Verified findings and implementation

- `sync_controls` now obtains Curves values only from
  `Document.pattern_state.curve_settings()` when Curves is selected; it no
  longer chooses a Curves synchronization arm or reads values from
  `RenderVariant::WebCurveV1`.
- `selected_curve_inks` and Curves callbacks obtain RGB/Crosshatch semantics
  through `curve_output_flags`, backed by `ArtworkPipelineSettings`; Crosshatch
  is no longer inferred from `WebCurveSettings.value_mode` in these paths.
- `change_curve_treatment` clones authoritative `WebCurveSettings`, applies its
  existing update closure, and writes with `DocumentEditor::set_curve_settings`.
  Existing undo/coalescing, target/mixed-value behavior, autosave, rendering,
  and deferred GTK synchronization remain unchanged.
- Touched Curves controls receive descriptor-backed tooltip/accessibility help
  using `PATTERN_REGISTRY.parameter_for_control`: layout, shared curve, color
  inputs, scalar sliders, and visible-channel label.  The regular shared-curve
  label derives its base text from the descriptor while the Crosshatch-specific
  layer label remains semantic pipeline terminology.
- The realized GTK test now installs contradictory transient Curve adapter
  settings and authoritative Curve settings, asserts authoritative line weight
  and coverage in controls, changes line weight, and asserts persistence in
  `pattern_state`.  It retains selector panel, deferred synchronization, and
  live `DropDown` model-identity coverage.

## Deliberate 4C2b boundary

Remaining non-test `RenderVariant::WebCurveV1` reads are not part of this
substage: `document_artboard_size`; direct curve-editor/motif-overlay helpers
(`current_motif_arrangement`, `motif_overlay_geometry`, `sync_motif_overlay`,
`current_curve_path`, `current_curve_color`); and `update_editing_context`.
They are the explicit 4C2b migration target.  No Shapes, selector, custom
pattern, Weighted Voronoi, persistence, or preset work was changed.

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

Results: all passed.  Full coverage was 138 library tests and 46 binary/UI
tests.  The realized GTK test uses the established single-initialization test
path, retains live model identity, and covers the scalar edit/callback.  No
manual GTK/Wayland session, screenshot, export, desktop validation, or core
inspection was performed or claimed for this bounded change.

## Inferences and unresolved uncertainty

- The existing direct editor/motif/context adapter reads must be migrated in
  4C2b before claiming all runtime Curves UI reads are authority-only.
- A later authorized Fedora GNOME/Wayland manual pass should verify the
  descriptor-provided help and end-to-end Curves interaction after 4C2b;
  automated realized GTK coverage is not manual visual acceptance.
- Documentation appears milestone-level rather than necessary for this
  substage; reassess after Stage 4 acceptance.

## Invalidation conditions

Reinspect/retest if Stage 4A accessors or Curves descriptor IDs change;
`ArtworkPipelineSettings` Crosshatch semantics change; GTK resource IDs,
dropdown model synchronization, `change_curve_treatment`, or the remaining
4C2b helpers change; or the current dirty worktree is rebased/replaced.
