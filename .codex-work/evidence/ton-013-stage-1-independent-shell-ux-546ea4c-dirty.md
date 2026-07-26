# TON-013 stage 1 independent shell UX review

- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `546ea4c5eb1fec8e91c2b307545e33e42331e308`
- Working tree: modified `src/ui.rs`, `ISSUES.md`, `.codex-work/cache-index.md`;
  untracked `resources/`, `docs/UI_ARCHITECTURE.md`, stage evidence, and backups
- Producing agent: UX reviewer
- Timestamp: 2026-07-26
- Scope: independent review of the GtkBuilder shell migration and its 900x680
  GTK artifact

## Verdict

Pass with one minor documentation issue, corrected by the parent. The header
ordering, labels, icons, tooltips, title/subtitle updates, stack transition,
Rust-owned page insertion, and Controls behavior preserve the prior shell. The
artifact shows a coherent editor state.

## Finding and correction

The docs originally grouped all tooltips under static XML ownership, while the
Controls tooltip is rewritten by Rust on every toggle. `docs/UI_ARCHITECTURE.md`
now documents the `controls_toggle` tooltip and accessibility description as
runtime-owned.

## Deferred scope

Narrow-layout resizing below the default width and assistive-technology
inspection were not run. Dynamic editor controls, callbacks, models, canvas,
dialogs, and the reusable channel template remain intentionally outside this
stage and are not regressions.

## Evidence inspected

- `resources/ui/Toniator.ui`
- `docs/UI_ARCHITECTURE.md`
- `src/ui.rs` shell loader, page insertion, state updates, and shell tests
- `test-artifacts/ton-013/shell.png`

## Commands

The reviewer ran `git status --short`, `git rev-parse HEAD`, `sed`, `nl`, `rg`,
`git diff --no-ext-diff -- src/ui.rs`, `git diff --check`, `xmllint --noout
resources/ui/Toniator.ui`, `ls -lh`, and `file`.

## Invalidation

Invalidate after changes to shell XML, IDs/properties, page insertion/state
code, docs, screenshot artifact, GTK/libadwaita versions, Git HEAD, or relevant
working-tree assumptions.
