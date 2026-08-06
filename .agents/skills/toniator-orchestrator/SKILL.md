---
name: toniator-orchestrator
description: Coordinate focused Toniator greenfield design, exploration, implementation, and verification through project custom subagents.
---

# Toniator Orchestrator

Own integration in the parent thread. Read relevant greenfield evidence before
choosing an agent, validate it against Git HEAD and the worktree, and select
only specialists whose output materially reduces uncertainty.

Use one writer for one short bounded stage. The parent settles the stage
decision, grants exact paths and acceptance checks, then reviews the writer's
report and evidence before handing off any later stage. A report is evidence,
not approval. Do not start the next stage automatically.

Use `codebase_explorer` only for a specific unresolved path after cache review.
It explores greenfield first; Legacy is a read-only, explicitly named quarry.
Use `desktop_implementer` for bounded writing and routine verification. Use a
UX or test reviewer only when the actual risk warrants independent review.
Use `documentation_maintainer` only after a verified milestone materially
changes durable documentation.

The greenfield core, CLI, and GTK app remain hard boundaries: no headless crate
depends on GTK/libadwaita or a frontend; `toniator-engine` is the future shared
pipeline boundary; canonical geometry is shared by preview and exports. Do not
use Legacy as architecture authority or add broad compatibility layers.

Keep cache entries under `.codex-work/` generation-marked and checkout-aware.
Never write, build, test, or format `ToniatorLegacy/`. Do not edit protected
normative specification files unless the parent explicitly assigns a
specification revision. Do not commit, push, publish, deploy, or delete.
