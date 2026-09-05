# Gate 21B-5 — Document Presets planning draft

Date: 2026-09-05. Status: **accepted awaiting checkpoint**.
Verified behavior and review disposition are in
[`STAGE_21B_GATE5_IMPLEMENTATION.md`](STAGE_21B_GATE5_IMPLEMENTATION.md).
The user authorized this gate with the explicit addition that Load preset...
accepts both `.toniator-preset` and valid `.toniator` files. The defaults below
are the implementation contract. The user accepted the gate and authorized local
acceptance checkpointing on 2026-09-05.
Protected specification files are not authorized for revision by this runtime
implementation instruction; their bounded terminology correction remains pending.

## Checkout and authority

- Inspected checkout: `main`, HEAD `bc3131f256ab663adf38e54243ed7702a66a8238`;
  local `origin/main` matches, ahead/behind 0/0. No remote refresh was needed
  for planning; this is a local-ref observation.
- Gate 21B-4/startup acceptance: implementation `4ed29d4`, documentation
  `b4afd6d`. Stage 21B remains In progress; Gate 21B-5 was Planned at entry.
- Existing uncommitted 0.3.0 version, packaging, tooling, documentation, asset
  deletions, and untracked user artwork are outside this planning change.
- [Addendum](../Project%20Specification/Addendum.md) remains normative.
  [Stage 21B contract](STAGE_21B_PATTERN_WIZARD_AND_PERSONAL_LIBRARY_PLAN.md),
  [roadmap](GREENFIELD_REWRITE_PLAN.md), [tracker](../ProgressTracker.md), and
  [UI references](ui/REFERENCES.md) bound this proposal.
- Semantic-map remains retired. The initial planning pass was read-only;
  the user subsequently authorized the bounded runtime gate recorded above.
  No legacy quarry or acceptance claim is included.

## Recommended product contract

A document Preset captures the complete reusable design configuration, including
heterogeneous channel Patterns. Loading replaces that configuration in the
open, source-backed project in one undoable transition. It always addresses the
whole document, regardless of the current Edit channel selection.
The Preset's model and complete topology replace the destination model/topology;
there is no channel-by-channel merge. RGB artwork can use a CMYK Preset through
the existing source interpretation pipeline. Do not invent source-format/model
compatibility restrictions; reject only violations of current domain/source
contracts during candidate binding.

Retain the destination source and canvas dimensions. Capture source interpretation
controls and the reusable placement rule: the only current rule is
`StretchToCanvas`. There is no crop, source-position transform, or alternative
placement mode to design in this gate. A different destination aspect changes
the resolved density according to existing domain authority. Store authored
density/aspect intent and geometry coordinates unchanged; do not bake resolved
axis counts or introduce automatic geometry rescaling.

| Capture | Exact boundary |
| --- | --- |
| Channel model/topology | Current RGB, CMYK, or SourceColorAlpha model, channel order/roles, and complete per-channel state. |
| Base Pattern | Definition bundles, ordered outputs and response settings, density/aspect, Pattern rotation, shape rotation. |
| Channel Pattern intent | Replacement references and optional typed deltas, preserving inheritance versus explicitly authored values; translations retain their current units. |
| Channel appearance | Solid/sample-source paint, color values, opacity, visibility. |
| Source interpretation | Component, inversion, gain, bias, and current reusable Stretch to Canvas placement rule. No original source identity. |
| Authored resources | Complete document-owned definition and authored-structure stores, including guides, shapes, motifs, IDs and sharing relationships. No dependency on an installed personal Pattern. |

| Exclude | Load/save behavior |
| --- | --- |
| Source payload/identity | No source bytes, logical reference ID, original filename/path, source manifest, digest, decoded dimensions, proxy, or thumbnail derived from artwork. |
| Project identity and location | No document ID, project path/title, savepoint, Recent Files entry, or persistent association with a loaded Preset. |
| Canvas dimensions | Keep the destination canvas; do not serialize the originating canvas as configuration. |
| Runtime/UI state | No revisions, history, pending jobs, caches, selection, zoom/pan, Preview/Source choice, open editors, warnings, or export-dialog preferences. |
| Deferred features | No animation/media, selective channel merge, Preset gallery/manager, compatibility adapters, or Pattern-format migration. |

