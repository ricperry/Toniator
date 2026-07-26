# TON-012 Stage 3 documentation reconciliation

- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `bac55f70e7a77ec638b8033d7801fa07141d4d7e`
- Timestamp: `2026-07-26`
- Producing agent: `documentation-maintainer`
- Task: Reconcile durable documentation for the completed and reviewed TON-012 Stage 3 implementation.

## Documentation files reviewed

- `README.md`
- `ISSUES.md` (TON-012 section only)
- `docs/ARTWORK_PIPELINE.md`
- `docs/ARTWORK_PIPELINE_AUDIT.md`

## Documentation files changed

- `README.md`
- `ISSUES.md` (TON-012 section only)
- `docs/ARTWORK_PIPELINE.md`
- `docs/ARTWORK_PIPELINE_AUDIT.md`

## Implementation evidence used

- `src/artwork_pipeline.rs`: source, alpha-policy, output-model, semantic channel, assignment, stable-ID, and output-transition definitions.
- `src/model.rs`: authoritative pipeline edits, one-entry undo behavior, output-mode cache transitions, Crosshatch restoration, and focused regression tests.
- `src/ui.rs`: shared Document controls, direct callbacks, conditional visibility, source guidance, Crosshatch action, realized GTK checks, stable StringList synchronization, and invalid-position rejection.
- `.codex-work/agents/desktop-implementer/ton-012-stage-3-correction-implementation.md`: reviewed correction evidence, verification counts, screenshot evidence, and remaining limitations.
- `.codex-work/agents/desktop-implementer/ton-012-stage-3-implementation.md` and `.codex-work/evidence/ton-012-stage-3-creative-usability-review.md`: initial Stage 3 implementation and bounded UI review context.
- `src/persistence.rs` and `src/preset.rs`: current v6 project/v3 preset acceptance and explicit rejection of unsupported pre-release formats.

## Stale or contradictory documentation found

- Architecture status still said Stage 2 was complete and the UI refactor was planned.
- Audit status still described a Stage 0-only document and had Stage 3/4 in the old order.
- TON-012 still reported Stage 2 as the current completion boundary and lacked the Stage 3 completion record.
- README described the combined Artwork Mapping workflow and obsolete document schema versions.
- Historical v1-v5 migration wording was clarified so it is not presented as a currently supported load path.

## Remaining documentation gaps

- Stage 4 preset scoping/cleanup remains intentionally undocumented as shipped behavior.
- Final RGB Curves completion, broad preview/export parity, and manual creative-workflow click-through remain open.
- The current RGB-mode Crosshatch raster/PNG versus SVG blend discrepancy remains documented as a later parity decision.

## Git HEAD and working-tree assumptions

- HEAD was `bac55f70e7a77ec638b8033d7801fa07141d4d7e`.
- Pre-existing dirty implementation changes were present in `src/artwork_pipeline.rs`, `src/model.rs`, and `src/ui.rs`; they were not edited by this documentation pass.
- Pre-existing untracked `AGENTS.md` and `.codex-work/` were preserved. No commit, push, reset, source edit, preset edit, or unrelated issue edit was performed.

## Invalidation conditions

Invalidate this entry if HEAD changes, the TON-012 implementation files or
verification evidence change, the v6/v3 format policy changes, Stage 4 begins,
or the documented UI/undo/cache/Crosshatch contracts are revised.
