# TON-013 Stage 2 channel inspector implementation

- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `546ea4c5eb1fec8e91c2b307545e33e42331e308`
- Timestamp: `2026-07-26`
- Producing agent: `desktop-implementer`
- Task: implement the bounded TON-013 Stage 2 Builder-owned Source/Output/
  Channel Settings hierarchy and separate real-channel/aggregate composites.

## Git and working-tree assumptions

Before this work, Stage 1 had already modified `src/ui.rs`, `ISSUES.md`,
`.codex-work/cache-index.md`, and `.codex-work/project-rehydration.md`, and
had created untracked `resources/`, `docs/`, previous evidence, and
`.codex-work/backups/`. Those changes were retained. This implementation
intentionally changed the pre-existing Stage 1 files `src/ui.rs`, `ISSUES.md`,
`docs/UI_ARCHITECTURE.md`, and `resources/ui/Toniator.cmb`, and added the three
Stage 2 UI resources below. It did not modify unrelated cache-index,
rehydration, backup, or historical evidence files.

## Exact implementation files

- `src/ui.rs`: loads the inspector and two composite resources with
  `gtk::Builder::from_string`, caches seven real semantic channel panels, adds
  the aggregate panel, synchronizes scope/panel state safely, and adds focused
  Builder/realized GTK coverage.
- `resources/ui/ToniatorInspector.ui`: static Source, Output, Channel Settings,
  Appearance, and Treatment expander structure plus stable content hosts.
- `resources/ui/ToniatorChannelControls.ui`: reusable real-channel composite.
- `resources/ui/ToniatorAggregateChannelControls.ui`: separate aggregate
  composite.
- `resources/ui/Toniator.cmb`: records SHA-256 values for all four UI files;
  the pre-existing `Toniator.ui` hash remains
  `5e91e8e92c1417bf7e59403bcbe3b5b2735e24f1c211a16230698da4265d6fba`.
- `docs/UI_ARCHITECTURE.md`: Stage 2 resource paths, IDs, ownership, and future
  editing rules.
- `ISSUES.md`: TON-013 Stage 2 boundary and remaining scope.
- `.codex-work/agents/desktop-implementer/ton-013-stage-2-channel-inspector-implementation.md`:
  this evidence entry.

## Verified decisions and reused abstractions

- Source, Output, and Channel Settings are the first inspector expanders in the
  actual widget tree. Source and Output default expanded; Channel Settings,
  Appearance, and Treatment default collapsed. Artifact expansion opens the
  first three groups.
- `OutputChannelId::{CMYK,RGB}`, `stable_id`, `belongs_to`,
  `sync_dropdown_strings`, `sync_controls_when_idle`, and the existing
  `DocumentEditor` pipeline edit APIs were reused. The scope callback converts
  its temporary presentation position to a current semantic ID only at the
  callback boundary; cached widget identity is the typed channel ID.
- `ToniatorChannelControls.ui` creates only real C/M/Y/K and R/G/B instances.
  `ToniatorAggregateChannelControls.ui` separately owns All Inks/All Channels
  status, mixed-value/apply-to-all messaging, and Crosshatch All Layers
  terminology. Runtime hosts contain explicit status/context copy rather than
  misleading empty treatment editors.
- The top-level control is named `Scalar Channel Routing` and remains the
  pipeline `ChannelAssignment`/active-channel control. Existing `Adjust Ink`/
  `Adjust Channel` controls remain the sole treatment-recipient controls; a
  realized regression proves selecting one does not silently select the other.
- Existing `StringList` identity, invalid-position rejection, deferred sync,
  bounded `RefCell` borrows, refresh-no-dirty behavior, output transitions,
  presets, undo/redo, and export paths remain in their established Rust owners.

## Verification

- `cargo fmt --check` passed.
- `cargo test --locked` passed: 117 library tests and 45 binary/UI tests.
  New focused checks cover Builder IDs, hierarchy ordering, aggregate/template
  separation, all seven cached semantic instances, repeated CMYK/RGB
  transitions, scope model identity, and Crosshatch All Layers.
