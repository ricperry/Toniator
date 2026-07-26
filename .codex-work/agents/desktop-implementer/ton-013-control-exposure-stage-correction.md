# TON-013 control-exposure stage correction

- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `546ea4c5eb1fec8e91c2b307545e33e42331e308`
- Timestamp: `2026-07-26`
- Producing agent: `desktop-implementer`
- Scope: bounded correction to the current TON-013 control-exposure stage.

## Git and working-tree assumptions

The checkout began dirty at the recorded HEAD. Existing modified files were
`.codex-work/cache-index.md`, `.codex-work/project-rehydration.md`, `ISSUES.md`,
and `src/ui.rs`; pre-existing untracked TON-013 evidence, backups,
`docs/UI_ARCHITECTURE.md`, and `resources/` were preserved. The requested
independent UX review was read before implementation. No reset, revert, commit,
deletion, publish, or Cambalache round-trip was performed. This report covers
only the additional correction layered onto that dirty state.

## Exact files changed for this correction

- `src/ui.rs`
- `resources/ui/ToniatorEditorControls.ui`
- `resources/ui/Toniator.cmb`
- `ISSUES.md`
- `docs/UI_ARCHITECTURE.md`
- `test-artifacts/ton-013/control-exposure-stage-corrected.png` (runtime artifact)
- `.codex-work/agents/desktop-implementer/ton-013-control-exposure-stage-correction.md`

## Implementation decisions and reused abstractions

The five Builder-owned Source/Output dropdowns now receive explicit accessible
names and `LabelledBy` relations to their existing stable Builder labels:
Artwork Source, Source Alpha, Output Model, Channel Assignment, and Active
Channel. `configure_dropdown_accessibility` centralizes the binding and keeps
the controls keyboard-focusable.

`ToniatorEditorControls.ui` now owns four Basic/native treatment row shells,
including stable row, label, control-container, and `GtkScale` IDs for Sampling
Detail, Coverage, Contrast, and Screen Angle. Rust retrieves those Builder
scales through `BuilderScaleSpec`, configures the original range/step/format
behavior, preserves disabled pointer-scroll adjustment, and attaches the same
adjustment-backed precision spin entries. The resource contains one scale per
native row and Rust no longer appends duplicate native `control_row` widgets.

The correction reuses `sync_dropdown_strings`, existing scale callbacks and
gesture handling, `precision_entry`, deferred synchronization, invalid-position
rejection, bounded `RefCell` borrowing, semantic `OutputChannelId` mapping, and
the separate aggregate treatment scope. Dynamic model ownership, callbacks,
visibility/sensitivity, treatment semantics, rendering, and crash protections
remain Rust-owned.

## Verification and artifacts

- `cargo fmt --check` — passed before final evidence write.
- `cargo test --locked --bin toniator ui::tests::realized_numeric_controls_leave_continuous_scroll_to_parent -- --exact` — passed; realizes and focuses all five dropdowns, verifies their Builder labels/roles, and verifies each native Builder scale range and exactly one precision entry.
- `cargo test --locked` — passed: 117 library tests and 46 binary/UI tests.
- `cargo clippy --locked --all-targets --all-features -- -D warnings` — passed.
- `cargo build --release --locked` — passed.
- `xmllint --noout resources/ui/Toniator.ui resources/ui/ToniatorInspector.ui resources/ui/ToniatorEditorControls.ui resources/ui/ToniatorChannelControls.ui resources/ui/ToniatorAggregateChannelControls.ui resources/ui/Toniator.cmb` — passed.
- `git diff --check` — passed before final evidence write.
- `cargo run --locked -- --demo --expand-document --window-size 1000x980 --screenshot test-artifacts/ton-013/control-exposure-stage-corrected.png` — passed; the 1000x980 RGBA PNG was inspected. Source, Output, Channel Settings, and their live controls render cleanly; Treatment Settings remains intentionally collapsed by artifact-mode progressive disclosure.
- `ToniatorEditorControls.ui` SHA-256: `203599b2461e8dd95c37e6e37f54460e3a9780ca2092f5780c3dce2eea7e6280`, recorded in `Toniator.cmb`.

## Boundary, limitations, and review targets

TON-013 remains In Progress. The corrected Builder boundary includes the
Source/Output accessible dropdown bindings and Basic/native treatment rows.
Shapes, Curves, and Motif detail rows remain Rust-built inside the stable
`web_panel_host` and `curve_panel_host`; this correction does not claim their
migration or TON-013 completion. Follow-up review should cover those rows only
when their mixed-value behavior, help placement, custom drawing, focus order,
and callback lifecycle can be preserved. A future screenshot path may expose
the collapsed Treatment Settings section specifically, but no product behavior
or artifact CLI flags were broadened for that purpose.

## Documentation and invalidation

`ISSUES.md` and `docs/UI_ARCHITECTURE.md` describe the corrected boundary and
explicitly retain the remaining Shapes/Curves/Motif work. Revalidate this entry
if `src/ui.rs`, any `resources/ui/*` resource or CMB hash, the focused realized
GTK test, GTK/libadwaita behavior, the screenshot artifact, Git HEAD, or the
preserved dirty-worktree assumptions change.
