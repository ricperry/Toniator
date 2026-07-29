# Desktop implementer — TON-010 Stage 4.5C2B-1 adapter authority

Date: 2026-07-28

## Scope and baseline

Implemented only C2B-1 in `/home/ricperry1/projects/Toniator` at Git HEAD
`f9c138c493a9d687b5300abddf14e78281f2ad63`. The worktree was intentionally
dirty before work began; all unrelated changes were preserved. No commit,
push, preset/schema migration, fixture change, CMYK/RGB transition work, C3,
4.5D, or Stage 5 work occurred.

## Files changed

- `src/model.rs` — Crosshatch entry now reads selected/settings authority from
  `PatternDocumentState`; focused contradictory-adapter undo/redo regression.
- `src/persistence.rs` — production C1 preset/render/save/reopen/history
  contradiction matrix for Shapes and Curves.
- `.codex-work/evidence/ton-010-stage-4.5c2b1-adapter-authority-f9c138c-dirty.md`
- this implementation record.

## Defect and implementation decision

The ordinary runtime renderer, serializer, loader, and UI were already
canonicalizing/rebuilding derived adapters. The audit found a narrower
exception: Crosshatch entry selected its source by matching `Document.render`.
The correction uses the existing `PatternDocumentState::selected_pattern_id`,
`shape_settings`, and `curve_settings` accessors instead, then retains the
existing Crosshatch snapshots, pipeline projection, history, and rendering
flow. This avoids a second authority without rewriting the transition.

## Verification

Focused C2B-1 render/save/reopen/undo/redo, Crosshatch authority, existing
model/render/C2A, and realized shipping GTK selector tests passed. Full
`cargo test --locked` passed (143 library and 48 binary/UI tests), as did
formatting, locked all-target check, strict locked Clippy, and diff check.

## Artifacts, limitations, and follow-up

No C3 artifact was generated. The realized GTK test exercises the shipped
Builder/GResource AppUi but no manual GNOME/Wayland or screen-reader
acceptance is claimed. C2B-2 is the sole follow-up for CMYK/RGB cache and
transition contradictions; documentation reconciliation remains parent-owned.

## Invalidation conditions

Re-run C2B-1 if pattern projection, Crosshatch entry/exit, persistence
canonicalization, legacy renderer dispatch, the C1 presets, or selector
synchronization changes.
