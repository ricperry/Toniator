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
