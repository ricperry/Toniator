# Toniator evidence cache index

This index points agents to reusable evidence under `.codex-work/`. It is
checkout-aware and must not be treated as authoritative over current files.

Add one entry per reusable cache record with:

- Cache key and relative entry path
- Repository absolute path, Git HEAD, and relevant dirty files
- Producing agent, task/subsystem, and timestamp
- Validity status or last validation
- Short scope note and invalidation conditions

Read the linked entry and validate it against the current checkout before use.
The parent thread records read-only-agent updates here after persisting them.

## Current entries

- `ton-013-control-exposure-documentation-reconciliation` —
  `agents/documentation-maintainer/ton-013-control-exposure-documentation-reconciliation.md`;
  reconciles current TON-013 control-exposure ownership, routing/scope and
  aggregate/channel semantics, verification counts, the 1000x980 artifact, and
  Cambalache round-trip limits against the dirty checkout on 2026-07-26.
  Invalidate after relevant source/UI/CMB, docs, evidence, artifact, Git HEAD,
  or dirty-worktree changes.

- `ton-013-control-exposure-independent-ux-546ea4c-dirty` —
  `evidence/ton-013-control-exposure-independent-ux-546ea4c-dirty.md`; historical
  pre-correction review that identified the five dropdown relations and
  native-row boundary correction. Superseded for current ownership by the
  control-exposure correction record. Invalidate after relevant UI, source,
  docs, artifact, GTK, HEAD, or dirty-worktree changes.

- `ton-013-control-exposure-stage-implementation` —
  `agents/desktop-implementer/ton-013-control-exposure-stage-implementation.md`;
  pre-correction bounded Builder ownership for Source/Output/Appearance,
  treatment chrome, and stable native/Shapes/Curves panel hosts, verified
  against dirty HEAD `546ea4c` on 2026-07-26. The correction record supersedes
  its accessibility/native-row details. Invalidate after `src/ui.rs`, UI/CMB,
  focused GTK tests, screenshot artifact, Git HEAD, or relevant dirty-file
  changes.

- `ton-013-control-exposure-stage-correction` —
  `agents/desktop-implementer/ton-013-control-exposure-stage-correction.md`;
  corrected five Source/Output accessibility relations and moved the four
  Basic/native scale row shells into Builder, preserving Rust adjustments and
  callbacks. Includes focused realized GTK coverage and the 1000x980 artifact.
  Invalidate after relevant source/UI/CMB/docs/tests/artifact/GTK/HEAD changes.

- `ton-013-control-exposure-inventory-546ea4c-dirty` —
  `evidence/ton-013-control-exposure-inventory-546ea4c-dirty.md`; read-only
  inventory confirming that most concrete Source, Output, Appearance, and
  Treatment controls remain Rust-created, with the practical Builder boundary,
  stable ID groups, and Rust-only dynamic/custom exceptions. Invalidate after
  changes to `src/ui.rs`, `resources/ui/*`, `Toniator.cmb`, GTK/libadwaita,
  focused tests, relevant docs, Git HEAD, or dirty-file assumptions.

- `ton-013-stage-2-documentation-reconciliation` —
  `agents/documentation-maintainer/ton-013-stage-2-documentation-reconciliation.md`;
  reconciled durable UI architecture and TON-013 Stage 2 tracker wording
  against the corrected implementation, CMB/UI resources, final UX review,
  and the 1000x760 screenshot artifact on 2026-07-26. Invalidate after
  relevant Stage 2 implementation, resource, documentation, evidence, or
  working-tree changes.

- `ton-013-stage-2-treatment-scope-correction` —
  `agents/desktop-implementer/ton-013-stage-2-treatment-scope-correction.md`;
  final semantic correction making Treatment Editing Scope the sole visible
  treatment recipient selector while Output routing remains authoritative.
  Includes focused tests and the inspected 1000x760 GTK artifact. Invalidate
  after target callbacks, pipeline controls, UI/CMB changes, or GTK changes.

- `ton-013-stage-2-treatment-scope-final-ux-546ea4c-dirty` —
  `evidence/ton-013-stage-2-treatment-scope-final-ux-546ea4c-dirty.md`; final
  review passes the corrected treatment scope, hierarchy, semantic templates,
  and 1000x760 GTK screenshot. Records focus/accessibility as follow-up.
  Invalidate after relevant Stage 2 changes.

- `ton-013-stage-2-independent-channel-inspector-ux-546ea4c-dirty` —
  `evidence/ton-013-stage-2-independent-channel-inspector-ux-546ea4c-dirty.md`;
  correction-required review: actual hierarchy placement, competing scalar vs
  treatment scopes, structural-only panel hosts, and expanded-state concerns.
  Invalidate after correction changes.

- `ton-013-stage-2-channel-control-architecture-546ea4c-dirty` —
  `evidence/ton-013-stage-2-channel-control-architecture-546ea4c-dirty.md`;
  current code ownership, semantic channel identity, aggregate-panel boundary,
  and safe Stage 2 implementation split.

- `ton-013-stage-2-channel-inspector-ux-546ea4c-dirty` —
  `evidence/ton-013-stage-2-channel-inspector-ux-546ea4c-dirty.md`; settled
  Source/Output/Channel Settings hierarchy, terminology, defaults, and
  aggregate-versus-real-channel acceptance criteria.

- `ton-013-stage-1-independent-shell-ux-546ea4c-dirty` —
  `evidence/ton-013-stage-1-independent-shell-ux-546ea4c-dirty.md`; reviewed
  the current GtkBuilder shell, docs, and real GTK screenshot on 2026-07-26;
  pass after correcting runtime ownership wording for the Controls tooltip.
  Invalidate after relevant shell, docs, version, or worktree changes.

