# Gate 21B-5 — Document Presets

Date: 2026-09-05. Status: **Accepted awaiting checkpoint**. All five Stage 21B
gates are accepted. The user authorized this gate and explicitly required
`Load preset...` to accept `.toniator-preset` and valid `.toniator` files.
The user accepted the gate and follow-up fixes and authorized local acceptance
actions on 2026-09-05. No push or release is authorized.
Implementation started on `main` at `bc3131f256ab663adf38e54243ed7702a66a8238`.
Existing version, packaging, tooling, documentation and user-artwork changes
remain outside this gate. Protected specifications and legacy files are untouched;
the previously identified protected-spec terminology correction remains pending.

## Product behavior

The New dropdown retains New/Open and adds **Load preset...**, then
**Save preset**. A Preset captures the whole reusable design: model, channel
topology/order, base Pattern and channel overrides/deltas, paint, opacity,
visibility, response settings, reusable source interpretation, and document-owned
definition/authored-resource stores. It preserves inheritance and sharing intent.
Personal structural Patterns and project Open/Save remain separate operations.

Load requires source-backed artwork. Both accepted file kinds replace the complete
configuration in the existing workspace while retaining its document identity,
source bytes/reference, canvas, project location and savepoint. Confirmation names
the file and model and explains whole-document replacement and Undo. A changed
load creates one history entry; equal configuration changes nothing, including
revision, dirty state and Redo. Undo/Redo need no access to the input file.
Cross-model load refreshes model/channel controls, selects All after a change,
and preserves viewport and Preview/Source selection. All identifies the displayed
default as **Base pattern**, including when channel Patterns differ.

Save captures committed configuration before the chooser, including source-less
New. It writes `.toniator-preset`, appending that suffix if absent without replacing
another extension. It does not set a project savepoint, change project identity,
add Recent Files, or associate the workspace with a Preset. The initial folder is
`$XDG_DATA_HOME/Toniator/document-presets` (standard home fallback); subsequent
operations reuse the session's last folder. This directory is separate from the
personal Pattern library. No Preset gallery, manager or structural-format migration
is included.

## Shared authority and format

- `toniator-domain::DocumentConfiguration` captures and binds the reusable fields;
  domain validation and `DocumentHistory::apply_document_configuration` own the
  atomic transition. Exact captured document/revision checks reject stale results.
- `toniator-io` shares `DocumentConfigurationDtoV7` with the ordinary project DTO.
  Current project container-v1/document-v7 wire structure remains valid; obsolete
  formats are rejected. Nested DTO unknown fields now reject in both readers;
  `serde_ignored` closes the flattened configuration/envelope boundary.
- A source-free ZIP contains exactly `preset.json` with kind `document_preset`,
  `document_preset_format_version: 1`, `document_schema_version: 7`, and
  `configuration`. The schema marker versions the shared authored configuration.
  Kind/version dispatch occurs before configuration decoding. Configuration has
  only `pattern_definition_bundles`, `pattern_settings`, `channel_configuration`, and
  `authored_structures`.
- No source payload, source ID/path/name/manifest/digest, canvas dimensions,
  project ID, thumbnail, history, runtime cache or UI state is serialized.
  Source mapping component/inversion/gain/bias and `StretchToCanvas` remain
  reusable configuration, including artwork-weighted site mappings.
- Archive shape selects the reader; filename extensions are chooser guidance.
  Project-as-Preset classification and the ordinary integrity reader share one
  no-follow file handle. Intact projects still require their matching source
  entry and integrity checks. Removing only that entry is invalid.
- ZIP reading is bounded by existing 256 MiB archive and 4 MiB JSON limits and
  rejects unsupported compression, duplicate/extra/unsafe entries and malformed
  configuration. Current serialization is deterministic.

Save prepares a private same-directory temporary file, syncs it, and publishes
without partially replacing the destination. New files use create-only hard-link
publication; existing files require a path-bound byte fingerprint and explicit
replacement confirmation before checked rename. A detected external change
preserves the current destination. This is a single-writer guarded workflow,
not a cross-process compare-and-swap promise. Directory-sync failure after
publication reports successful save with a durability warning. Native file
choosers may show their own overwrite question before Toniator confirms the
normalized, fingerprinted destination; this extra prompt is a current UX cost.

