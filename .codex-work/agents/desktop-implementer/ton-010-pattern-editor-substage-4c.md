# TON-010 Stage 5, Substage 4C — Pattern Editor

- Timestamp: 2026-08-01T17:33:58-04:00
- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `262c7e857446ded100d4a90fd23d651e52460665`
- Producing agent: `desktop-implementer` (`/root/ton010_recipe_contract_2a`)
- Scope: bounded custom Shapes Pattern Editor UI on top of accepted 4B runtime.

## Checkout assumptions

The checkout was materially dirty before 4C with prior TON-010 recipe/runtime/schema work, TON-013 Blueprint work, documentation, presets, and evidence. The only active writer was this agent and all existing changes were preserved. No reset, clean, revert, commit, push, publication, or deletion occurred.

## Exact files changed for 4C

- `resources/toniator-window.blp`
- `src/ui.rs`
- `.codex-work/agents/desktop-implementer/ton-010-pattern-editor-substage-4c.md`

## Inspected subsystems and reused abstractions

- Inspected the tracked Blueprint resource, `build_editor_view`, `AppUi::sync_controls`, the production preview worker, `after_treatment_edit`, autosave, XDG preset paths, and the 4B custom runtime evidence.
- Reused `adapt_shapes_settings_to_recipe`, `PatternDefinition`/instance validation, `DocumentEditor::install_and_select_embedded_pattern`, `after_treatment_edit`, `queue_autosave`, and `request_rendered_preview` rather than creating a parallel renderer or persistence path.

## Implementation

- Added accessible `Edit Pattern…` action in the existing treatment action group plus a resource-defined `custom_pattern_panel` with a concise project-pattern name/summary and an edit action. The resource also declares the modal draft controls. `sync_controls` now selects this panel for an embedded custom definition and does not label it as Legacy/native.
- The modal uses `AdwAlertDialog` with Blueprint-defined Pattern name, Circle/Regular Polygon/Triangle/User Defined mark choice, Grid density, and Grid spacing controls. Cancel merely dismisses the draft; it does not borrow/mutate the document or undo history.
- Apply derives a new `custom.<slug>.v1` definition and instance from the authoritative Shapes settings through the existing Shapes adapter, then changes only the declared draft parameters. It validates before calling the 4B atomic editor install/select API.
- A successful Apply clears rendered cache, queues normal autosave, synchronizes controls, requests the normal cancellable rendered preview, and updates actions. The focused test proves the installed draft generates non-empty production canonical marks.
- Save As first validates/serializes the definition, writes an atomic `.tnpattern` file beneath `$XDG_DATA_HOME/toniator/patterns` (or `$HOME/.local/share/toniator/patterns`) or a selected path, then applies/selects the same project-embedded definition. Bundled definitions are never written or altered.

## Verification

- Focused `pattern_editor_draft_is_nonmutating_until_one_install_edit`: draft construction leaves the document identical (Cancel contract); install is one undo entry; generated custom definition produces non-empty canonical marks.
- Focused resource test verifies the new Blueprint IDs and five-panel stack count; focused XDG path test verifies `.tnpattern` path normalization.
- `cargo test --locked` — passed: 249 library tests and 50 binary/UI tests.
- `cargo check --locked --all-targets` — passed.
- `cargo check --locked --release` — passed.
- `cargo clippy --locked --all-targets -- -D warnings` — passed.
- `cargo fmt --all -- --check` — passed.
- `git diff --check` — passed.
- Blueprint resource compiled through the Cargo resource build and the builder resource-contract test passed.
- `timeout 12s cargo run --locked` — application startup smoke completed to the running application boundary. No manual GNOME acceptance or screenshot is claimed.

## Artifacts

No persistent test `.tnpattern`, screenshot, PNG, or SVG artifact was created. The runtime path is exercised in-process via production canonical output, not fake preview geometry.

## Known limitations and review targets

- No live per-keystroke draft preview was added; the canonical preview is requested after Apply/Save As only.
- User Defined exposes the valid existing Shapes custom-motif default; freeform motif/path authoring remains the existing Shapes editor's responsibility and was not duplicated in this modal.
- The `AdwAlertDialog` provides the modal shell while the tracked Blueprint owns its draft field layout, discoverability, panel hierarchy, and stable object contract.
- Review manual GNOME/Wayland interaction: opening the modal, Cancel, Apply, Save As chooser, and custom panel copy, plus PNG/SVG parity for an editor-created custom pattern.

## Documentation likely affected

Stage 5 architecture and creator workflow documentation may require durable custom-pattern editor/save-path guidance after parent milestone review. This cache entry is implementation evidence, not durable documentation.

## Invalidation conditions

Reinspect if Blueprint compilation/resource IDs, `AppUi` preview/autosave lifecycle, the Shapes adapter, custom runtime/editor API, XDG storage policy, or custom selection synchronization changes. Evidence is valid only for the recorded HEAD and pre-existing dirty worktree.
