# Toniator issues

This ledger records confirmed limitations and deferred reports outside the
current gate. Closing an item requires a reproducer or verification evidence;
an entry does not authorize a later stage or change an accepted contract.

## TON-010 — Scatter choices need greater artistic variety and clearer priorities

- Status: Open; development paused at the user's request (2026-09-20).
  This entry records follow-up work only; resume development when requested.
- Report: Clustered produces blotchy coverage that can compete with the artwork.
  It has artistic uses, but receives too much prominence relative to choices
  intended for image reproduction. The available algorithms remain too limited.
- Current implementation: Four generators—Random, Poisson disk, Jittered cells,
  and Clustered. There are no additional hidden generators. Uniform/artwork-weighted
  density and optional ordinary/weighted Lloyd relaxation modify these generators;
  they are not additional scatter algorithms.
- Explanation: Clustered deliberately groups points around randomly placed centers.
  With Uniform initial density those centers do not follow the artwork. Lower
  strength or broader spread softens the effect, but a few weighted Lloyd steps
  do not globally redistribute the clusters into faithful image stippling.
- Proposed direction, pending design review: Prioritize even blue-noise stippling,
  genuinely artwork-adaptive spacing, and a coherent artwork-weighted relaxed
  stippling workflow. Retain Random and Clustered as secondary texture effects.
  Distinguish initial placement, artwork-density control, and subsequent relaxation
  in the interface rather than presenting more combinations as new algorithms.
