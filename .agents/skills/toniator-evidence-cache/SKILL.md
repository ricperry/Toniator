---
name: toniator-evidence-cache
description: Maintain reusable, checkout-aware evidence for Toniator exploration, implementation, review, and documentation work.
---

# Toniator Evidence Cache

Use `.codex-work/` as a reusable evidence layer, never as an authority over the
current checkout. Read only entries relevant to the assigned subsystem before
broad exploration. Reuse an entry only when its rewrite generation, subsystem,
Git HEAD, relevant files, and working-tree assumptions still match the task.
Validate those conditions against the current repository before relying on it.

Read `docs/GREENFIELD_REWRITE_PLAN.md` and `ProgressTracker.md` as durable
scope/status inputs, and validate the tracker against HEAD and the worktree.
Evidence may explain a transition but cannot advance tracker status or replace
parent review, user acceptance, or a checkpoint.

For source, sampling, rendering, preview, or export evidence, record which
baseline files from `assets/` were exercised and verify their documented
SHA-256 values. Derived artifacts never replace the baseline inputs.

Greenfield entries must state `generation: greenfield-rewrite`. Evidence from
the legacy tree is invalid by default for greenfield decisions, even if an
algorithm or workflow has a similar name. Do not write evidence under
`ToniatorLegacy/`, and do not build, test, format, or otherwise mutate it.

Keep verified findings separate from inferences and uncertainty. Cache use
should reduce repository hydration, not replace inspection when files changed.
Evidence is not durable product documentation and never supersedes the
normative files under `Project Specification/`; the Addendum has precedence
when those documents conflict.

An agent with workspace-write access updates the appropriate cache directory
directly. A read-only agent returns a final section titled exactly
`CACHE_UPDATE`; the parent thread persists that section directly under
`.codex-work/` immediately after collecting it. Never spawn another agent just
to summarize or persist cache evidence.

## Required entry fields

- `generation: greenfield-rewrite`
- Repository absolute path
- Git HEAD
- Relevant working-tree state or dirty files
- Producing agent
- Task or question
- Subsystems, files, and symbols inspected
- Verified findings
- Inferences and unresolved uncertainty
- Commands run and artifacts produced
- Exact changed files, when applicable
- Invalidation conditions
- Timestamp
