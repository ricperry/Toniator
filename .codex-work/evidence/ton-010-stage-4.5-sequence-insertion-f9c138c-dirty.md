# TON-010 Stage 4.5 — sequence insertion and gate contract

- Recorded: 2026-07-28
- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `f9c138c493a9d687b5300abddf14e78281f2ad63`
- Parent: orchestrator
- Scope: tracker and orchestration update only; no 4.5 substage started

## Decision

Inserted Stage 4.5, “Baseline restoration and framework demonstrability,”
between technically accepted Stage 4 and Stage 5 Weighted Voronoi. Stage 4
remains complete and is explicitly not failed or superseded. Stage 5 is now
blocked until the user explicitly accepts Stage 4.5D.

## Four independent gates

- 4.5A: read-only pre-TON-013 comparison, complete shape-editor lost-feature
  inventory, and testing-preset matrix; stop for approval.
- 4.5B: restore the complete Blueprint shape editor, reconnect behavior, and
  add GTK regression coverage plus visual artifacts; stop for approval.
- 4.5C: add visibly distinct current-format testing presets and fixtures for
  authority, schemas, persistence, undo/redo, adapters, CMYK/RGB, and
  preview/PNG/SVG; reject obsolete schemas; stop for approval.
- 4.5D: integrated automated and manual readiness review establishing the
  accepted pre-Voronoi baseline; stop for explicit approval.

No substage auto-advances. The orchestration contract requires one bounded
assignment at a time, intermediate reports and parent evidence/cache updates,
patient status checks for progressing agents, preservation of blocked work,
and parent-owned acceptance.

## Files and validation

- Updated `ISSUES.md` with the nine-gate sequence, four 4.5 substage statuses,
  the Stage 5 dependency, and explicit preservation of Stage 4 acceptance.
- Updated `AGENTS.md` with the durable 4.5 approval boundary.
- Added `docs/TON-010_STAGE_4_5_BASELINE_RESTORATION.md` with deliverables,
  acceptance checks, and orchestration rules.
- Updated `docs/TON-010_STAGE_4_SCHEMA_UI.md` to point at the inserted gate.
- `git diff --check` passed.

No implementation, historical audit, preset creation, fixture creation,
manual workflow review, or Weighted Voronoi work was performed.

