# TON-010 Stage 4, Substage 4B — authoritative selector synchronization

- Timestamp: 2026-07-28T16:06:42-04:00
- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `f9c138c493a9d687b5300abddf14e78281f2ad63`
- Producing agent: `desktop-implementer`
- Scope: bounded Stage 4B only; migrate selector state, labels, active panel, and selector writes to the Stage 4A authority/schema surface.

## Checkout assumptions

The checkout was intentionally dirty before this substage, including TON-010 Stages 1–3, TON-013 GTK/Blueprint work, Stage 4A API additions, presets, fixtures, evidence, and documentation. `src/ui.rs` was already modified against HEAD. This substage patched only the selector-specific locations in that file, preserved all other dirty files, and made no reset, clean, revert, commit, push, deletion, or agent delegation.

## Files inspected

- `src/ui.rs`: selector callbacks, `sync_controls`, `sync_controls_when_idle`, `sync_dropdown_strings`, and realized GTK tests.
- `src/model.rs`: `DocumentEditor` selector writes, Crosshatch exit, and saved Shapes/Curves restoration.
- `src/pattern.rs`: Shapes/Curves registry metadata and `PatternInspectorPanel`.
- `.codex-work/agents/desktop-implementer/ton-010-stage-4-substage-4a-authority-schema-api.md` and Stage 2 selector handoff evidence.
- Toniator GTK stabilization guidance from the local memory skill.

## Exact files changed

- `src/ui.rs`
- `.codex-work/agents/desktop-implementer/ton-010-stage-4-substage-4b-selector-authority.md`

## Implementation decisions and reused abstractions

- Added `AppUi::sync_pattern_selector`, which reads only `PatternDocumentState::selected_pattern_id` and `selected_metadata` obtained before synchronization. It applies the registered Shapes/Curves labels, help text, accessibility metadata, active selector button, Legacy visibility, and `treatment_modes` panel from authority/registry metadata.
- Removed `RenderVariant` as the selector/panel decision source in `sync_controls`. The remaining `RenderVariant` match in that method is intentionally retained for parameter value reads only, pending Substage 4C.
- Narrowed `activate_shape_treatment` and `activate_curve_treatment` with an authority selection check. Crosshatch exit and saved-treatment restoration remain transition-only `DocumentEditor` operations; ordinary selection uses `DocumentEditor::select_pattern` and no adapter state selects a pattern.
- Reused existing `sync_controls_when_idle` and `sync_dropdown_strings` unchanged, including `StringList::splice` model identity and invalid-position safeguards.
- Added a realized selector regression sequence to the existing single-threaded GTK test rather than a second GTK initializer: GTK cannot be initialized from different Rust test threads. The sequence covers contradictory Shapes-authority/Curves-adapter and Curves-authority/Shapes-adapter state, both selector callbacks, active panels, metadata labels, and deferred DropDown model identity.

## Verified findings

- `Document.pattern_state` determines the Shapes/Curves selector and active inspector panel even when the transient adapter has the opposite variant.
- Crosshatch and saved-treatment selector transitions continue through `DocumentEditor`; no selector callback infers a pattern from `RenderVariant`.
- Existing DropDown synchronization retains its `StringList` model and defers work through the idle queue.

## Reasonable inferences

- The later parameter migration can use the Stage 4A state accessors without changing this selector seam, provided it preserves the existing invalid-position and deferred-sync guards.

## Verification

- `cargo fmt --all` and `cargo fmt --all -- --check` — passed.
- `cargo test --locked` — passed: 138 library tests and 46 binary/UI tests.
- `cargo check --locked --all-targets` — passed.
- `cargo clippy --locked --all-targets -- -D warnings` — passed.
- `git diff --check` — passed.

The realized GTK regression presents an `AppUi` fixture and uses the compiled GResource. It is automated GTK/resource evidence, not a manual desktop click-through or screenshot review.

## Artifacts produced

- No screenshot or export artifact. The realized GTK test is the produced runtime/resource verification evidence.

## Known limitations and follow-up targets

- Substage 4C owns migration of parameter control reads and parameter callback bodies away from `RenderVariant`; they remain intentionally unchanged here.
- No new pattern, Weighted Voronoi, persistence change, obsolete-format migration, or custom-pattern ecosystem work was added.
- Before a user-facing milestone acceptance, perform the approved manual Wayland selector click-through if required; no standalone desktop screenshot was captured in this substage.
- Durable documentation may need Stage 4 reconciliation only after the milestone closes; this evidence is not durable documentation.

## Invalidation conditions

Reinspect this evidence if `src/ui.rs`, `src/model.rs`, `src/pattern.rs`, Stage 4A authority accessors, registry selector metadata, GTK deferred synchronization helpers, or the current dirty TON-010/TON-013 baseline changes; if parameter migration starts; or if Git HEAD/worktree assumptions no longer match.
