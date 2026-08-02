# TON-010 bundled recipes — Substage 3E3B parent review

Date: 2026-08-01
Repository: `/home/ricperry1/projects/Toniator`
Git HEAD: `262c7e857446ded100d4a90fd23d651e52460665` (dirty worktree preserved)

## Reviewed boundary

Curves live rendering now dispatches through
`execute_bundled_curves_recipe_cancellable` using the authoritative
`Document.pattern_state.curve_settings()` value. The retained whole-Curves
generator is test-only and remains an oracle/helper seam. No production
whole-generator call remains in the live document path.

## Parent checks

- `render::tests::live_curves_document_render_enters_recipe_not_retained_oracle`
  passed.
- `render::tests::live_curves_document_render_reads_authoritative_pattern_state_not_transient_adapter`
  passed.
- `render::tests::cancelled_live_curves_render_stops_before_recipe_or_retained_oracle_work`
  passed.
- `svg_export::tests::bundled_curves_recipe_consumers_match_retained_canonical_paths_through_live_dispatch`
  passed.
- `cargo test --locked curves_native --lib` passed (25 tests).
- `git diff --check` passed.

## Status and limits

Substage 3E3B is parent-accepted for live Curves recipe dispatch and the
covered preview/PNG/editable-SVG consumer parity. This does not constitute
manual GNOME/Wayland or reference-artifact acceptance. Schema/library/editor
work remains pending.

## Invalidation

Invalidate if live Curves dispatch, `Document.pattern_state` authority,
retained-oracle test gating, canonical consumers, cancellation behavior, or
the dirty checkout assumptions change.
