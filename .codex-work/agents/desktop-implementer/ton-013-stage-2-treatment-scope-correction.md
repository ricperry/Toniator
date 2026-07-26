# TON-013 Stage 2 treatment-scope semantic correction

- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `546ea4c5eb1fec8e91c2b307545e33e42331e308`
- Timestamp: `2026-07-26`
- Producing agent: `desktop-implementer`
- Task: correct the final Stage 2 duplicate authoring locus without changing the
  established Builder hierarchy, cached real-channel composites, or pipeline
  controls.

## Git and working-tree assumptions

The checkout was already dirty with the complete TON-013 Stage 1/Stage 2 work:
`src/ui.rs`, `ISSUES.md`, `.codex-work/cache-index.md`, and
`.codex-work/project-rehydration.md` were modified; `resources/`, `docs/`,
historical evidence, and backups were untracked. This correction preserved
those changes and adds only the Stage 2 semantic correction described here.
No commit, push, deletion, or Cambalache round-trip was performed.

## Exact changed files

- `src/ui.rs`: repurposes the top selector as Treatment Editing Scope; converts
  its position through a real `OutputChannelId` only at the callback boundary;
  synchronizes both hidden Shapes and Curves targets without editing the
  document pipeline; retains stable `GtkStringList` models; hides duplicate
  target rows; revises scope/status copy; and extends realized GTK coverage.
- `resources/ui/ToniatorInspector.ui`: labels and describes Treatment Editing
  Scope truthfully.
- `resources/ui/Toniator.cmb`: updates the Inspector UI SHA-256 while retaining
  entries and hashes for all four UI files.
- `docs/UI_ARCHITECTURE.md`: documents the pipeline/treatment ownership split,
  hidden compatibility target widgets, Full Color, Crosshatch, and intentionally
  collapsed Treatment section.
- `ISSUES.md`: records the settled Stage 2 correction and remaining TON-013
  scope.
- `.codex-work/agents/desktop-implementer/ton-013-stage-2-treatment-scope-correction.md`:
  this implementation evidence.

## Verified implementation decisions and reused abstractions

- Output `Channel Assignment` and conditional `Active Channel` remain the only
  controls that mutate `ChannelAssignment` and `active_channel`.
- The top `Treatment Editing Scope` is the only visible treatment-recipient
  selector. It drives the existing `web_target` and `curve_target` widgets for
  both Shapes and Curves without creating a document edit.
- `channel_scope_channel`, `OutputChannelId`, `sync_dropdown_strings`, existing
  target callbacks, and deferred control synchronization were reused. Aggregate
  scope is represented as `None`, never as an aggregate `OutputChannelId`.
- Full Color leaves the treatment scope enabled. Crosshatch shows one disabled
  `All Layers` scope. The seven cached real channel composites and separate
  aggregate composite remain unchanged in identity and ownership.
- Legacy Adjust Ink/Adjust Channel rows are hidden but retained for callback and
  mixed-value compatibility. Treatment Settings remains collapsed by default as
  setup-first progressive disclosure.

## Verification and artifacts

- `cargo fmt --check` passed.
- `cargo test --locked` passed: 117 library and 45 binary/UI tests. The realized
  GTK regression proves top scope updates both target models/selections, remains
  usable for Full Color, preserves each target `StringList` identity across
  CMYK/RGB transitions, and leaves a configured Output active channel unchanged.
- `cargo clippy --locked --all-targets --all-features -- -D warnings` passed.
- `cargo build --release --locked` passed.
- `xmllint --noout` passed for all four UI files and `Toniator.cmb`.
- `git diff --check` passed.
- Bounded GTK screenshot capture passed and was visually inspected:
  `test-artifacts/ton-013/stage2-treatment-scope-correction.png` (1000x760).
  It shows Source, Output, and Channel Settings in order, the new scope label,
  and no visible duplicate selector while Treatment remains collapsed.
- The earlier combined screenshot/coredump command was blocked by execution
  policy before it ran. The screenshot-only retry succeeded; no new crash claim
  is made from the unavailable coredump query.

## Known limitations and follow-up review targets

- TON-013 remains In Progress. Substantial treatment layout and custom controls
  are still Rust-built; this correction does not move them into Builder hosts.
- Recheck the target-model synchronization if treatment targets become persisted
  document state or if a later stage replaces the hidden compatibility widgets.
- Future UX review should repeat normal and narrow captures after any treatment
  layout migration; the normal screenshot was sufficient for this bounded
  correction.

## Documentation affected

`docs/UI_ARCHITECTURE.md` and the TON-013 Stage 2 section of `ISSUES.md` now
describe the corrected authoring boundary. Durable milestone reconciliation is
still deferred to the documentation maintainer after milestone review.

## Invalidation conditions

Revalidate after changes to `src/ui.rs`, `resources/ui/ToniatorInspector.ui`,
`resources/ui/Toniator.cmb`, treatment target callbacks, output pipeline
controls, the realized GTK test, GTK/libadwaita versions, Git HEAD, or the
working-tree assumptions above.
