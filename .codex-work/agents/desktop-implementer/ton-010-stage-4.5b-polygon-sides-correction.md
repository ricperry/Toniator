# TON-010 Stage 4.5B correction — Regular Polygon sides control

- Recorded: 2026-07-28
- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `f9c138c493a9d687b5300abddf14e78281f2ad63`
- Producing agent: desktop-implementer
- Scope: bounded Stage 4.5B correction only; no 4.5C, 4.5D, Stage 5, presets,
  fixtures, persistence/schema, or Weighted Voronoi work.

## Working-tree assumptions

The checkout was intentionally dirty before this correction with completed
TON-010 Stages 1–4/4.5B, TON-013 resource work, presets, fixtures, evidence,
and docs. Relevant current files were inspected rather than treating prior
cache as authoritative. Existing changes were preserved; no reset, clean,
commit, push, or deletion occurred.

## Subsystems and files inspected

- `resources/toniator-window.blp`: existing `web_polygon_sides_row`, label,
  help host, and numeric `SpinButton` default of 4.
- `build.rs` and `resources/toniator.gresource.xml`: Blueprint compilation and
  generated `toniator-window.ui` registration path.
- `src/ui.rs`: `build_editor_view`, `connect_actions`, `sync_controls`, and
  `change_web_treatment` callbacks.
- `src/pattern.rs`: Shapes `web_polygon_sides` descriptor (`PolygonMark`).
- `src/model.rs`: default/validation range and authoritative settings fields.

## Verified cause and correction

The control, descriptor, authority mutation callback, and GResource object were
already present. The actual omission was contextual realization: the Blueprint
row container was not retained by `AppUi` and depended only on a child
`visible-notify` callback. After choosing Regular Polygon, authoritative state
changed but the row remained hidden.

`src/ui.rs` now retains `web_polygon_sides_row` through `EditorWidgets` and
`AppUi`, and `sync_controls` explicitly sets the row, spin button, and label
visibility from the existing authoritative polygon-context calculation. The
indirect `visible-notify` bridge was removed. No duplicate control or shape
algorithm/editor-state rewrite was made.

Writes remain in the existing `change_web_treatment` →
`DocumentEditor::set_shape_settings` path, preserving history, autosave, render
refresh, shared/per-target behavior, and adapter projection. No production UI
read uses `RenderVariant` for this control.

## Added realized regression coverage

`verify_realized_resource_polygon_sides_authority_workflow`, called from the
existing realized GTK test, uses a real `AppUi` built through the shipping
Blueprint/GResource resource. It verifies:

- Circle starts with hidden sides control and default value 4.
- Regular Polygon reveals an enabled integer 3–6 spin control and its schema
  label/help.
- Contradictory transient `RenderVariant::WebShapeV1` data cannot override
  `pattern_state` Regular Polygon/6 UI state.
- Shared edits persist 3 then 6 authoritatively and update the derived adapter.
- Circle and User Defined hide the sides control.
- Unshared Magenta-only edit persists 3 while Cyan remains 6; All Inks mixed
  geometry hides the sides control and exposes the existing mixed apply path.

## Verification

Passed commands:

- `cargo test --locked realized_numeric_controls_leave_continuous_scroll_to_parent -- --nocapture`
- `blueprint-compiler lint -r syntax resources/toniator-window.blp resources/toniator-channel-controls.blp resources/toniator-aggregate-channel-controls.blp`
- `cargo test --locked` — 138 library and 46 binary tests passed.
- `cargo check --locked --all-targets`
- `cargo clippy --locked --all-targets -- -D warnings`
- `cargo fmt --all -- --check`
- `git diff --check`

## Files changed by this correction

- `src/ui.rs`
- `.codex-work/agents/desktop-implementer/ton-010-stage-4.5b-polygon-sides-correction.md`

No Blueprint, GResource manifest/build script, descriptor, schema, preset,
fixture, or artifact files changed. Existing Stage 4.5B screenshots were not
updated because the controlled visible behavior is covered by the realized GTK
test and their prior capture purpose was the User Defined editor workflow.

## Remaining uncertainty and invalidation

No human GNOME/Wayland manual acceptance was performed. Stage 4.5D should still
manually verify physical keyboard/pointer operation and responsive inspector
layout. Invalidate this entry on changes to `src/ui.rs`, the Blueprint/GResource
build path, Shapes schema/model authority APIs, GTK/Adwaita version, Git HEAD,
or relevant dirty working-tree state.