- `ton-013-stage-1-documentation` —
  `agents/documentation-maintainer/ton-013-stage-1-documentation-reconciliation.md`;
  reconciled the Stage 1 architecture and TON-013 issue wording against the
  current shell resource, Rust loader/page insertion, review evidence, and
  GTK artifact on 2026-07-26. Invalidate after relevant shell, docs, evidence,
  version, or worktree changes.

- `ton-013-gtkbuilder-migration-seams-546ea4c` —
  `evidence/ton-013-gtkbuilder-migration-seams-546ea4c.md`; valid for HEAD
  `546ea4c` with only untracked `.codex-work/backups/`; safe first GtkBuilder
  boundary and current GTK/model stability seams. Invalidate after relevant
  source, build, packaging, or working-tree changes.

- `ton-012-stage-1b-complete` — `evidence/ton-012-stage-1b-complete.md`;
  validated against HEAD `32022df` on 2026-07-26; authoritative pipeline state,
  schema v6/v3 persistence, migration, and active/saved/inactive snapshots.
- `ton-012-stage-2-render-resolution-paths` —
  `evidence/ton-012-stage-2-render-resolution-paths.md`; validated against HEAD
  `32022df` on 2026-07-26; live decode, sampling, separation, alpha, consumer,
  and SVG export boundaries for Stage 2.
- `ton-012-stage-2-review` — `evidence/ton-012-stage-2-review.md`; validated
  against HEAD `32022df` on 2026-07-26; independent review findings and parent
  corrections for SVG semantic output and field-cache isolation.
- `ton-012-stage-3-implementation` —
  `agents/desktop-implementer/ton-012-stage-3-implementation.md` and
  `agents/desktop-implementer/ton-012-stage-3-correction-implementation.md`;
  validated against working tree based on HEAD `bac55f7` on 2026-07-26;
  shared semantic Document controls, direct callbacks, output-cache transition,
  Crosshatch restoration, and verification evidence.
- `ton-012-stage-3-review-current-head` —
  `evidence/ton-012-stage-3-review-current-head.md`; validated against the
  corrected working tree on 2026-07-26; GTK/state review findings and required
  corrections, now resolved by the implementation pass.
- `ton-012-stage-3-creative-usability-review` —
  `evidence/ton-012-stage-3-creative-usability-review.md`; validated against
  the corrected working tree on 2026-07-26; source/alpha/assignment guidance
  findings, now resolved by the implementation pass.
- `ton-012-stage-3-documentation` —
  `agents/documentation-maintainer/ton-012-stage-3-documentation-reconciliation.md`;
  validated against the corrected working tree on 2026-07-26; durable docs and
  issue-ledger reconciliation.
- `ton-012-stage-4-preset-ownership-current-head` —
  `evidence/ton-012-stage-4-preset-ownership-current-head.md`; validated
  against HEAD `236cdb1` on 2026-07-26; targeted preset ownership, call paths,
  bundled inventory, retained compatibility adapters, and Stage 4 format
  assumption.
- `ton-012-stage-4-preset-review-236cdb1` —
  `evidence/ton-012-stage-4-preset-review-236cdb1.md`; reviewed against the
  uncommitted Stage 4 diff on 2026-07-26; independent atomicity, scope,
  channel-identity, and nested-validation findings.
- `ton-012-stage-4-implementation` —
  `agents/desktop-implementer/ton-012-stage-4-implementation.md`; validated
  against the corrected uncommitted Stage 4 worktree on 2026-07-26; v4 scoped
  format, atomic application, bundled conversion, UI scope chooser, tests,
  and retained adapters.
- `ton-012-stage-4-creative-preset-review-236cdb1` —
  `evidence/ton-012-stage-4-creative-preset-review-236cdb1.md`; reviewed
  against the corrected uncommitted Stage 4 worktree on 2026-07-26; bundled
  naming/output, scope wording, Crosshatch framing, and visual findings.
- `ton-012-stage-5-rendering-parity-audit-current-head-4161635` —
  `evidence/ton-012-stage-5-rendering-parity-audit-current-head-4161635.md`;
  validated against HEAD `4161635` on 2026-07-26; targeted semantic preview,
  PNG/SVG, transition, appearance, and retained-adapter findings. Invalidate
  after relevant Stage 5 implementation edits.
- `ton-012-stage-5-independent-export-parity-review-4161635` —
  `evidence/ton-012-stage-5-independent-export-parity-review-4161635.md`;
  reviewed the Stage 5 implementation on 2026-07-26; found one major RGB
  Crosshatch SVG blend mismatch and minor test gaps. Invalidate after the
  correction changes rendering or export paths.
- `ton-012-stage-5-rendering-parity-implementation` —
  `agents/desktop-implementer/ton-012-stage-5-rendering-parity-implementation.md`;
  validated against the corrected current worktree on 2026-07-26; per-model
  Preview Surface cache, preview/export separation, semantic Curves SVG, and
  focused test evidence.
- `ton-012-stage-5-rgb-crosshatch-svg-correction` —
  `agents/desktop-implementer/ton-012-stage-5-rgb-crosshatch-svg-correction.md`;
  validated against the corrected current worktree on 2026-07-26; RGB-output
  Crosshatch SVG Multiply parity correction and regression evidence.
- `ton-012-stage-5-artifact-creative-output-review-4161635` —
  `evidence/ton-012-stage-5-artifact-creative-output-review-4161635.md`;
  reviewed the ignored Stage 5 visual/serialized artifacts on 2026-07-26;
  no blocker or major finding, with minor dark-viewer Crosshatch friction and
  opaque-source alpha fixture limitations recorded.
- `ton-012-closeout-4161635` — `evidence/ton-012-closeout-4161635.md`;
  final accepted TON-012 architecture, verification, retained adapters, and
  deferred-issue record prepared for the closeout commit.
