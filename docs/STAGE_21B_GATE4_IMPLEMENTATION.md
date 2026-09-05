# Gate 21B-4 implementation and review evidence

Gate 21B-4 is **Complete and user-accepted** on 2026-09-04, together with the
[startup follow-up](STARTUP_SCREEN_IMPLEMENTATION.md). The user authorized
local acceptance actions. Checkpoint details are in `ProgressTracker.md`;
Gate 21B-5 has not begun. Acceptance reuses the verified implementation evidence
below; documentation/Git closeout does not claim a fresh test or visual run.

## Resulting behavior

The main window follows the v1.2 UI reference: a dominant canvas, header New
menu and history/help actions, contextual right inspector, actual-model channel
segments, mixed-channel Pattern summaries, and grouped appearance, variation,
options, and Advanced controls. Preview and Source share canvas extent, zoom,
and viewport state. Source display uses the canonical decoder with a bounded
4096-pixel longest edge. Preview display removes presentation padding without
changing generated geometry. Theme selection inherits GTK and the GNOME
light/dark preference, including live preference changes.

Pattern size uses a precise number entry following user review. Values such as
0.2, 0.1, and 0.05 can be typed directly and apply on Enter or focus leave.
The entry retains full numeric precision and adds no slider minimum; existing
finite-positive density validation remains authoritative. Invalid text stays
available for correction, and completed valid edits use normal Undo/Redo.

The Pattern Wizard presents five steps: Choose a layout, Shape the layout,
Choose placement, Draw and style, and Review. Controls use artist-facing
labels and explanatory tooltips. Invalid text remains visible with a specific
reason and invalid accessibility state; navigation and publication validate
pending entries before proceeding. Private history, nested editor history,
and the final single document Apply remain distinct. Saving a Pattern does
not Apply it to the document.

Personal Pattern management supports new saves, confirmed updates, fresh-ID
copies, stable-ID rename, trash/Undo, refresh, and library-root switching.
External file or root changes reject stale writes and offer appropriate
recovery. A successful own-write refresh preserves the active Review step.
Changing roots validates and scans the candidate before persisting it and
does not move the old library. Unchanged personal thumbnails are reused across
wizard opens. A dedicated private evaluation worker is reused across modal
lifetimes; closing the wizard cancels work, clears its caches, and joins the
old event bridge before another wizard can open.

Curve Motif accepts authored straight and curved guide provenance. Motifs
follow consecutive sites on each guide component; missing cadence runs split
instead of bridging gaps. Construction retains the existing adjacent-site
chord mapping, mirror/phase rules, shared geometry, and final clipping boundary.
The repair introduces no new preset format or Pattern-ID evaluator dispatch.
Guide-relative marks also choose compatible guide contributors when switching
to Points along guides; canonical validation rejects incompatible provenance.

## Verification record

Stage-owned runtime and artifact evidence is under
`target/validation/stage21b4/`; private GTK bundles are under
`.codex-work/evidence/ui-run-20260904-*`. These are local evidence, not portable
product fixtures or an acceptance checkpoint.

- `final-edit/summary.json`: all 17 built-ins and one personal curved Pattern
  reached Review and Apply in the real GTK wizard. `new-routes/summary.json`
  covers representative Guides, Scatter, and Spiral New workflows, All/named
  targets, and main history. Exact app tests cover recreation/materialization
  of all 17 recipes, channel targeting, reconstruction, and private history.
- Focused invalid-input, keyboard/Tab, dropdown, nested Guide/Shape/Motif,
  save/copy/rename/trash/Undo, root, and external-conflict workflows have
  semantic action/readback evidence. `save-conflict-final.json` verifies one
  recovery row, Reload/private Undo, and two consecutive own-write saves.
- Main-window screenshots cover dark/light, wide/narrow layout, header menus,
  Help, channel summaries, actual hover tooltips, and both source formats.
  Source/Preview at 67% use the same logical artwork extent.
  Final native readback at 200% retains both pan adjustments at 200 across
  Preview → Source → Preview (`final-viewport.json`).
