# TON-010 Stage 4 Substage 4C1 parent handoff

Date: 2026-07-28
Repository: `/home/ricperry1/projects/Toniator`
HEAD: `f9c138c493a9d687b5300abddf14e78281f2ad63` with the existing dirty
TON-010/TON-013 worktree preserved.
Producing writer: `desktop_implementer` agent `019faa4b-ec1b-7403-96b0-d16f31cb38ed`

## Deliverable

Stage 4C1 migrated runtime Shapes parameter reads and edits in `src/ui.rs` to
`Document.pattern_state.shape_settings()` and
`DocumentEditor::set_shape_settings`. This covers Shapes synchronization,
shape editing, shared-mark helpers, treatment edits, artboard/context values,
and touched registry-derived labels/help/accessibility metadata. Crosshatch and
output semantics use `ArtworkPipelineSettings` rather than adapter fields.

Curves parameter reads/callbacks remain intentionally adapter-backed for the
separate 4C2 handoff. This is progress evidence, not Stage 4 acceptance.

## Parent review

The writer’s realized GTK regression installs contradictory Shapes authority and
transient adapter settings, verifies the authoritative widget values, edits a
Shapes control, and verifies the persisted pattern-state setting. Existing
selector, deferred synchronization, and DropDown model-identity checks remain
in the same GTK lifecycle.

## Verification

- `cargo test --locked` — 138 library tests and 46 binary/UI tests passed.
- Focused authority, realized GTK control, and resource tests passed.
- `cargo check --locked --all-targets` — passed.
- `cargo clippy --locked --all-targets -- -D warnings` — passed.
- `cargo fmt --all -- --check` — passed.
- `git diff --check` — passed.

No manual Wayland click-through or screenshot was claimed.

## Safe handoff

Substage 4C2 is the next explicit handoff: migrate Curves parameter reads,
callbacks, context/help, and schema descriptor binding from `RenderVariant` to
authoritative `pattern_state`, while preserving current Curves geometry and
Crosshatch transition behavior.

Invalidation: changes to the Shapes UI migration, authority accessors, registry
descriptors, GTK resource IDs, or current dirty worktree require reinspection.