Resource stores are copied as one internally coherent graph, retaining shared
references within the configuration. They replace destination configuration
stores, so local numeric IDs need no merge/remapping against discarded stores.
The destination document ID and source binding remain independent. Subsequent
edits are document-local; external Pattern or Preset file changes cannot change
the loaded document. Save captures all current stores rather than introducing
an unrelated pruning policy.

## Load, history, and save behavior

1. Under New, retain existing New/Open entries and append **Load preset...**,
   then **Save preset**, as a distinct menu section. Preserve primary New.
   Labels and native accessible names clearly distinguish these from project
   Open/Save and Pattern Gallery/Save Pattern.
2. Load requires an open source-backed workspace; disable it at startup and in
   source-less New, with explanatory product help. The user first opens artwork
   through the existing workflow. This gate adds no staged source-attachment
   workflow. Save preset requires a valid workspace, including source-less New,
   because configuration capture itself does not require artwork.
3. Load opens a native chooser filtered to document Presets and `.toniator`
   projects, reads off the GTK
   thread, and validates an isolated candidate bound to the captured destination.
   Before publication, show a compact confirmation naming the Preset and its
   color model: it replaces all channel Patterns/settings, keeps current artwork
   and canvas, and can be undone. Actions: Cancel and Load preset; Cancel is the
   default. This is a scope disclosure, not a new editable preview wizard.
4. Dirty projects do not enter Save/Discard/Cancel: the current work is retained
   in history. Successful changed load adds exactly one Undo entry, clears redo,
   preserves the project savepoint/location/source, and therefore uses existing
   dirty-state comparison. Undo restores exact former authored state; redo
   restores the loaded state without rereading the Preset. Equal configuration
   is a no-op that preserves revision, undo/redo, and dirty state.
5. After changed load, select All and refresh model/channel controls and preview.
   Keep viewport and Preview/Source choice. Undo/redo reconcile the current
   selected target using existing valid-target rules; UI selection is not stored
   in document history. Do not install a fresh Workspace or reset history.
6. While loading/confirming, gate conflicting edits and lifecycle actions. Still
   capture workspace generation plus document revision and recheck them before
   publish; a late/stale callback fails without mutating a new workspace.
   Decode/validation/cancellation failures preserve source, canvas, configuration,
   savepoint, history, UI selection, and the last successful preview.
   A rendering failure after a valid commit reports the preview error and retains
   the loaded configuration, Undo, and last successful image; it does not silently
   roll back an accepted command. Stale preview results cannot replace current work.
7. Save preset snapshots the committed main-document configuration at invocation,
   opens a native Save chooser, and writes that snapshot off-thread. It does not
   capture a private wizard draft or mark the project saved. Disable conflicting
   modal entry while private authoring is open; restore invoking focus on close.
   Gate overlapping file operations until completion. Ordinary edits after the
   snapshot do not change the saved payload; completion reports that snapshot's
   success without changing any workspace savepoint or applying document state.
8. Each Save preset invocation chooses a filename; there is no implicit Update
   linked to the last loaded Preset. Suggest `Untitled.toniator-preset` initially
   so an artwork filename is not automatically copied into reusable metadata.
   The user-chosen filename is the display name; no redundant stored name/UUID,
   tags, thumbnails, or catalog are needed.
9. Initial chooser folder: `$XDG_DATA_HOME/Toniator/document-presets` (fallback
   `~/.local/share/Toniator/document-presets`). Create on explicit Save use;
   arbitrary user-selected directories remain supported. If unavailable, keep
   the chooser usable and report the problem. Remember the last folder in the
   current app session only. Existing personal Pattern root configuration and
   `presets/` storage remain unchanged; document Presets do not enter Recent Files.