- `intrinsic/summary.json`: 34 isolated release evaluations and 34 separate
  export invocations, all 17 Patterns against both immutable source formats.
  All exported SVGs rasterize with Inkscape. Later Curve Motif changes have
  fresh focused exports; unaffected matrix results are reused with that scope.
- Parent visual inspection covers at least one native PNG representative per
  Pattern, authored Curve Motif PNG/SVG output at 1024×1024 and 900×620, and
  representative SVG rasterizations. Generating the entire matrix does not
  imply individual visual inspection of every output. Native RGBA stays
  unchanged; separate alpha checks distinguish coverage and hidden RGB.
- `curve-performance-comparison.json`: three quiet release evaluator repeats
  per source and binary. Median cold time changed −2.99% for PNG and −0.74%
  for SVG; sampled peak RSS changed −0.56% and −0.58%. Both pass the 25% time
  and 15% RSS limits. Exporters are excluded from these measurements.
- Current focused IO tests cover atomic writes/readers, stale-write rejection,
  permissions, no-follow/traversal safeguards, and trash/Undo collision
  preservation. Current Curve Motif and orientation tests cover canonical
  construction and evaluation of both immutable source formats.
- Independent regression and hands-on UX reviews identified defects that were
  repaired and rechecked. Five orientation tests cover active-output validation,
  one-guide reduction, structural-only materialization, and reconstruction.
  The worker-reuse regression checks cancellation, stale-publication rejection,
  fresh cold-cache evaluation, queued-result cleanup, and shutdown. Final
  source review reports no further substantive findings.
- `wizard-cycles.json`: three warmups plus 30 real open/edit/cancel cycles
  pass. Settled RSS changes from 252,856 to 260,092 KiB: +7,236 KiB (7.07 MiB),
  or +2.86%, below both 32 MiB and 15% limits; readings include plateaus and a
  decrease. Initial large allocator retention from worker churn and GTK
  callback reference cycles were investigated and repaired. Release F owns
  this measurement; release G changes only three explanatory strings.
  Release H subsequently replaces the main Pattern size slider with an entry;
  it does not change the measured wizard lifecycle.
- `final-orientation-ui.json`: Points along guides evaluates successfully;
  a dropdown change with invalid point spacing rolls back while preserving
  the raw input and explanation. Correction reaches Review and one Apply.
  Parent inspected the valid, invalid, Review, and resulting main preview.
- Strict app/affected-engine Clippy, selected-file Rust formatting, architecture
  validation, and whitespace checks pass. Both immutable fixture hashes match.
- Release H: three focused Pattern size tests and strict app Clippy pass.
  `pattern-size-entry-ui.json` covers 0.2/0.1/0.05, Enter/Tab commits, Undo/Redo,
  invalid zero and correction. Parent inspected the numeric field, diagnostic,
  and completed 0.2 preview. Smaller values were checked for entry/history,
  not render completion. Fine-size preview latency is tracked as TON-003.
  The final private bundle is `ui-run-20260904-205534-215083`; focus automation
  emitted a GTK root-focus critical before recovering. This run is not claimed
  log-clean or human desktop acceptance.

## Review boundaries

The private harness uses Sway/wlroots, Cairo GTK presentation, isolated config
and library roots, and a private accessibility bus. Its evidence is not human
manual or GNOME Shell/Mutter acceptance. The SVG baseline contains live text;
host font fallback can affect its appearance as described in `assets/README.md`.
Keyboard dropdown checks retain a private VNC connection across focus and
selection because disconnecting a one-shot virtual keyboard can clear focus.

The first new or externally changed personal thumbnail still renders
synchronously; follow-up responsiveness work is tracked in `ISSUES.md` as
TON-002. The older intermittent RGB-edit-to-CMYK crash remains deferred as
TON-001. No compatibility migration, document-Preset implementation, protected
specification revision or push is included in this gate. The user's subsequent
acceptance authorizes local checkpointing and a planning-only next-gate handoff.

Gate 21B-5 remains the planned document-Preset gate. Its New-menu entries are
Load preset... and Save preset. The preferred source-free `.toniator` format
direction still needs an explicit serialization and application contract.
