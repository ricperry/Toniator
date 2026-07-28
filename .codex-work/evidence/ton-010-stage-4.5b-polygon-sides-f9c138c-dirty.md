# TON-010 Stage 4.5B correction — Regular Polygon side count

- Recorded: 2026-07-28
- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `f9c138c493a9d687b5300abddf14e78281f2ad63`
- Scope: bounded 4.5B correction; 4.5C, 4.5D, and Stage 5 were not started.

## Parent review

The existing `web_polygon_sides` control was already declared in the shipping
`resources/toniator-window.blp`, compiled through the GResource manifest, and
bound to the current pattern descriptor and `change_web_treatment` mutation
path. The omission was the Blueprint row container: it was not retained by
`AppUi` and relied on an indirect child `visible-notify` bridge. Selecting
Regular Polygon changed authoritative state, but the shipping row could remain
hidden.

The correction in `src/ui.rs` retains the production row widget and explicitly
synchronizes row, label, and spin-button visibility from the authoritative
polygon context. It does not add a duplicate control, change shape algorithms,
or make `RenderVariant` authoritative.

## Realized GTK coverage

`verify_realized_resource_polygon_sides_authority_workflow` exercises the
shipping `AppUi` through `gtk::Builder::from_resource`. It verifies:

- Circle initially hides the control; Regular Polygon reveals an enabled
  integer 3–6 control with default value 4 and schema label/help.
- Contradictory transient `RenderVariant::WebShapeV1` state cannot override
  authoritative Regular Polygon/6 state.
- Shared changes to 3 and 6 persist through authoritative settings and update
  the derived adapter; a Magenta-only edit persists 3 while Cyan remains 6.
- Circle, User Defined, and mixed All Inks contexts hide the sides control and
  preserve the existing mixed-mark workflow.

## Validation

Parent reran and passed:

- focused realized GTK regression;
- `cargo test --locked` — 138 library and 46 binary/UI tests;
- `cargo check --locked --all-targets`;
- `cargo clippy --locked --all-targets -- -D warnings`;
- `cargo fmt --all -- --check`;
- `blueprint-compiler lint -r syntax` for all three current Blueprints; and
- `git diff --check`.

Existing 4.5B shape-editor screenshots remain applicable; no artifacts were
added or changed. Manual GNOME/Wayland inspection is still required before
4.5B can be user-accepted. Do not begin 4.5C automatically.

