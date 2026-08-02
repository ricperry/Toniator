# TON-010 Pattern Editor / Channel Distribution Boundary

- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `262c7e857446ded100d4a90fd23d651e52460665`
- Timestamp: 2026-08-01
- Producing agent: `/root`
- Scope: move per-channel Shapes site-constructor state into the main Channel Settings panel, keep pattern structure in Pattern Editor, and remove the editor-only 8,192 random-site cap.

## Working-tree boundary

The checkout was already dirty. This change touched `src/model.rs`, `src/ui.rs`, `src/shapes_native.rs`, `src/pattern_definition.rs`, `resources/toniator-window.blp`, `resources/toniator-channel-controls.blp`, and this evidence/cache record. Existing TON-010 files, generated assets, documentation, presets, and the unrelated `ISSUES.md` draft were preserved; no reset, clean, commit, or push was performed.

## Verified implementation

- `WebShapeChannel` now persists `point_sampler` (`grid`, `uniform`, `weighted`), `random_seed`, and finite `weight_influence` (`0.001..=16.0`). `DocumentEditor::set_shape_channel_distribution` applies them as one undoable edit and synchronizes a selected embedded recipe's output-channel values when that recipe declares the parameters.
- Pattern Editor draft state contains only pattern structure and deformation: X/Y grid mode and curve amount, X/Y spacing, point definition, mark, density/scale, and jitter factor. Sampler, seed scope, unified seed, and per-channel seeds are not editor controls or draft fields.
- Custom recipe construction still declares the typed output-channel runtime parameters, but fills them from authoritative `WebShapeChannel` settings. The editor operation consumes `point-sampler`, `channel-seed`, and `channel-weight-influence` for production execution.
- Editor numeric parameters are continuous within their declared bounds; GTK increments remain input hints rather than hard validation grids. This preserves fractional X/Y spacing defaults and nonzero jitter values such as `0.123`.
- Main Channel Settings now has per-channel Point sampler, Random seed, and Weight influence controls. Controls are disabled for channels outside the active output model, non-Shapes treatments, and legacy Crosshatch; values are synchronized from `Document.pattern_state`.
- Uniform and source-weighted editor sampling reuse `generate_site_distribution_cancellable`. Shapes no longer rejects random editor counts above 8,192; request-local limits are sized to the requested lattice and remain bounded by the existing one-million Shapes resource limits.
- Repeated modal entry proactively detaches the Blueprint-owned draft box before `AlertDialog::set_extra_child`, in addition to unparenting it on every response.

## Commands and results

- `cargo test --locked --no-fail-fast` — passed: 251 library tests, 53 binary/UI tests, 0 doc tests.
- `cargo clippy --locked --all-targets -- -D warnings` — passed.
- `cargo check --locked --release` — passed.
- `cargo fmt --all -- --check` — passed.
- `git diff --check` — passed.
- `cargo run --locked -- --demo --show-controls --screenshot /tmp/toniator-ton010-channel-boundary-20260801.png` — real GTK launch and screenshot completed; the Shapes treatment and `Edit Pattern…` entry point rendered.
- `cargo run --locked -- --demo --show-controls --expand-document --window-size 1200x1400 --screenshot /tmp/toniator-ton010-channel-controls-tall-20260801.png` — real GTK resource load completed; expanded Channel Settings hierarchy rendered.
- Focused regression tests passed for `shape_channel_distribution_is_undoable_and_scoped`, `editor_random_sampling_reuses_distribution_without_the_legacy_site_cap`, the three Pattern Editor recipe persistence tests, and `pattern_editor_accepts_common_nonzero_jitter_values`, including synchronization of a changed channel value into an installed embedded recipe.

## Unresolved uncertainty / manual gate

Automated GTK launch verifies resource loading and visible entry points, not human GNOME/Wayland interaction. A manual check is still required: open Pattern Editor, change structural fields, Apply, re-enter it; choose a channel scope and change sampler/seed/influence; select Uniform and Weighted Random; verify the canvas changes, values persist after Save As/reopen, and no Adwaita critical is printed.

## Invalidation

Invalidate this entry if any listed UI/model/native files change, if HEAD changes without reconciling the dirty files, or if the Pattern Editor/channel-control resource hierarchy is reorganized.