Net-change invalidation preserves Source precedence over geometry changes,
including nested artwork-weighted mappings. Regions algorithm/sampling changes
are Family; site filters and response changes are Realization; output painter
reordering is Presentation. Tests cover combined changes as well as individual
cases. Existing private-draft squash uses the same corrected classifier.

## Operation and accessibility boundaries

Immutable worker requests carry operation serial, phase, workspace generation,
and the captured load document/revision. Completion is published only by the GTK
main context. Load/confirmation and modal save decisions block conflicting edits.
Save publication permits ordinary edits to continue against the document while
writing the captured snapshot. Lifecycle/export/private-editor entry stays gated
until file completion. Cancelling a read invalidates its eventual callback.

| Control or changed projection | Native authority and verification |
| --- | --- |
| New Preset menu items | GTK menu-item click actions, exact requested names/order, native action sensitivity. Names derive from visible GTK labels; an empty generated label relation is cleared. Source-less Load disabled, Save enabled; New tooltip explains artwork prerequisite. |
| Load/Save chooser | Native dialogs, Cancel/accept buttons, combined Preset/project Load filter, Preset-only Save filter and Name text relation. AT-SPI text/action/readback; retained private keyboard uses Ctrl+L for location. Chooser cancellation verified. |
| Whole-document Load confirmation | Native dialog named from its actual heading and transient parent; Cancel default/focus, Tab/Enter acceptance and Escape cancellation verified. Filename/model/replacement/source/canvas/Undo disclosure inspected. |
| Guarded replacement confirmation | Named native dialog, Cancel default, Replace preset button. Cancel, successful replacement and changed-destination rejection verified against file bytes. |
| Read/preparation progress | Named transient modal frame with a focused, enabled Cancel button and native click action. Its shared progress surface was inspected in native pixels and AT-SPI. In-flight Cancel and Escape close it, restore New focus and ignore the late read completion; phase/token cancellation also has app-state tests. |
| Model/channel/history and Base pattern | Existing native model selector and channel toggles project domain state. Cross-model refresh and keyboard Undo restore the saved state; no-op preserves Redo. Updated All/base label inspected. |
| Failure/status messages | Concise recovery appears in the wrapped sidebar and existing banner; stale success status is replaced. Full low-level diagnostics go to stderr. Invalid-source project leaves revision/dirty/savepoint unchanged. |

Native GTK stock chooser details remain toolkit-owned, including its unnamed
location entry and native overwrite-dialog root. New Toniator confirmation roots
and menu items have stable names. No alternate accessibility schema is introduced.

## Verification and evidence

Focused checks passed:

- Domain Preset integration tests: 5; I/O Preset integration tests: 5; private
  temporary-file permissions unit test: 1; app Preset state tests: 2; engine
  Preset/render test: 1.
- Existing affected `stage20f_drafts`: 10; Stage 20G authority-only witness: 1;
  current document-v7 deterministic persistence witness: 1.
- Current app/CLI build, strict domain/I/O/engine library and Preset-test Clippy,
  app binary/test Clippy, scoped formatting, architecture and diff checks.
- Independent headless regression review found and prompted precise invalidation,
  metadata dispatch and same-handle reader corrections. Independent UX review
  prompted readable failure recovery, the Base pattern label and dialog naming.

One obsolete historical schema-v5 test was accidentally invoked during the first
headless pass and failed its hard-coded `5 == 7` assertion. It was neither changed
nor rerun and is not included among current gate checks.

Native review artifacts are under `target/validation/stage21b-gate5/`:
`heterogeneous-cmyk.toniator-preset`, intact donor/project variants,
`raster-cli.png` (1024×1024), `vector-cli.svg` (900×620), companion PNG/SVG files,
and direct-engine outputs. The four channels use Even Random Circles, One Guide
Lines, Curve Motif and Grid Voronoi Scale. Both immutable source hashes are
unchanged. Engine tests bind both input kinds onto both source aspects, compare
configuration, retain source/canvas, Undo/Redo and continue editing.

The exact CLI PNGs were viewed with their native CMYK white background. The exact
CLI SVGs match the engine SVGs byte-for-byte and were inspected through Inkscape
at intrinsic size. SVG inspection PNGs are labeled derivatives; transparent
direct-engine outputs were retained unchanged, and their alpha was inspected
separately. No flattened composite replaces native evidence.

