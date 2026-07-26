# TON-013 GtkBuilder migration seams

- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `546ea4c5eb1fec8e91c2b307545e33e42331e308`
- Working tree: only untracked `.codex-work/backups/`
- Producing agent: codebase explorer
- Timestamp: 2026-07-26
- Task: identify a safe first GtkBuilder/Cambalache migration boundary for TON-013

## Verified findings

- Top-level shell construction is in `src/ui.rs:1763-1917`, with the shell at
  `1812-1852` and Rust inserting the `start` and `editor` pages at
  `1897-1917`.
- `build_start_view` is at `src/ui.rs:8090-8163`; hero loading and the
  conditional recovery button remain Rust-managed.
- Document controls are constructed at `src/ui.rs:8956-9018`; callbacks are
  around `2442-2586`, synchronization around `5432-5518`, and semantic mapping
  helpers around `9788-9895`.
- The current source has no `.ui` resources, `build.rs`, GResource manifest, or
  reusable channel composite. `Cargo.toml` has no resource-build dependency.
- Existing realized GTK tests cover invalid list positions and model identity at
  `src/ui.rs:12389-12568`; `sync_dropdown_strings` preserves `StringList`
  identity at `9921-9939`.

## Recommended first boundary

Create a main `Toniator.ui` resource owning
`AdwApplicationWindow -> AdwToolbarView -> AdwHeaderBar -> AdwToastOverlay -> GtkStack`,
with stable IDs for header actions and `start`/`editor` placeholders. Rust
continues to insert the two page widgets and owns dynamic content, visibility,
sensitivity, callbacks, drawing, dialogs, synchronization, and crash
protections. Static dropdown shells can move later; a reusable per-channel
composite should follow after its semantic model and lifecycle are defined.

## Unresolved uncertainty

Choose between simple compile-time `Builder::from_string` loading and a formal
GResource/build-script pipeline. Either keeps the checked-in `.ui` editable in
Cambalache; a formal GResource pipeline is the more durable packaging choice.

## Commands and artifacts

The explorer used `sed`, `nl`, `rg`, `find`, `wc`, `git rev-parse HEAD`, and
`git status --short`; it inspected the existing `.codex-work` cache and TON-013
issue text. No source files were edited.

## Invalidation

Invalidate after changes to the listed source/build/packaging files, a new
commit, or working-tree changes beyond `.codex-work/backups/`.
