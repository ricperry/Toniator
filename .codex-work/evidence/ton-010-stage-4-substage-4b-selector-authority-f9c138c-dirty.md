# TON-010 Stage 4 Substage 4B parent handoff

Date: 2026-07-28
Repository: `/home/ricperry1/projects/Toniator`
HEAD: `f9c138c493a9d687b5300abddf14e78281f2ad63` with the existing dirty
TON-010/TON-013 worktree preserved.
Producing writer: `desktop_implementer` agent `019faa4b-ec1b-7403-96b0-d16f31cb38ed`

## Deliverable

Stage 4B migrated the Shapes/Curves pattern selector synchronization and
selection callbacks in `src/ui.rs` to authoritative `Document.pattern_state`
and registry metadata. Labels, help, accessibility metadata, active selector,
Legacy visibility, and active inspector panel no longer use `RenderVariant` to
choose the pattern. Selector writes continue through `DocumentEditor` authority
APIs; Crosshatch and saved-treatment caches remain transition-only support.

The remaining `RenderVariant` reads in `src/ui.rs` are parameter reads and
callbacks intentionally deferred to Substage 4C. This is a bounded handoff,
not Stage 4 acceptance.

## Parent review

The realized GTK regression covers contradictory authority/adapter pairs in both
directions, selector callbacks, active panels, registry labels, deferred
synchronization, and DropDown model identity. The selector-specific diff is
limited to `src/ui.rs` on top of the existing dirty UI work.

## Verification

- `cargo test --locked` — 138 library tests and 46 binary/UI tests passed.
- `cargo check --locked --all-targets` — passed.
- `cargo clippy --locked --all-targets -- -D warnings` — passed.
- `cargo fmt --all -- --check` — passed.
- `git diff --check` — passed.

No manual Wayland click-through or screenshot was claimed; the automated GTK
resource/realization test is the current evidence.

## Safe handoff

Substage 4C is the next explicit handoff: migrate Shapes/Curves parameter
control reads and callback bodies from `RenderVariant` to the Stage 4A
authority accessors, while binding visibility/help to registry descriptors.

Invalidation: changes to `src/ui.rs`, the Stage 4A authority/schema API,
registry metadata, GTK resource bindings, or the current dirty worktree require
parent reinspection.
