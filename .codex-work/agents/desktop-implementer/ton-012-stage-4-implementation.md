# TON-012 Stage 4 implementation evidence

- Repository absolute path: `/home/ricperry1/projects/Toniator`
- Git HEAD: `236cdb190a091029c1e7436d65716bf889b31010`
- Relevant working-tree assumptions: at start, `.codex-work/cache-index.md` and
  `.codex-work/project-rehydration.md` were already modified; `AGENTS.md`,
  `.codex-work/backups/`, and the Stage 4 ownership evidence were already
  untracked. They were preserved. No other writer was active. This work leaves
  those inputs untouched and adds the implementation files below.
- Producing agent: `desktop-implementer`
- Task: TON-012 Stage 4 scoped current `.tntr` serialization/application,
  bundled preset conversion, focused GTK save-scope UI, and bounded docs/tracker
  reconciliation.
- Timestamp: 2026-07-26

## Exact files changed

- `src/preset.rs`
- `src/model.rs`
- `src/ui.rs`
- `assets/presets/Chunky Fingerprints.tntr`
- `assets/presets/ComicBook.tntr`
- `assets/presets/Skinny Curve.tntr`
- `assets/presets/Tiled Stacked Motif Stress Test.tntr`
- `README.md`
- `docs/ARTWORK_PIPELINE.md`
- `docs/ARTWORK_PIPELINE_AUDIT.md`
- `ISSUES.md` (TON-012 Stage 4 section only)
- this evidence entry

## Verified implementation decisions

- The only accepted current preset is `format: "toniator-preset"`, `version: 4`.
  Pre-v4 input rejects as unsupported pre-release input; no migration was added.
- `PresetScope` is explicit: `pipeline`, `treatment`, `channel`, or
  `complete-workflow`. The parser accepts exactly the declared sections and
  rejects missing/extra/contradictory scope data.
- Pipeline state uses existing stable dotted IDs through `ArtworkPipelineSettings`.
  Channel records use semantic `channel.cmyk.*` / `channel.rgb.*` keys in a
  deterministic `BTreeMap`; no GTK index or legacy map label is serialized.
- Treatment sections carry kind, common settings, geometry, and shared
  motif/path state. Per-channel maps and the retained renderer
  `value_mode`/`single_channel` compatibility projection are excluded.
- Channel scope carries one channel record and applies only that record. It
  preserves pipeline and unrelated treatment state, and rejects incompatible
  output-model or treatment-kind input rather than silently switching treatment.
- Complete Workflow carries pipeline, treatment, and every active-model channel
  record. It intentionally omits artwork, document identity, appearance/export
  presentation, and transient UI/undo state.
- `ParsedTreatment::candidate_for` builds and validates a complete document copy.
  `DocumentEditor::replace_with_preset_candidate` performs the sole live state
  replacement in one ordinary undo entry. Failed parse/validation leaves live
  document and history unchanged.
- Pipeline output changes use the existing `switch_output_mode` cache transition.
  Complete Crosshatch stages its Curves treatment before final projection so a
  temporary invalid Shapes/Crosshatch snapshot is never retained. The existing
  output transition behavior is preserved; Black is not slot-clamped to RGB.
- UI save uses a compact popover scope choice before the existing native file
  dialog. Import retains the existing background parse/generation gate and
  applies the validated candidate, then read-only control synchronization and
  one preview request. The bundled menu now contains all four runtime files.

## Existing abstractions reused

- `Document.artwork_pipeline`, `ArtworkPipelineSettings`, `OutputChannelId`,
  `OutputModel`, `ChannelAssignment`, and their stable-ID serde boundary.
- `Document::switch_output_mode`, paired inactive CMYK/RGB caches,
  `sync_legacy_projection`, `normalize_crosshatch_render`, `DocumentEditor`
  undo/redo state, `RenderGate`, `LatestSlot`, `sync_controls`, and
  `persistence::atomic_write`.

## Tests and runtime checks

- `cargo fmt && cargo fmt --check` — passed.
- `cargo clippy --locked --all-targets -- -D warnings` — passed.
- `cargo test --locked` — passed: 110 library tests, 43 binary tests, 0 doc tests.
  Focused new coverage includes all scopes, omitted-section preservation,
  deterministic/no-mutation serialization, one undo/redo, malformed/old/unknown
  rejection, semantic CMYK/RGB channel portability, kind/output incompatibility,
  Crosshatch Curves representation, and all four runtime bundled presets.