- Research candidates: Variable-density Poisson sampling; a complete
  [weighted Voronoi stippling](https://www.cs.ubc.ca/labs/imager/tr/2002/secord2002b/)
  workflow; and [weighted sample elimination](https://www.cemyuksel.com/research/sampleelimination/)
  for point-count control. A different implementation alone does not establish
  a meaningfully different artistic result.
- Verification when resumed: Present labeled side-by-side render comparisons on
  both immutable sample artworks, with settings, native PNG/SVG artifacts, test
  logs, and preview/Review timings. Check tonal fidelity, spacing, visible variety,
  Feature size response, cancellation and Apply/Undo. Keep the approximately
  50-second fine Poisson Review limitation tracked separately under TON-003.

## TON-004 — A second launch does not forward a file to the running app

- Status: Fixed and user-accepted (2026-09-05), checkpoint `8deb02d`.
- Evidence: The app parses its initial path locally and registers activation,
  without a GApplication file-open handler. A second launch presents the existing
  window, but its requested file is not forwarded to that window.
- Fix: GApplication forwards one local file to the existing window through its
  normal dirty-document guard and asynchronous loader. Cancel and failed loads
  preserve the document; Save retains the requested path until the save succeeds.
  Busy/modal requests ask the user to retry rather than interrupt current work.
  File choosers retain explicit ownership, including out-of-process portals;
  pending replacement loads also guard authored edits and queued callbacks.
  Caller-relative and non-UTF-8 paths survive forwarding; file batches reject.
- Verification: focused native-path, unsaved-decision and close-controller tests,
  strict app Clippy and architecture checks pass. Private GTK reproduced the old
  dropped request, then verified PNG/SVG/project forwarding, Cancel/Escape,
  Discard, Save As cancellation, successful Save then Open, competing requests,
  failure preservation and success-only Recent Files. Evidence:
  `.codex-work/evidence/ui-run-20260905-135310-78698`, final chooser checks in
  `.codex-work/evidence/ui-run-20260905-141335-92087`, and
  `target/validation/ton004/`. Automated Sway is not GNOME/Mutter acceptance.

## TON-005 — Overall preview progress understates rasterization and updates sparsely

- Status: Fixed and user-accepted (2026-09-05), checkpoint `8deb02d`.
- Report: Overall reserves only roughly its last 5% for rasterization, which
  often takes about half the rendering time; updates also arrive too infrequently.
- Scope: Rebalance stage contributions and improve real progress reporting across
  main/private previews, retaining monotonicity, cancellation and canonical output.
- Fix: Preparation/decode share 10%, family generation 15%, output realization
  20%, scene construction 5%, rasterization 49%, and final publication 1%.
  Main, Wizard and Advanced labels expose tenths of a percent. Cancellation polls
  no longer count as completed output work. Region/motif/raster producer callbacks
  retain real increments; other outputs report completed coordinator milestones.
- Verification: Fixed-weight, monotonic/ticketed progress, duplicate coalescing,
  pixel-neutral raster progress and strict affected-target checks pass. Native
  raster progress moves at roughly 100 ms observation intervals in a dense SVG
  check; exact JSON and screenshot are in `target/validation/ton004/` and the
  private GTK run ending `142902-100954`. Weights are effort estimates, not an ETA.

## TON-006 — Preset actions remain disabled after applying a customized Pattern

- Status: Fixed and user-accepted (2026-09-05), checkpoint `8deb02d`.
- Reproducer: Customize and Apply `assets/AuthoredPresets/TestPatternDoc.toniator`;
  after the document preview updates, Load preset and Save preset stay disabled.
  Saving the project refreshes them again.
- Cause: Apply refreshes sensitivity before releasing the private wizard; its
  terminal close omits the subsequent menu refresh.
- Fix: Refresh applicability after releasing private editor/library surfaces.
- Verification: Customized the actual TestPatternDoc, applied a guide-angle edit,
  observed both enabled menu actions, and saved `customized.toniator-preset` while
  the document remained dirty. No project Save was needed; the input file is
  not written by the check. Native menu screenshot and event evidence:
  `.codex-work/evidence/ui-run-20260905-140256-86459` and
  `target/validation/ton004/`.

## TON-007 — Wizard titlebar close can leave a detached window until another click

- Status: Fixed and user-accepted (2026-09-05), checkpoint `8deb02d`.
- Report: Wizard Cancel or X sometimes requires repeated clicks.
- Confirmed reproducer: Open the user's TestPatternDoc, open its unchanged
  Pattern Wizard, and invoke one native window.close action. The wizard remains
  visible after its controller has already detached; Cancel then has no controller
  to target. The outer close-request veto defeats the nested window.close call.
- Fix: Destroy the terminal wizard window after the existing dirty-draft decision,
  then refresh the main window. The existing cancellation/join cleanup remains.
- Verification: One clean Cancel or X dismisses the window; dirty Cancel and X
  each dismiss after one Discard changes confirmation. Semantic absence, screenshots
  and logs are in the 140256 and 141335 private GTK runs listed above.

## TON-008 — CMYK channel rotation plus offset can hang

- Status: Resolved; user confirmed the rebuilt numeric-edit fix works on 2026-09-12.
- Report (2026-09-12): Applying rotation and an offset to one CMYK channel
  repeatedly reports `gtk_widget_get_parent: assertion 'GTK_IS_WIDGET (widget)' failed`
  and hangs; the shell subsequently reports the process killed. Source is
  `assets/vector-sample.svg`, default pattern, Feature size 0.25. Prior RGB:
  R rotation30/X4.5, G rotation60/Y4.5, B neutral. CMYK: C X4.25/rotation22.5,
  M Y4.25, Y X3.18/Y3.18, K neutral. Intended M45/Y67.5 rotations were never
  entered because of the stall. A later CMYK-first/RGB-second attempt worked.
- Current evidence: The release completes grid transforms in CMYK for raster
  Cyan (23 degrees, X 7.25) and SVG Black (23 degrees, X 7.25, Y -4.5).
  Private GTK screenshots/readbacks show updated previews. A separate focus
  automation attempt emitted a different `gtk_root_get_focus` warning; no
  matching hang or stack trace is established. No matching OOM journal entry.
- Fix: Accepted numeric/reset/End edits defer inspector hierarchy replacement
  to the existing idle queue. A native GTK regression proves the old code
  detached selector children during entry activation; the fix preserves them
  until the callback unwinds while publishing history immediately. The original
  intermittent hang is not established as solely caused by this defect.
- Acceptance: After being asked to retry the rebuilt release with RGB first,
  then CMYK, the user confirmed: “Okay that work is good now.” See
  `.codex-work/evidence/review-0.3-cmyk-transform-investigation.md`.

## TON-009 — Feature size may not increase site density in random Patterns

- Status: Implemented and user-accepted (2026-09-27) for release 0.3.3.
- Reported Pattern: `Even random circles`. The user suspects most random
  Patterns are affected; that broader scope needs verification.
- Report: Reducing `Feature size` correctly reduces mark size but does not add
  more sites to maintain image density.
- Expected behavior: Finer feature sizes increase the number of sites as well
  as reducing mark size, maintaining the intended image density.
- Resolution: Feature size now scales Poisson separation, cluster spread, and
  explicit exclusion distances alongside density-derived population. Parametric
  pitch and along-curve spacing also follow Feature size. The control rejects
  values below 0.01. Full-size SVG Poisson evaluation at 0.5 and 2, native PNG/SVG
  witnesses, and private GTK inspection pass; both immutable sources are covered
  by focused family tests. Evidence: `.codex-work/evidence/2026-09-20-feature-size.md`.
  This fix is not part of the previously published 0.3.1 release.

## TON-001 — Intermittent RGB edit to CMYK crash

- Status: Open; deferred pending a reliable reproducer and separate authorization.
- Report: Switching an edited RGB document to CMYK has intermittently crashed.
  A deterministic reproduction is not established. Gate 21B-4 does not diagnose
  or claim to repair this older report.
- Next step: Capture the exact document and edit sequence with a private GTK
  log/backtrace, then scope the repair from that evidence.

## TON-002 — First personal Pattern thumbnail can block the gallery

- Status: Open; follow-up performance work.
- Evidence: Gate 21B-4 private GTK checks observed a first gallery open taking
  more than three seconds with two saved curved-motif Patterns. Rendering a
  thumbnail for a new recipe/fingerprint currently runs synchronously.
- Current mitigation: Repeated opens reuse a cache keyed by recipe/fingerprint;
  stale entries are evicted when the catalog changes.
- Next step: Schedule bounded thumbnail generation off the GTK thread, retain
  revision/cancellation checks, and verify responsiveness on first open and
  after external edits. Do not change canonical rendering to speed up thumbnails.

## TON-003 — Fine Pattern sizes have slow full-resolution previews

- Status: Open; follow-up performance work.
- 2026-09-20 wizard regression: `vector-sample.svg`, Clustered with four
  artwork-weighted Lloyd steps, then Poisson disk at Feature size `0.1`.
  The new GTK workflow exceeded its 90-second Review watchdog. Normal Poisson
  construction also hit an obsolete reserved neighbor-work ceiling; that ceiling
  is removed, with cancellation and explicit caller bounds retained. Identical
  still-image Start/End checks now share one construction result. Review then
  completed in about 50 seconds, which remains unacceptable interactive latency.
  Native profiling reports roughly 543,155 sites per channel and about 45 seconds
  in family generation/refinement. No density clamp or disabled refinement is used.
  Reproduction and coverage: [Pattern Wizard tests](docs/PATTERN_WIZARD_TESTING.md).
- 2026-09-05 improvement: The progress investigation found a quadratic circular-mark
  usage lookup. An exact-coordinate index now preserves the first matching family
  site and positive-radius membership without scanning every site for every mark.
  The same private debug SVG size-0.2 observation fell from about 45.4 seconds to
  2.9 seconds between first observed progress and completion. These are individual
  UI observations, not controlled benchmarks. Membership and dependent-output
  ordering regressions pass; broader fine-size performance remains open.
- Reproducer: Open `assets/vector-sample.svg` with the default Straight circular
  marks Pattern and RGB channels; enter Pattern size `0.2`. In the private
  release-H GTK run, preview completion exceeded the 20-second automation wait
  and subsequently completed. This is an observed latency bound, not a benchmark.
- Evidence: `ui-run-20260904-205109-212917/pattern-size-0.2-rendered.png` under
  `.codex-work/evidence/`. Exact smaller entries and history are checked separately
  from render completion; this does not claim usable rendering speed at `0.05`.
- Next step: Profile dense-site construction and preview rendering at 0.2 and
  below, including cancellation and both source formats. Preserve canonical
  geometry/export fidelity and truthful preview state when scoping an optimization.

## TON-011 — Optional garment PNG preparation

- Status: G0 and its measurement contract are accepted as the G1a implementation
  baseline; the uncertainty band remains provisional. G1a, G1b, and G2a are
  accepted. The user accepted the revised G2b implementation on 2026-10-07 for
  the v0.4.0 release. G2c broader evidence and work beyond the explicitly
  authorized G2b corrections remain gated.
- Request: Offer an optional PNG preparation path for garment printing. Let the
  artist set maximum physical dimensions and DPI, choose export-local pixel
  corrections, preview the result, and preserve the editable project and normal
  PNG export behavior.
- Boundary: No supplier product, placement, universal DPI recommendation, or
  minimum print feature is assumed. Prepared output is not a print-safe
  certificate or a prediction of fabric color, adhesion, hand, or wash life.
  Preparation is explicitly enabled for each PNG export; the default is off.
- Implemented G1a: document-owned optional physical size in canonical mm and
  independent positive-feature/negative-gap thresholds; zero disables either
  threshold. History-only edits support undo/redo and do not invalidate renders.
  Projects write schema 11 and strictly read schema 10 with default intent;
  opening preserves the schema-10 file, while saving writes schema 11. The
  pre-G1a v0.3.3 application cannot read schema 11. Preset format 3/configuration
  schema 10 and Pattern format 5 are unchanged.
- Evidence: [G1a report](target/validation/garment-g1a-20261003/report.md)
  records ten focused passing tests, scoped checks, and byte-identical output
  for six PNG and two available SVG comparisons. The broader all-targets Clippy
  attempt failed on an unrelated older domain lib-test initializer; the scoped
  library/binary check passed. The earlier [G0 report](target/validation/garment-g0-20261003/report.md)
  records the separate baseline evidence. No detector, cleanup, GUI, or GTK
  behavior is claimed for G1a.
- Implemented G1b: the project-only `toniator preflight --input PROJECT`
  command evaluates the selected current frame's final transparent output and
  emits deterministic JSON. Runtime placement and width-threshold overrides
  apply only to this run. Alpha support inventory is independent of the width
  checks; positive-feature and negative-gap results are advisory, with exact
  candidate runs and explicit unavailable states. G1b adds no persistence
  schema, GUI, artistic cleanup, or print-safety claim.
- Evidence: [G1b report](target/validation/garment-g1b-20261003/report.md)
  records 16 focused passing engine and CLI tests, 50 headless CLI commands with
  185 checks, both immutable source assets, and native PNG parity. Morphology
  review uses a tiny analytic fixture; it was not run across the full asset
  matrix. Stress performance and active cancellation during heavy work remain
  unverified. See the report for limits, raw outputs, and review derivatives.
- Accepted G2a: The [G2a headless report](target/validation/garment-g2a-20261004/report.md)
  remains historical evidence for the accepted internal report/raster pair and
  chooser-entry capture. Its 2026-10-04 tests do not establish the later G2b PNG
  preparation workflow. The [permission diagnosis](target/validation/garment-g2a-20261004/permission-diagnosis.md)
  remains attached to that earlier work.
- Accepted G2b: PNG options have an explicit, default-off **Prepare for
  garment printing** control. Maximum physical width/height and DPI determine
  one aspect-preserving pixel target, with the corresponding PNG density
  metadata. At alpha 128, the prepared raster becomes binary alpha. Optional
  export-local corrections remove isolated marks below a separate cutoff,
  thicken narrow positive features, and treat narrow gaps using average-edge
  fill, background-color fill, custom-color fill, or growth to minimum width.
  All correction thresholds default to zero, and zero disables that pass.
  Preview and PNG export use the same captured
  prepared pixels. Preview updates automatically on open and after changes,
  with debounced cancellation and one worker retaining the latest request. The
  mm/in selector precedes dimensions. See the [automatic-preview evidence](target/validation/garment-auto-preview-20261007/verification.md). The correction preview supports 100%/200% zoom and scrollbars
  for panning. A separate background-fill swatch is used on transparent output;
  **Use garment color** copies the viewer swatch once and later viewer changes do
  not change the copied fill. These choices do not edit or persist to the
  project. There is no advisory findings section in this dialog. Ordinary PNG
  export remains on its existing path when preparation is off.
- G2b verification: Focused cleanup, alpha-boundary, preview/export parity,
  physical-fit, view, and PNG metadata tests passed. Formatting, strict app
  Clippy, app build, and `git diff --check` passed. Private Sway verification
  covered the final controls, keyboard/pointer input, swatch independence,
  zoom/pan, and a native chooser export from the raster input. Headless export
  tests cover both immutable inputs, raw RGBA parity, binary alpha, dimensions,
  and density metadata. The exact commands,
  screenshots, logs, and raw outputs are in
  [revised G2b verification](target/validation/garment-binary-alpha-20261007/verification.md).
- Limits: Square-grid/axis conventions approximate diagonal physical widths.
  Correction is capped at 32 million pixels, 256 pixels per width, and bounded
  expansion work; a budget failure writes no partial file. This is not physical
  print validation. GNOME/Mutter, production-portal behavior, and human review
  are not established by the private Sway run. The earlier
  [G2b runtime report](target/validation/garment-g2b-20261007/runtime-verification.md)
  is historical evidence for the superseded advisory-review UI, not proof of the
  revised workflow. TON-013 remains a separate deferred SVG issue; no SVG change
  is part of this work.

## TON-012 — Transparency and color confidence needs separate investigation

- Status: Open investigation; current evidence does not establish a blanket
  hidden-RGB, alpha, color, or export-pixel defect.
- Confirmed by current source and bounded G0 output: The renderer converts its
  linear result to straight sRGBA. Current PNGs contain IHDR/IDAT/IEND without
  ICC, sRGB, or pHYs metadata; focused tests confirm straight-sRGBA encoding.
  Source raster decoding uses `to_rgba8()` without a Toniator-owned ICC
  conversion policy. Missing metadata alone does not show that output pixels
  are wrongly encoded. In 18 tested **Source color + alpha** hidden-RGB cases,
  decoded RGBA and file bytes matched. Clean-fade and white-matted inputs had
  identical alpha planes, while positive-alpha RGB differed toward white in
  the matted output; that supports a visible-color distinction, not alpha bleed.
- User-reported from Debian v0.3.3, not reproduced this turn: Flat-art output
  was preserved; 18 hidden-RGB cases showed no bleed; visible edges whose RGB
  was deliberately blended toward white while partial alpha remained looked
  pale; disabling antialiasing made edges jagged and lost some tiny marks; 2x2
  details survived with both antialiasing settings; minimum fill 0.2 added 8.4%
  coverage without discarding marks by size; CMYK PNG export dialog defaulted
  to white; exported PNG lacked profile and resolution metadata. The report
  does not reproduce those Debian GUI observations. The current tiny fixture's
  minimum-fill 0→0.2 support increased 29.3%; it is separate from the reported
  8.4% result.
- G0 evaluated both immutable sources and saved/reopened project outputs, in
  addition to source fixtures; see the linked report for exact scope. A private
  GTK startup was automatically rejected before launch, with no retry or
  bypass. No GTK readback, independent Debian 13.6 reproduction, GNOME/Mutter
  review, supplier print, or ICC conversion/soft proof was performed. The
  alpha-based preflight cannot detect color-only detail
  inside opaque areas. TON-001 and TON-003 remain separate open reproductions;
  TON-008 is resolved in this ledger, and TON-010 remains paused.
- Next step: The proposed G2 preview/backing status is presentation only and
  does not resolve TON-012. A transparency/color investigation remains separate
  and gated; keep supplier/process behavior and the separate TON-001 and TON-003
  reports distinct. G0 remains accepted headless evidence; GTK is separately
  pending, and current evidence does not establish broad source-profile behavior.

## TON-013 — SVG export emits zero-radius circles and rounds tiny positive radii to zero

- Status: Open, deferred as a separate bounded SVG-export correctness task.
  This issue is independent of garment preflight/G2a and G2R; no fix is part of
  the current stage.
- User-reported impact: Official SVG exports contain non-rendering circles that
  clutter Inkscape layers. The user reports selection/union difficulty and an
  associated crash risk; selection, union, and crash causality have not been
  independently reproduced.
- Diagnostic scope: The supplied fixture contains 10,347 circles: 2,212 with
  literal `r="0"` (Red 999, Green 987, Blue 226), and 8,135 positive-radius
  circles, all retained. The smallest positive radius is 0.003137. The checked
  objects have no stroke, transform, CSS override, animation, filter, `use`
  reference, or mask making the zero-radius elements render. These counts apply
  to this fixture only.
- Supplied inputs remain unchanged and user-owned: `assets/raster-sample-export.svg`
  (SHA-256 `eddd1d2fee279d00a704c072f1519a7f2c0a01c7bfd7265bb189a7c4e96c1457`)
  and `assets/raster-sample-svg-export.toniator` (SHA-256
  `8a84ddfcd93b15e9057bfca714315358b4ba6baad251845d08934d542469129a`). The
  clean diagnostic comparison `raster-sample-export.zero-radius-removed.svg`
  (SHA-256 `06f371bd45ba3b058eea3a485951e845414c845f69660267ac4487dc2c70c39a`)
  is a separate derivative, not the failing fixture or a replacement input.
- The supplied Inkscape 1.4.4 parity record compares transparent 1024×1024 RGBA
  output for the fixture and that derivative: zero changed pixels, maximum
  channel difference zero, and equal RGB and alpha. This is limited to the
  supplied fixture and resolution. It does not test every zoom or output size;
  no Inkscape rerun occurred for this triage. The additional XML recount was
  automatically rejected before execution and was not retried; the counts and
  parity above are attributed to the supplied diagnostics.
- Diagnostic records retained locally: `diagnosis-report.txt`,
  `xml-analysis.json`, and `render-parity.json`. These supplied records and
  user-owned input files are not distributed in this checkpoint.
- Source assessment at HEAD `a980005`: both supported circle-writing branches
  emit circles unconditionally in `crates/toniator-render/src/lib.rs`
  (`GeometryOutput::CircularMarks` near 4443 and `CanonicalMark::Circle` near
  4493). `compact_number` near 4719 formats to six fractional digits, so a
  positive radius such as `1e-7` can serialize as zero. This is a confirmed
  serialization hazard, but it is not proven to explain every zero-radius
  element in the supplied fixture. `crates/toniator-patterns/src/lib.rs`
  `radius_from_ink_with_diameter` near 13703/13726 can produce genuine zero
  geometry when minimum fill and evaluated ink are zero; do not change geometry
  to fix serialization.
- Next step: In a separately approved SVG task, omit only genuine `r == 0`
  circles in every supported circle-writing branch. Preserve every valid
  positive radius, including `1e-7` and values that the current formatter would
  round to zero; do not add an epsilon cutoff or drop values after formatting.
  Keep canonical marks, sites, IDs, paint, and intentional tiny geometry intact;
  do not force circles into paths. Add focused structural tests for exact zero,
  tiny positive radii, both branches, and retained associations, plus bounded
  raster appearance parity. Test Inkscape selection/union only in a separately
  authorized GUI check. Do not alter raster/preview output, minimum-fill
  geometry, or unrelated numeric formatting as part of this fix.

## TON-014 — SVG font discovery can reject available system fonts

- Status: Open; low priority and deferred at the user's request. Tracking only;
  no diagnosis or fix is authorized by this entry.
- Observed failure: The unchanged published `b819fd4` AppImage rejects
  `vector-sample.svg` on a clean Debian 13.7 / Xfce 4.20 / X11 VM during stock
  file-chooser import and a separate direct-file launch:
  `source.svg.font_policy: no usable system sans-serif font is available`.
  DejaVu fonts are installed and `fc-match sans` succeeds. Root cause remains
  unresolved; these observations do not establish why the importer rejects them.
- Candidate SHA-256:
  `9643e6fc6577c2ec7a687283d71fdbe132062c569ebac652bfba50f2ed00e86a`.
  Existing packaged-QA evidence: `packaged-qa-progress.json` and
  `vm-check-toniator-vector-direct.png` in the local 2026-10-05 QA evidence.
- Scope: Raster import/edit, project save/close/relaunch/reopen, and PNG/SVG
  export passed in that VM. SVG export success does not establish SVG source
  import success or exact font/output parity. This issue is separate from
  TON-013's SVG-export geometry finding.
- Product boundary: Toniator is a deforming/conversion tool; original artwork
  is retained as reference material, not reproduced pixel for pixel. This issue
  concerns SVG import availability and font resolution, not source/output pixel
  equivalence. Deterministic transformations, saved reference data, and persistence
  remain separate verification concerns.
- User guidance: Convert SVG text to paths/curves in the authoring tool before
  export when precise source-reference interpretation matters, instead of relying
  on other consumers having identical fonts installed. This stabilizes the source
  reference, not Toniator output fidelity. It is not a verified workaround for
  the importer error above; textless/outlined SVG import has not been tested for
  this failure.

## TON-015 — Advanced sidebar entry has conflicting disclosure behavior

- Status: Open; user-reported, not reproduced; tracking only. No implementation
  is authorized by this entry.
- Report: The main sidebar presents a single button under a collapsible
  Advanced heading. The user considers this organization confusing.
- Acceptable designs, pending implementation planning: make Advanced a complete
  collapsible section containing its settings, or make it a direct button that
  opens a dialog. This entry does not choose between them.
- Verification: Inspect the live sidebar and keyboard/accessibility behavior;
  confirm the chosen design has either settings inside the expanded section or
  a direct dialog-opening action, with no orphan button under the heading.

## TON-016 — Shared appearance baseline should survive channel-model changes

- Status: Open; user-reported concern, not reproduced; tracking only. No
  persistence or schema change is proposed by this entry.
- Report: Appearance settings applied to All compatible channels may lose their
  shared baseline when Color & Channel Model changes.
- Expected behavior: Preserve the shared baseline across model changes. Only
  channel-specific deviations without a relevant counterpart in the destination
  model may change; such an exception must not reset the shared baseline or all
  channel appearances.
- Verification: Apply distinct appearance values to All compatible channels,
  change Color & Channel Model, and verify the shared values remain. Repeat with
  a per-channel override and verify only an override without a destination-model
  counterpart may change.

## TON-017 — Applying a built-in Pattern should not require a review step

- Status: Open; user-reported, not reproduced; tracking only. No implementation
  is authorized by this entry.
- Report: Selecting a built-in Pattern in Pattern Wizard presents Review Pattern
  as an action before applying the familiar Pattern.
- Expected behavior: The selected built-in offers exactly Cancel, Customize,
  Create new, and Apply Pattern. Apply Pattern directly applies the familiar
  built-in without another review step; this is a behavior change, not only a
  label change. Preserve Cancel, Customize, and Create new semantics. Customize
  remains the way to inspect and step through settings. This issue does not
  authorize redesigning the custom-Pattern workflow.
- Verification: In the live wizard, verify the four actions and direct built-in
  application; also verify Customize can inspect/step through settings and that
  Cancel and Create new retain their current behavior.

## TON-018 — Calibrate RGB and CMYK advanced settings for source-tone fidelity

- Status: Deferred at the user's request (2026-10-07); tracking only. Do not
  begin calibration until the user requests it and supplies the presets.
- Goal: Dial in Advanced settings separately for RGB and CMYK, using each
  mode's respective user-provided preset, so the overall image tone matches
  the source artwork.
- Inputs pending: The user will provide the preset or presets for the two
  modes before calibration begins.
- Scope when resumed: Compare each preset's rendered result with the source,
  tune its Advanced settings, and record the final values for each mode.
- Verification: Present source, baseline, and calibrated comparisons for RGB
  and CMYK, with the corresponding preset and settings identified, for the
  user's review of overall tonal fidelity.
