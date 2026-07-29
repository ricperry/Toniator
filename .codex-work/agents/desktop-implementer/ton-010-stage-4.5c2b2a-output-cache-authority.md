# Desktop implementer — TON-010 Stage 4.5C2B2-A output-cache authority

Date: 2026-07-28

## Scope and baseline

Bounded C2B2-A only at Git HEAD `f9c138c493a9d687b5300abddf14e78281f2ad63`; the pre-existing dirty worktree was preserved. C2B2-B, C2C, C3, 4.5D, Stage 5, migrations, and unrelated work were not started.

## Exact files changed

- `src/persistence.rs` — one production C1 fixture transition/cache contradiction regression.
- `.codex-work/evidence/ton-010-stage-4.5c2b2a-output-cache-authority-f9c138c-dirty.md`
- this record.

## Findings and reused abstractions

Reused `DocumentEditor::set_output_mode`, `Document::switch_output_mode`, `OutputTreatmentCache`, normal renderer canonicalization, atomic save/load, and the C1 production presets. The active and cache adapters are overwritten by `sync_legacy_projection` from each cache's authoritative `pattern_state`. No transition defect was found, so no product-source rewrite was justified.

The regression corrupts both active and inactive adapter kinds/parameters, then covers CMYK↔RGB cache creation/restoration, real render pixels, save/reopen, undo/redo, Preview Surface restoration, and stable Export Background through both Shapes and Curves fixtures.

## Verification

Focused C2B2-A and existing transition/presentation tests passed. Full locked tests passed (144 library, 48 binary/UI), as did locked all-target check, strict locked Clippy, formatting, and diff check.

## Artifacts and limitations

No C3 artifact or manual/runtime GTK interaction was created; realized UI transition verification is intentionally reserved for C2B2-B. Documentation reconciliation remains parent-owned.

## Invalidation conditions

Re-run C2B2-A if output transition/cache lifecycle, authority projection, persistence, history, C1 fixtures, or presentation ownership changes.
