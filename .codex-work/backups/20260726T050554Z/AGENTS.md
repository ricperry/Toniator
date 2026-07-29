# Toniator Codex guidance

- Use `.agents/skills/toniator-evidence-cache/SKILL.md` for reusable evidence; its working cache lives under `.codex-work/` and is ignored by Git.
- Read-only agents return a final `CACHE_UPDATE` section to the parent. The parent persists reusable updates directly; no cache-maintenance agent is needed.
- Major milestones require sequential documentation reconciliation after implementation, review, and corrections. Use `.agents/skills/toniator-milestone-documentation/SKILL.md` and keep durable guidance concise.
