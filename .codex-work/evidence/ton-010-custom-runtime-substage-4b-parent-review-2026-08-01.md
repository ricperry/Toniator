# TON-010 custom runtime — Substage 4B parent review

Date: 2026-08-01
Repository: `/home/ricperry1/projects/Toniator`
Git HEAD: `262c7e857446ded100d4a90fd23d651e52460665` (dirty worktree preserved)

## Accepted boundary

Project-embedded custom Shapes-compatible definitions and value-only recipe
instances are now persisted inside `PatternDocumentState`. Selection requires
an embedded definition; invalid or missing content fails validation without a
built-in fallback. `DocumentEditor` installs and selects a validated custom
definition as one undoable edit. Live rendering dispatches the selected
embedded definition before the derived `RenderVariant` facade through the same
bounded Shapes native registry, artwork pipeline, cancellation, and canonical
output path.

## Parent checks

- `model::tests::embedded_custom_recipe_is_authoritative_and_undoable` passed.
- `model::tests::custom_selection_without_embedded_definition_fails_validation`
  passed.
- `model::tests::invalid_embedded_recipe_install_is_inert` passed.
- `render::tests::embedded_custom_shapes_selection_dispatches_before_legacy_render_adapter`
  passed.
- `persistence::tests::current_v9_roundtrips_embedded_custom_recipe_and_rejects_missing_selection`
  passed.
- Full library tests passed (249); the writer also reports 48 binary/UI tests,
  all-target/release checks, strict Clippy, format, diff, and bounded startup.

## Limits

This boundary is runtime/model-only. The GTK pattern editor, user-library
Save As/import, custom preset channel handling, and dedicated custom PNG/SVG
parity fixtures remain pending. Manual GNOME/Wayland acceptance remains
pending.

## Invalidation

Invalidate if embedded definition/instance serialization, Shapes operation
validation/execution, `DocumentEditor` install semantics, live dispatch, or
dirty-checkout assumptions change.