- `cargo clippy --locked --all-targets --all-features -- -D warnings` passed.
- `cargo build --release --locked` passed.
- `xmllint --noout` passed for `Toniator.ui`, `ToniatorInspector.ui`,
  `ToniatorChannelControls.ui`, `ToniatorAggregateChannelControls.ui`, and
  `Toniator.cmb`.
- `git diff --check` passed.
- `coredumpctl list toniator --no-pager` showed only historical crashes through
  2026-07-21; no new coredump followed this work.

## Runtime artifacts

- GTK artifact-mode launches were attempted at 1000x760 and 780x680 with the
  Stage 2 expanders exposed. Both launched but the current Wayland compositor
  did not provide a GTK render node within the application capture timeout, so
  `stage2-normal.png` and `stage2-narrow.png` were not created and no screenshot
  inspection is claimed. Existing `test-artifacts/ton-013/shell.png` is a Stage
  1 artifact and was not reused as Stage 2 visual evidence.
- Cambalache Flatpak availability was confirmed, but no round-trip was run to
  avoid driving or obscuring the user desktop. The `.cmb` project references
  relative resource paths and verified current hashes.

## Known limitations and follow-up review targets

- TON-013 remains In Progress. Treatment-specific rows, custom drawing,
  dialogs, and much of `build_editor_view` remain Rust-built by design.
- Real-channel panels currently provide semantic heading/inclusion status and a
  stable content host; per-channel treatment rows should move into those hosts
  only with a separately scoped lifecycle and regression review.
- Correction pass: actual inspector order, progressive disclosure defaults,
  scalar-routing versus treatment-recipient semantics, and status-only panel
  copy were corrected after independent UX review. The Builder channel hosts
  remain ready for a later safe treatment-control migration.
- Re-attempt normal and narrow screenshot capture when the compositor supplies
  a render node; inspect responsive height, expander scanning order, and panel
  visibility then. A non-interactive Cambalache round-trip remains a follow-up.

## Documentation affected

`docs/UI_ARCHITECTURE.md` and the TON-013 section of `ISSUES.md` were updated.
No milestone-complete claim was made.

## Correction pass after independent UX review

- Actual widget order was corrected: `hierarchy.root` is now the first child of
  the real inspector box, and it contains Source, Output, Channel Settings,
  Appearance, and Treatment in that order. Source/Output remain expanded;
  Channel Settings, Appearance, and Treatment collapse by default. The artifact
  expansion path opens Source, Output, and Channel Settings.
- `Scalar Channel Routing` now names the top-level pipeline routing control.
  It preserves `ChannelAssignment` and active-channel behavior. Existing
  `Adjust Ink`/`Adjust Channel` controls remain the sole treatment-recipient
  controls. The realized test sets each independently and confirms no silent
  cross-selection.
- Real and aggregate hosts are explicitly framed as runtime status/context
  panels, with copy identifying the treatment controls below; aggregate remains
  one separate object and the seven real semantic instances remain cached.
- Focused realized GTK coverage passed inside the full suite, including actual
  hierarchy order, collapsed defaults, scalar-routing/treatment-target
  separation, aggregate identity, and repeated CMYK/RGB model transitions.
- Post-correction artifact commands were attempted exactly as follows:
  `cargo run --locked -- --demo --show-controls --window-size 1000x760
  --expand-document --screenshot test-artifacts/ton-013/stage2-correction-normal.png`
  and the equivalent 780x680/280 inspector-width narrow command. Both launched
  and both reported `Could not write window screenshot: GTK did not produce a
  render node within 5 seconds`; no screenshot artifact or visual claim exists.
- Cambalache round-trip remains unavailable because it would drive/obscure the
  user's desktop; XML validation and relative-path/hash checks passed.

## Invalidation conditions

Revalidate after changes to `src/ui.rs`, any `resources/ui/Toniator*.ui`,
`resources/ui/Toniator.cmb`, semantic pipeline/output transition code, GTK or
libadwaita versions, focused GTK tests, Git HEAD, or the working-tree
assumptions above.