10. Normalize the extension before probing/confirming the final destination.
    Confirm replacement of an existing file and retain its observed fingerprint;
    if a recheck immediately before publication detects a change, fail with
    retry/choose-another-file recovery.
    A missing destination uses create-only publication. Cancellation and failure
    preserve the existing destination and never mark save success.

| Pending operation | Allowed interaction |
| --- | --- |
| Load chooser/read/confirmation | Cancel, viewport/navigation and Help; block document edits, model/target selectors, Undo/Redo, private editors, project lifecycle, Save/Export and overlapping Preset actions. |
| Save chooser/overwrite confirmation | Cancel and dialog interaction; block conflicting main-window actions while the dialog owns focus. |
| Save worker | Allow ordinary document edits, model/target selection, Undo/Redo and viewport/navigation. Block lifecycle, private editors, Save/Export and overlapping Preset actions until completion. |

Capture document ID, source identity/binding, canvas, revision and workspace
generation for Load; recheck all at the final main-thread commit. Compare the
fully bound candidate against the current document: equality returns unchanged
with no revision/history/redo changes; otherwise publish one revision and one
history entry and clear redo. Save completion is scoped to its operation token
and workspace generation; a stale completion never updates another workspace.

## Source-free serialization contract

Proposed extension: **`.toniator-preset`**. Use the existing ZIP/JSON machinery
with exactly one file entry, `preset.json`. Its explicit envelope is:

```json
{
  "kind": "document_preset",
  "document_preset_format_version": 1,
  "document_schema_version": 7,
  "configuration": { "...": "shared authored configuration fields" }
}
```

The example illustrates the envelope; the configuration fields are the capture
table, using existing document DTO field names/types. The new Preset format v1
defines this single-entry archive layout. It does not redefine project
container-v1 or make a project with its source removed valid.

Extract a shared authored-configuration DTO from the current document DTO:
`pattern_definition_bundles`, `pattern_settings`, `channel_configuration`, and
`authored_structures`. Use it for both formats, flattened at the current location
in ordinary project JSON so document-v7 output remains structurally unchanged.
Reuse existing nested converters for definitions, channel state, and resources.
Project-only `id`, `canvas`, `source_reference_id`, source manifest and archive
source entry remain required by ordinary project readers/writers.
Here `document_schema_version: 7` identifies the authored configuration schema
shared with document-v7, not permission to parse this envelope as a project.
The separate Preset format version owns its envelope, field exclusions and
binding policy; neither version substitutes for the other.

Both paths delegate validation to the same domain reconstruction authority.
Preset decoding validates schema and configuration invariants, then binding to
the destination canvas/source performs full effective-document validation before
publication. If validation needs canvas context, require it at this boundary;
never fabricate a source or synthetic canvas merely to satisfy project loading.

Read content kind/version before treating it as configuration. Wrong-kind files,
unsupported versions, missing/dangling resources, invalid effective values, and
extra source/unknown envelope fields fail with actionable diagnostics. Require
unknown-field rejection recursively through the configuration using shared DTO
shape authority, without a second hand-maintained whitelist. Implementation
testing showed that ignored-field callbacks miss internally tagged enum contents;
shared nested DTOs therefore reject unknown fields in both formats. This tightens
malformed-project rejection while preserving valid document-v7 wire format.
Cover ordinary channel mappings and
nested artwork-weighted mappings, retaining their existing reusable component,
inversion, gain, bias and placement semantics. This prevents hidden project/source
metadata in otherwise ignored nested fields; canonical project output stays
unchanged. Limit archive and expanded JSON reads to the existing 4 MiB configuration budget;
reject duplicate/extra entries, paths, directories, encrypted entries, and
unsupported compression. Reuse Stored/Deflated support with bounded reads.
Write deterministic JSON/ZIP with stable ordering and no timestamp metadata.