- `cargo build --locked --release` — passed.
- `git diff --check` — passed.
- `desktop-file-validate packaging/appimage/com.toniator.Toniator.desktop` — passed.
- `appstreamcli validate --no-net packaging/appimage/com.toniator.Toniator.metainfo.xml`
  — successful (pedantic: 2 advisory findings reported by AppStream).
- `cargo run --locked -- --preset assets/presets/ComicBook.tntr --screenshot
  /tmp/toniator-ton-012-stage4-comic.png` — passed; screenshot inspected.
- `cargo run --locked -- --preset 'assets/presets/Tiled Stacked Motif Stress
  Test.tntr' --screenshot /tmp/toniator-ton-012-stage4-stress.png` — passed;
  screenshot inspected.
- `cargo run --locked -- --preset 'assets/presets/Chunky Fingerprints.tntr'
  --screenshot /tmp/toniator-ton-012-stage4-chunky.png` — passed; screenshot
  inspected.
- `cargo run --locked -- --preset 'assets/presets/Skinny Curve.tntr'
  --screenshot /tmp/toniator-ton-012-stage4-skinny.png` — passed; screenshot
  inspected.
- `coredumpctl list toniator --since '2026-07-26 00:00:00' --no-pager` — no
  coredumps found.

## Artifacts and visual result

- `/tmp/toniator-ton-012-stage4-comic.png` (1280x820): normal Shapes CMYK
  halftone rendering after loading the converted v4 ComicBook preset.
- `/tmp/toniator-ton-012-stage4-stress.png` (1280x820): Curves repeated motif
  rendering after loading the converted v4 stress preset.

## Known limitations and follow-up review targets

- The user subsequently completed the manual smoke gate with Toniator exit
  status `0`; the Stage 4 checkpoint is therefore complete. The save-scope
  popover was not separately captured at narrow width.
- Stage 4 deliberately retains `value_mode`, `single_channel`, output-mode
  projection, and transitional Crosshatch adapters. Final RGB Curves and broad
  preview/PNG/SVG parity remain outside this scope.
- Milestone review should inspect the save-scope wording at narrow inspector
  widths and exercise imported user Channel presets against both CMYK and RGB
  documents.
- Durable documentation likely affected at milestone review: user-facing
  release notes and any future file-format reference beyond the bounded updates
  already made to `README.md` and pipeline docs.

## Invalidation conditions

- Changes to `src/preset.rs`, `src/model.rs` candidate/cache transitions,
  `src/ui.rs` preset workflow, stable pipeline IDs, renderer projection,
  bundled preset inventory, or Git HEAD/working-tree assumptions require this
  evidence to be revalidated.

## Review correction (2026-07-26)

- Parent-owned `Document::apply_preset_pipeline_unchecked` omitted-active
  restoration was preserved and tightened: it now validates an omitted
  ActiveChannel assignment with a representative destination, retains a
  same-model active semantic channel, maps a valid legacy slot across output
  models, and uses the target default only when no slot exists. CMYK Black is
  therefore never clamped into RGB.
- `src/preset.rs` now performs current-v4-only recursive schema validation
  before shared serde DTO conversion. It rejects unknown nested Settings,
  Shapes/Curves geometry, shared/custom path data, channel payload fields, and
  pipeline fields without changing v6 project deserialization policy.
- Treatment-only regression coverage now proves same-kind Shapes and Curves
  imports retain receiving channel sentinel values. Cross-kind Treatment is
  explicitly deterministic: it installs the target kind with current default
  channel maps; it does not claim channel preservation.
- Active-channel coverage now includes omitted ActiveChannel state for
  CMYK-to-RGB, RGB-to-CMYK, and same-model imports. Crosshatch is accepted only
  as Complete Workflow with Curves and CMYK compatibility records; Current
  Channel and Pipeline-only Crosshatch scopes reject.
- Documentation update: `docs/ARTWORK_PIPELINE.md` records cross-kind channel
  reset behavior and omitted-active transition rules.
- Verification after correction: `cargo fmt --check`; focused `cargo test
  --locked preset::tests --lib` (11 tests); full `cargo test --locked` (110 library,
  43 binary, 0 doc tests); `cargo clippy --locked --all-targets -- -D warnings`;
  and `git diff --check` all passed.
