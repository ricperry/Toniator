# TON-010 Stage 4, Substage 4C1 — Shapes UI authority migration

- Recorded: 2026-07-28
- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `f9c138c493a9d687b5300abddf14e78281f2ad63`
- Producing agent: desktop-implementer
- Scope: Shapes runtime parameter reads and callbacks only; Curves remains for 4C2.

## Worktree assumption

The worktree was already intentionally dirty with completed TON-010 stages 1–4B,
TON-013 work, presets, fixtures, resources, evidence, and documentation.  Those
paths were preserved.  This substage changed `src/ui.rs` and added this evidence
file only; no commit, push, reset, cleanup, migration, preset, fixture, or
persistence change was made.

## Verified implementation

- `Document.pattern_state.shape_settings()` is now the runtime source for Shapes
  artboard dimensions, control synchronization, shape editing, treatment edits,
  shared-mark enabling, and Shapes editing-context terminology.
- `change_web_treatment` writes only through `DocumentEditor::set_shape_settings`.
  Existing edit/coalescing, undo, autosave, render refresh, deferred GTK sync,
  mixed-value handling, target/channel scope, and saved-cache transition behavior
  remain on their existing paths.
- Crosshatch/output semantics for Shapes use `ArtworkPipelineSettings` via
  `pipeline_uses_crosshatch` / `document_uses_crosshatch`, rather than inferring
  them from a `RenderVariant` adapter.
- `sync_shapes_schema_metadata` binds Shapes controls touched in this slice to
  Stage 4A registry descriptors through `PATTERN_REGISTRY.parameter_for_control`:
  shared-mark, mark shape, polygon sides, user-defined-mark editor, and visible
  channel/layer terminology receive descriptor labels/help/accessibility text.
- The only remaining non-test `RenderVariant::WebShapeV1` matches in `src/ui.rs`
  are non-parameter fallbacks: a default artboard fallback, the Curves-only
  `sync_controls` fallback arm, and the native fallback in editing-context text.
  None reads `WebShapeSettings`.
- Curves parameter reads/callbacks that still inspect `RenderVariant::WebCurveV1`
  were deliberately left untouched for 4C2.

## Tests and checks

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

Results: all commands passed.  Full test counts were 138 library tests and 46
binary/UI tests.  The realized GTK test deliberately installs a Shapes document
with contradictory transient `WebShapeV1` settings, asserts the authoritative
shared-mark toggle and mark-size value, edits mark size through the widget, and
asserts the persisted `pattern_state` setting.  It also retains selector panel
and live `DropDown` model-identity/deferred-synchronization coverage.  The
resource test verifies the editor control resource structure.  No manual GTK
session, screenshot, or export artifact was taken because this bounded
substage's realized GTK regression covers the touched behavior without claiming
manual visual acceptance.

## Reused abstractions and decisions

- Reused Stage 4A authority accessors and descriptor lookup; no new public API.
- Reused `DocumentEditor::set_shape_settings` for all Shapes writes.
- Reused the existing single GTK-initialization realized test to avoid a second
  `gtk::init` test and preserve GTK lifecycle safety.
- Retained model identity and idle/deferred synchronization behavior; no selector
  redesign or broad parameter rewrite was attempted.

## Known limitations / follow-up review targets

- 4C2 must migrate the remaining Curves parameter reads/callbacks away from
  `RenderVariant::WebCurveV1`, including its crosshatch terminology where the
  artwork pipeline is already the semantic owner.
- A later manual Fedora GNOME/Wayland interaction pass can visually confirm the
  descriptor-derived help/labels after the complete 4C migration; it was not
  represented as completed here.
- Durable documentation may need milestone reconciliation after Stage 4, but no
  documentation change is warranted for this bounded handoff.

## Invalidation conditions

Re-run this evidence if `Document.pattern_state` authority accessors, Shapes
descriptor control IDs/metadata, `ArtworkPipelineSettings` crosshatch semantics,
the GTK resource IDs, deferred dropdown synchronization, or Shapes callback
ownership changes.  This evidence does not validate any Curves migration.