Reuse existing same-directory temporary-file, flush/sync, bounded regular-file
read, no-follow, and guarded publication techniques. Share a small internal IO
helper only where it removes real duplication; do not redesign personal library
storage. Preserve existing files on failed serialization, write, or publication.
Create-only publication must use a no-replace filesystem primitive, so a file
created after the chooser check is never overwritten. Guarded overwrite uses
the existing local single-writer contract: recheck the fingerprint immediately
before atomic rename and reject detected changes. There remains a cross-process
race between that check and rename; this gate does not promise filesystem CAS
or protection against an unrelated writer in that interval. Do not present
ordinary rename as either create-only or a conditional replacement primitive.
File content is synced before publication. If directory sync fails after the
atomic publication, return a successful-save outcome with a durability warning;
do not falsely report that the old destination was preserved. A leftover temporary
hard link is cleaned up best-effort after successful create-only publication.

Ordinary project container-v1/document-v7, structural Pattern preset-v4, and
authored-resource-v1 remain unchanged. A new Preset format version accompanies
this new resource kind. Shared authored schema changes would require explicit
future version decisions; no migration or backwards-compatibility adapter is
introduced here. Neither project Open nor Pattern import silently accepts this
new format; explain which Load action the user should use.

**Project-as-Preset load:** Load preset... also accepts a valid current `.toniator`
project. Run its ordinary reader and complete source-integrity validation, then
capture the shared configuration and bind it to the destination. Do not import
the selected project's source, canvas, identity, location, or history. Preserve
the selected file untouched. A project with its source entry removed is invalid;
do not reinterpret it as a source-free Preset. Both routes share the same final
domain configuration application and history operation.

## Implementation boundaries and order, after authorization

1. **Respect the protected terminology boundary.** Prepare the bounded corrections
   described in `PatternAndPresetTerminologyRefactor.md`: structural resources
   are Patterns; document configurations are Presets; clarify that Addendum
   section 17's reset semantics apply to Pattern replacement. Obtain explicit
   protected-spec revision authorization before editing any of the five files.
   Preserve existing application, reconstruction, and no-name-dispatch semantics.
   Reconcile affected active documentation without rewriting checkpoint history.
   Current explicit user vocabulary distinguishes document Presets from the
   structural Pattern operations described with older internal/spec terms; do
   not delay authorized runtime work or alter those existing Pattern semantics.
2. **Domain and persistence.** Add immutable configuration capture/binding and
   one atomic history operation in domain, plus source-free IO and shared DTO
   extraction. Use existing domain types rather than a second effective model.
   Keep project IDs/source/canvas immutable for this operation. Validate before
   any revision/history change; include no-op and stale-root handling.
3. **GTK integration.** Add the two actions and bounded dialogs, worker events,
   busy-state/focus handling, status messages, and canonical preview scheduling.
   Keep new workflow logic in a focused `document_presets` app module with small
   main-window integration; do not undertake a general `main.rs` split.
4. **Verification and review.** Run the focused checks below, inspect native
   artifacts, obtain independent regression and hands-on UX review, fix findings,
   and stop at Implemented awaiting review. Acceptance, checkpointing, and any
   later gate require separate user action.

One writer at a time, including the parent. Expected production paths are
`crates/toniator-domain/src/lib.rs` plus a focused configuration module if useful;
`crates/toniator-io/src/lib.rs` and a document-Preset module; and
`crates/toniator-app/src/main.rs`, `app_events.rs`, and `document_presets.rs`.
Touch personal-library IO internals only for the narrowly shared file helper.
Add gate-specific domain/IO tests and an engine integration witness; engine,
geometry, sampling, renderer and CLI production changes are not anticipated.
Frontend compilation still verifies consumers of shared domain APIs. No new
dependency is expected: this is composition of existing domain, serde/ZIP,
filesystem and GTK mechanisms. Document touched nontrivial Rust functions/tests
with literal `///` under the repository rule.

## Focused verification and acceptance evidence

- Round trip a deliberately heterogeneous CMYK configuration with different
  recipes and channel deltas, source mappings, paint/opacity/visibility, ordered
  outputs and shared authored Curve Motif/shape/guide resources. Compare authored
  intent including absent versus explicit deltas, not only rendered equivalence.
