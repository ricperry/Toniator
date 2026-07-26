# TON-013 stage 1 GtkBuilder shell implementation

- Repository: `/home/ricperry1/projects/Toniator`
- Timestamp: 2026-07-26T14:56:10-04:00
- Git HEAD: `546ea4c5eb1fec8e91c2b307545e33e42331e308`
- Producing agent: `desktop-implementer`
- Task: migrate the first coherent static application shell boundary from Rust
  construction to an editable GtkBuilder resource without changing dynamic
  editor behavior.

## Git and working-tree assumptions

At handoff, HEAD was the value above and the explorer cache recorded only
untracked `.codex-work/backups/`. Before editing, the parent had also created
the untracked `evidence/ton-013-gtkbuilder-migration-seams-546ea4c.md` and a
modified `.codex-work/cache-index.md`; both were preserved untouched. No other
source changes were present. This implementation added the files listed below
and modified only `src/ui.rs`.

## Exact implementation files

- `resources/ui/Toniator.ui` (new): static
  `AdwApplicationWindow -> AdwToolbarView -> AdwHeaderBar ->
  AdwToastOverlay -> GtkStack` shell and all required stable action IDs.
- `src/ui.rs`: compile-time `include_str!` loader, typed shell-object lookup,
  retained runtime page insertion, and focused resource contract tests.
- `docs/UI_ARCHITECTURE.md` (new): stage boundary and future editing rules.
- `.codex-work/agents/desktop-implementer/ton-013-stage-1-gtkbuilder-shell-implementation.md`
  (this evidence entry).

## Verified findings and decisions

- `Toniator.ui` is embedded with `include_str!` and loaded using
  `gtk::Builder::from_string`. This avoids a new build dependency or
  build-script/GResource pipeline while storing a normal checked-in `.ui` file
  that is editable in Cambalache. The docs state the clean later GResource
  conversion path.
- `build_top_level_shell` retrieves typed objects by stable IDs, assigns the
  application, and restores the runtime-clamped default window size. `AppUi`
  remains the owner of its existing fields and behavior.
- Rust still constructs `build_start_view` and `build_editor_view`, then adds
  them to the Builder-owned `main_stack` under the stable runtime page names
  `start` and `editor`. They deliberately are not placeholder Builder objects.
- Header labels, Undo/Redo icon names, tooltips, title/subtitle behavior,
  crossfade/180ms stack transition, header layout, and the runtime Controls
  accessible description/toggle behavior remain equivalent to the former Rust
  construction.
- Reused abstractions: `AppUi`, `InspectorPaneController`, `build_start_view`,
  `build_editor_view`, and the existing realized GTK regression coverage.
  Dynamic `StringList` identity, invalid-position rejection, deferred control
  synchronization, bounded `RefCell` borrows, and no-refresh-dirty-state
  behavior were not moved or changed.
- The no-display focused test checks the XML contract and required stable IDs.
  The already-realized GTK regression test now parses the resource through
  GTK/libadwaita, checks typed object IDs, and verifies the stack transition.

## Verification

- `gtk4-builder-tool validate resources/ui/Toniator.ui` was attempted but
  cannot load libadwaita object types and reported `Invalid object type
  'AdwApplicationWindow'`; this is a host-tool limitation, not a resource
  failure. The actual GTK/libadwaita Builder path passed below.
- `cargo fmt --check` passed.
- `cargo clippy --locked --all-targets --all-features -- -D warnings` passed.
- `cargo test --locked` passed: 117 library tests and 44 binary/UI tests.
  The realized GTK test parsed the shell and retained dropdown crash guards.
- `cargo build --release --locked` passed.
- `git diff --check` passed.
- `cargo run --locked -- --demo --window-size 900x680 --screenshot
  test-artifacts/ton-013/shell.png` exited 0. The real GTK screenshot was
  inspected at `test-artifacts/ton-013/shell.png` (900x680): header actions,
  title/subtitle, toolbar layout, Controls state, and editor content rendered
  correctly.
- `coredumpctl list toniator --no-pager` showed only historical entries through
  2026-07-21; no new coredump followed the 2026-07-26 smoke launch.
- Cambalache Flatpak `ar.xjuan.Cambalache` 1.0.3 is installed; it was not
  launched, so the user's desktop was not driven or obscured.

## Artifacts

- Ignored runtime screenshot: `test-artifacts/ton-013/shell.png`.
- No exports were required by this shell-only stage.

## Known limits and follow-up review targets

- TON-013 is not complete. The editor hierarchy, selector-driven document
  controls, callbacks, custom DrawingAreas, dialogs, and dynamic models remain
  programmatic by design.
- No reusable per-channel composite was introduced. The controls are currently
  selector/model-driven rather than repeated channel rows; define its semantic
  model and lifecycle before considering a composite template.
- Reconsider a formal GResource/build pipeline only when resource packaging or
  more Builder files justify its maintenance cost. Verify GTK/libadwaita type
  support in the chosen validator as part of that conversion.
- Future review should inspect Cambalache round-trip compatibility, localized
  XML properties, and accessibility behavior after any Builder-side changes.

## Documentation affected

`docs/UI_ARCHITECTURE.md` is the durable stage documentation. Broad product
documentation and issue completion status were intentionally not changed;
TON-013 remains in progress.

## Invalidation conditions

Revalidate this entry after any change to `src/ui.rs`,
`resources/ui/Toniator.ui`, GTK/libadwaita dependency versions, shell widget
IDs/properties, page insertion behavior, test/runtime artifact code, Git HEAD,
or relevant working-tree assumptions.
