# TON-013 control-exposure stage implementation

- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `546ea4c5eb1fec8e91c2b307545e33e42331e308`
- Timestamp: `2026-07-26`
- Producing agent: `desktop-implementer`
- Stage: bounded Builder exposure for Source, Output, Appearance, and treatment chrome/shells

## Git and working-tree assumptions

The checkout began dirty. Existing modified files were `.codex-work/cache-index.md`,
`.codex-work/project-rehydration.md`, `ISSUES.md`, and `src/ui.rs`; existing
untracked Stage 1/2 evidence, backups, `docs/UI_ARCHITECTURE.md`, and
`resources/` were preserved. No reset, revert, commit, deletion, publish, or
Cambalache round-trip was performed. This entry describes only the additional
bounded control-exposure changes layered onto that state.

## Exact changed files for this stage

- `src/ui.rs`
- `resources/ui/ToniatorEditorControls.ui` (new)
- `resources/ui/Toniator.cmb`
- `docs/UI_ARCHITECTURE.md`
- `ISSUES.md`
- `test-artifacts/ton-013/control-exposure-stage.png` (new runtime artifact)
- `.codex-work/agents/desktop-implementer/ton-013-control-exposure-stage-implementation.md`

## Implementation decisions and reused abstractions

`ToniatorEditorControls.ui` owns static Source, Output, and Appearance layout:
labels, notes, dropdown shells, color-button shells, and the Crosshatch action.
It also owns treatment chrome (pattern buttons, preset actions, stack, native
panel, and named Shapes/Curves panel hosts). `src/ui.rs` retrieves these by
stable IDs and detaches their top-level groups from the Builder root before
inserting them into the existing inspector hierarchy.

The implementation reuses `sync_dropdown_strings` to retain live
`GtkStringList` identity, `help_handle`/`help_button` for runtime help,
`build_inspector_hierarchy`, `EditorWidgets`, existing callbacks, deferred
synchronization, and `GtkStackPage::set_name` for native/web/curve page names.
It preserves the invalid-position, deferred-model-sync, and bounded-`RefCell`
crash protections. Channel templates and aggregate scope are unchanged.

## Ownership boundary

Rust retains dynamic source/status content, dropdown model values, callbacks,
conditional visibility/sensitivity, help popovers, custom curve/motif
`DrawingArea` content, mixed-value status, rendering, and semantic document
state. Shapes/Curves/Motif detail rows remain Rust-built inside the stable
`web_panel_host` and `curve_panel_host`; no duplicate visible widgets were
created.

## Verification and artifacts

- `cargo fmt --check` — passed.
- `cargo test --locked` — passed: 117 library and 46 binary/UI tests.
- `cargo clippy --locked --all-targets --all-features -- -D warnings` — passed.
- `cargo build --release --locked` — passed.
- `xmllint --noout resources/ui/*.ui resources/ui/Toniator.cmb` — passed.
- `git diff --check` — passed.
- Focused realization: `cargo test --locked --bin toniator ui::tests::realized_numeric_controls_leave_continuous_scroll_to_parent -- --exact` — passed, including Builder IDs, realized controls, model identity, semantic callback, and channel-scope coverage.
- Runtime artifact: `cargo run --locked -- --demo --expand-document --window-size 1000x760 --screenshot test-artifacts/ton-013/control-exposure-stage.png` — passed. The resulting 1000x760 PNG was inspected; migrated Source/Output rows, Channel Settings scope, Legacy Crosshatch action, and healthy preview were visible.
- Current resource hashes: `ToniatorEditorControls.ui` is `147a6e13c971c9dcff008e3adc5cb61f11d74950720c2691f45d228fdd429ad8`; all resource hashes are recorded in `Toniator.cmb`.

## Known limitations and review targets

TON-013 remains In Progress. The detailed Shapes, Curves, and Motif row layout
is still created in Rust, though its stable stack hosts and treatment chrome
are Builder-owned. A follow-up should migrate those rows incrementally with
mixed-value, help-placement, focus-order, and custom-DrawingArea checks. A
narrow-window and assistive-technology review remain useful after that follow-up.

## Documentation and invalidation

`docs/UI_ARCHITECTURE.md` and the TON-013 issue entry now describe this stage
without claiming completion. Revalidate this report if `src/ui.rs`, any
`resources/ui/*` file or CMB hash, Builder-ID tests, GTK/libadwaita versions,
the screenshot path, Git HEAD, or the preserved dirty-worktree assumptions
change.