- Save/load RGB and SourceColorAlpha, load across color models, and assert exact
  model/topology/order. Use role identity, never a coincidentally equal numeric
  channel ID, wherever existing selection/state must be reconciled.
- Inspect archive names and JSON to prove source data/identity, project ID,
  source filename, canvas and UI/runtime state are absent. After round trip,
  edit or remove only temporary test copies of personal Pattern resources and
  prove the Preset and loaded document remain independent.
- Verify canvas retention on both aspect ratios, unchanged density intent,
  and canonical geometry resolution. Keep authored resource coordinates in their
  existing units; do not assume an aspect change guarantees identical artwork.
- Verify successful load/undo/redo, no-op, dirty/clean savepoint restoration,
  redo preservation on cancellation/failure, stale workspace/revision callbacks,
  model-change refresh, save snapshot behavior, and original-source retention.
- Check invalidation combinations, not only individual fields: Pattern geometry
  plus mapping, topology plus paint, and response plus visibility. Current
  `squash_result` continues after effective geometry changes before examining
  mapping changes. Do not reuse that classification unchanged for arbitrary
  configuration replacement. Use one domain net-change classifier that considers
  all changed inputs and reports the strongest required level, with regressions
  for these concrete combinations and directly affected existing draft behavior.
  Classification precedence is ChannelTopology (model/roles/order/IDs), Source
  (mapping changes), Family (family inputs/definitions or family-used resources),
  Realization (response/shape/realization-used resources), then Presentation
  (paint/opacity/visibility). Combine all differences; preserve existing
  authority-only/no-effective-change semantics and existing dependency-based
  resource classification. Source bytes/canvas do not change in this operation.
- Round trip current ordinary project files through the shared serializer;
  verify mandatory source entry/reference, source length/digest and topology
  checks still reject malformed project archives. Test wrong-kind/version and
  failed/oversized reads, create/overwrite conflicts and interrupted publication
  using temporary directories. Do not restore deleted historical fixtures.
  Load valid projects as Presets and prove source/canvas/identity retention in
  the destination, source-integrity failure preservation, and equivalence with
  loading their source-free Preset exports.
- Exercise immutable `assets/raster-sample.png` (1024×1024) and
  `assets/vector-sample.svg` (900×620) through loaded-Preset canonical evaluation,
  PNG and SVG. Inspect exact native outputs under
  `target/validation/stage21b-gate5/`; preserve native RGBA and the SVG text/font
  caveat in `assets/README.md`. Existing all-17-Pattern checks need no blanket rerun.
- Build/check affected frontend and headless targets, run focused new tests and
  directly affected current persistence/history tests, strict Clippy for affected
  targets, formatting, and `scripts/validate_architecture.sh`. Avoid package-wide
  historical test sweeps and earlier validation-directory regeneration.
- In the private GTK harness, inventory each new/changed interactive control:
  menu items, choosers, scope confirmation, overwrite/error recovery and their
  actions. Check name/hierarchy, actual GTK role/state/actions, label relations,
  sensitivity, keyboard path, focus restoration and semantic action/readback.
  Follow `ui wait` → scoped `ui controls` → `ui inspect` → action → readback;
  inspect screenshots and logs, bundle evidence, then stop the private session.
  If a native chooser escapes to the user's desktop portal, stop immediately;
  request only the remaining desktop-specific inspection. Sway evidence does
  not establish GNOME/Mutter/portal or human acceptance.
- Independent regression review covers persistence sharing, source exclusion,
  atomic publication, state/history and invalidation. Hands-on UX review covers
  Preset-versus-Pattern clarity, whole-document scope and native dialog recovery.

## Planning exit

Current-project load, destination-canvas retention, and the remaining defaults
above now govern implementation, with both accepted input formats. Protected-spec
revision remains separately authorized. Acceptance and the subsequent issue
follow-up are recorded in the implementation record and `ISSUES.md`; the original
planning scope above does not authorize later product stages.
