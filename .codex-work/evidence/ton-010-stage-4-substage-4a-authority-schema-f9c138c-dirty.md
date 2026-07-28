# TON-010 Stage 4 Substage 4A parent handoff

Date: 2026-07-28
Repository: `/home/ricperry1/projects/Toniator`
HEAD: `f9c138c493a9d687b5300abddf14e78281f2ad63` with the existing dirty
TON-010/TON-013 worktree preserved.
Producing writer: `desktop_implementer` agent `019faa4b-ec1b-7403-96b0-d16f31cb38ed`

## Deliverable

Stage 4A established the authority/schema read contract before UI refactoring.
`PatternDocumentState` now exposes read-only selected-pattern, selected-metadata,
selected-parameter-record, Shapes-settings, and Curves-settings accessors. The
pattern registry and metadata expose stable control-ID descriptor lookup. The
UI-facing types are re-exported from `src/lib.rs`; writes remain on
`DocumentEditor` authority APIs. No `src/ui.rs` changes were made.

## Parent review

The model test deliberately leaves a contradictory transient `RenderVariant`
adapter while authoritative `pattern_state` selects and stores Shapes. The new
accessors continue to return the authoritative selection and settings. Registry
tests discover current Shapes and Curves descriptors by stable control ID.
The implementation is bounded to `src/model.rs`, `src/pattern.rs`, and
`src/lib.rs`; no obsolete-format migration or new pattern work was introduced.

## Verification

- `cargo test --locked --lib` — 138 passed.
- `cargo check --locked --all-targets` — passed in the writer handoff.
- `cargo fmt --all -- --check` — passed in the writer handoff.
- `git diff --check` — passed in the writer handoff and parent review.

## Safe handoff

Substage 4B is the next explicit handoff: migrate selector synchronization and
selection callbacks in `src/ui.rs` to authoritative `pattern_state` accessors,
without beginning the broad parameter callback migration. GTK runtime checks
are required at that boundary. This record is progress evidence, not Stage 4
acceptance.

Invalidation: changes to `src/model.rs`, `src/pattern.rs`, `src/lib.rs`, the
relevant Stage 2/3 authority contract, or the dirty worktree invalidate this
handoff and require parent reinspection.
