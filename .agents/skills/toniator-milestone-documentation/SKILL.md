---
name: toniator-milestone-documentation
description: Decide when Toniator milestones require durable documentation reconciliation and guide the documentation-maintainer agent.
---

# Toniator Milestone Documentation

Invoke `documentation_maintainer` only after a completed and verified milestone
materially changes durable architecture, workflows, document or preset formats,
interaction conventions, rendering/export behavior, build procedures, or a
substantial capability. It is not an automatic final stage for a foundation
shell, small correction, or internal refactor.
The parent may reconcile bounded documentation directly when delegation adds
no value. Model selection follows `toniator-astra-orchestrator` and the active
agent definitions; evidence-cache writes are proportional to reuse value.

The files under `Project Specification/` are protected normative inputs. The
Addendum supersedes conflicting text in the other four specifications. A
documentation-maintenance pass must not edit them or reinterpret them as
current implementation documentation unless the parent explicitly assigns a
normative specification revision.

Before changing durable documentation, inspect the milestone diff, verified
implementation evidence, and existing documentation structure. Document only
what the implementation proves; label future work as planned. Evidence under
`.codex-work/` aids review but does not replace durable documentation. Do not
create documentation files mechanically, and never commit, push, publish,
deploy, or delete implementations.

When a qualifying milestone changes GTK controls or interaction, preserve the
current accessibility contract in durable documentation: each changed control
is accounted for by stable product identity/hierarchy, real role/state/value or
selection/actions, label relation, enabled/applicability truth, keyboard path,
semantic action/readback, and visual/log evidence. Describe existing GTK and
descriptor authority rather than inventing a parallel accessibility model.

Distinguish the protected normative specifications from explicitly approved
plan revisions and routine tracker updates. Tracker maintenance alone does not
trigger a documentation-maintainer pass; protect an approved plan from
opportunistic rewrites and reconcile tracker text only to verified state.