Private GTK evidence directories:

- `.codex-work/evidence/ui-run-20260905-125321-41040`: menu/chooser, cancellation,
  both load kinds, Undo/Redo, Save/create/replace/cancel, external-change refusal,
  stripped-project rejection and source-less Save applicability.
- `.codex-work/evidence/ui-run-20260905-131016-55474`: corrected readable failure
  sidebar, source-less Preset loaded onto vector artwork, clean-savepoint Undo,
  project-as-Preset no-op preserving Redo and Save preserving clean status.
  Its duplicated-title intermediate confirmation captures are superseded below.
- `.codex-work/evidence/ui-run-20260905-131653-62730`: final named confirmations
  without duplicate headings, final raster CMYK preview, native overwrite prompt,
  guarded cancellation, controls/logs/evidence bundle.
- `.codex-work/evidence/ui-run-20260905-133118-68551`: completion-audit progress
  inspection and in-flight Cancel/Escape, restored New focus, disabled Undo,
  unchanged clean project and absence of a late confirmation. The exact progress
  screenshot is `target/validation/stage21b-gate5/progress-native.png`; earlier
  `progress-loading*.png` captures in this run missed the transient surface and
  are not its visual evidence. A target-only fixture pads a copied PNG to the
  existing 128 MiB source limit and updates its length/digest; ordinary reading
  validates it. Cancellation checks temporarily limit only the private app to
  10% CPU so native button activation completes during the read. CPU quota is
  restored before checking the late completion. This is scheduling evidence,
  not a performance measurement; no production code or immutable input changes.
  `progress-events.jsonl` retains revision 0, clean status and the savepoint;
  the final screenshot, AT-SPI readbacks and empty diagnostic logs agree.

`gtk-events.jsonl` records revision/workspace/savepoint/dirty readbacks. The final
app log contains no panic or GTK warning; the invalid-project run contains its
expected diagnostic. Harness state and isolated test XDG roots remain under
`.codex-work/gtk-wayland-gate21b5` and the stage validation directory. The retained
keyboard client and private Sway session were stopped at handoff.
Automated Sway/wlroots evidence does not constitute human or GNOME/Mutter/portal
acceptance. Unrelated deferred issues remain in `ISSUES.md`.

## Accepted issue follow-up

The same acceptance includes TON-004 through TON-007: files from a second launch
enter the existing Save/Discard/Cancel loader; native/portal chooser ownership
and pending replacement loads gate competing requests and edits; Preset actions
refresh after private editor/library dismissal; and terminal wizard destruction
avoids repeated Cancel/X clicks while retaining the dirty-draft decision.

Overall preview shares are preparation/decode 10%, family 15%, output 20%, scene
5%, raster 49%, and final publication 1%. Main, Wizard and Advanced labels expose
tenths of a percent. Cancellation checks no longer fabricate completed work;
real producer callbacks and completed-output milestones supply progress.
An exact-coordinate index replaces TON-003's quadratic circular-mark membership
lookup, preserving first-match site identity, positive-radius filtering and
deduplication. The observed SVG size-0.2 preview fell from roughly 45.4 to 2.9
seconds between first progress and completion; these are individual debug UI
observations, not controlled benchmarks. Broader TON-003 performance remains open.

Focused lifecycle, progress, membership and dependent-output ordering tests,
strict affected-target Clippy, architecture and formatting checks pass. Private
GTK runs ending 135310-78698, 140256-86459, 141335-92087 and 142902-100954 verify
forwarding decisions, the actual TestPatternDoc Apply/Save preset sequence,
clean/dirty Cancel and X, Advanced dismissal and dense PNG/SVG progress.
Evidence details are in `target/validation/ton004/README.md` and `ISSUES.md`.
The final run records one GtkText focus-out warning during document replacement,
without a panic, crash or failed action. An overbroad engine test-target Clippy
attempt encounters historical references to user-deleted Reddit fixtures; those
are excluded from current scoped checks and were not restored.

Acceptance reuses completed verification and directly inspected native artifacts.
Private sessions and retained keyboard clients are stopped. User acceptance does
not turn automated Sway observations into GNOME/Mutter/portal test evidence.
