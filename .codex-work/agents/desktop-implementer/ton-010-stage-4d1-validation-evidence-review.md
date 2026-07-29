# TON-010 Stage 4D1 — UI authority validation and evidence review

- Recorded: 2026-07-28
- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `f9c138c493a9d687b5300abddf14e78281f2ad63`
- Producing agent: desktop-implementer
- Scope: read-only final validation of completed Stage 4 authority/schema UI
  migration. Parent owns acceptance and durable documentation.

## Worktree basis and changes made

The worktree remains intentionally dirty with completed TON-010/TON-013 work,
resources, presets, fixtures, evidence, and documentation.  This validation
did not modify source, schemas, presets, fixtures, documentation, tracker, or
product behavior.  The only new file is this `.codex-work` validation record;
there was no commit, push, reset, cleanup, or Weighted Voronoi work.

## Evidence reviewed and current-code inspection

Reviewed the Stage 4A, 4B, 4C1, 4C2a, and 4C2b implementation evidence and
validated it against current `src/ui.rs`, the same HEAD, and the preserved
dirty-worktree baseline.

Verified production `src/ui.rs` authority seams:

- selector and panel synchronization: `sync_pattern_selector` uses selected
  pattern state plus registry selector metadata;
- Shapes and Curves parameter synchronization: `sync_controls` obtains typed
  settings only through `pattern_state.shape_settings()` /
  `pattern_state.curve_settings()`;
- Shapes and Curves edits: `change_web_treatment` /
  `change_curve_treatment` write only through `DocumentEditor::set_shape_settings`
  / `DocumentEditor::set_curve_settings`;
- schema binding: `sync_shapes_schema_metadata` and
  `sync_curves_schema_metadata` use `PATTERN_REGISTRY.parameter_for_control`;
- artboard, direct Curve editor/path/color, motif overlay, and editing context
  read selected authoritative state; Crosshatch/output semantics use
  `ArtworkPipelineSettings`.

## Adapter inventory and rationale

There are zero production `RenderVariant` or `Document.render` references in
`src/ui.rs` before the test module (`mod tests` begins at line 11328 at review
time).  `RenderVariant` is imported only under `#[cfg(test)]`.

The remaining test-only adapter uses deliberately construct contradictory
transient Shapes/Curves adapters and verify legacy projection/undo behavior.
They are retained because the Stage 4 contract requires proving that UI
authority ignores an adapter that disagrees with `Document.pattern_state`; they
do not participate in production selector or parameter reads.

## Commands and results

All passed:

```text
cargo test --locked --bin toniator ui::tests::realized_numeric_controls_leave_continuous_scroll_to_parent -- --exact
cargo test --locked --lib model::tests::authority_read_accessors_ignore_a_contradictory_transient_adapter -- --exact
cargo test --locked --lib pattern::tests::registry_exposes_shapes_and_curves_control_descriptors -- --exact
cargo test --locked
cargo check --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --all -- --check
git diff --check
```

Full results: 138 library tests and 46 binary/UI tests passed.  The realized
GTK test covers selector/panel authority, live `DropDown` model identity and
deferred synchronization, contradictory Shapes and Curves adapter values,
authoritative scalar edits, Curve editor/path/color, motif overlay, profile
persistence, descriptor metadata, and pipeline-owned Crosshatch context.

## Manual-validation limitation

No manual GTK/Wayland click-through, screenshot, export artifact, desktop
launch, or post-run core inspection was performed in this review.  Automated
realized GTK/resource coverage is evidence of callback/resource behavior, not
manual visual, accessibility, or interaction acceptance.

## Remaining uncertainty and invalidation

Parent acceptance and durable documentation reconciliation remain pending by
design.  Re-run this review if `src/ui.rs`, Stage 4A authority accessors,
registry descriptors/control IDs, artwork-pipeline Crosshatch semantics, GTK
resource/model synchronization, or the dirty-worktree/HEAD baseline changes.
