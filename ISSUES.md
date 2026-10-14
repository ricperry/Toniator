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

- Status: Permanently deferred at the user's request (2026-10-08).
- Report: Switching an edited RGB document to CMYK has intermittently crashed.
  A deterministic reproduction is not established. Gate 21B-4 does not diagnose
  or claim to repair this older report.
- Disposition: The user has been unable to reproduce the issue across numerous
  coding sessions, trials, and tests since shortly after it was added to this
  ledger. Retain the historical report without claiming a diagnosed fix; no
  further investigation or repair is planned.

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
  inside opaque areas. TON-003 remains separately open; TON-001 is permanently
  deferred. TON-008 is resolved in this ledger, and TON-010 remains paused.
- Next step: The proposed G2 preview/backing status is presentation only and
  does not resolve TON-012. A transparency/color investigation remains separate
  and gated; keep supplier/process behavior and the separate TON-001 and TON-003
  reports distinct. G0 remains accepted headless evidence; GTK is separately
  pending, and current evidence does not establish broad source-profile behavior.

## TON-013 — SVG export emits zero-radius circles and rounds tiny positive radii to zero

- Status: Fixed and user-accepted (2026-10-08); not yet committed. The fix is confined
  to SVG serialization and remains independent of garment preflight/G2a and G2R.
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
  The current retained `.toniator` working copy has a different hash from this
  historical diagnostic hash and was preserved without using it as a reproduction
  baseline.
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
- Implementation: Both supported circle-writing branches now omit only exact
  zero radii during SVG serialization. Original enumeration IDs and paint
  associations remain stable, and every positive radius is retained. Radius
  formatting alone uses shortest nonzero round-trip text when the shared compact
  formatter would produce zero; canonical geometry, raster output, and other
  numeric formatting are unchanged.
- Verification: Focused renderer tests cover signed zero, tiny positive radii,
  both writer branches, and retained IDs/paint. The 64×64 Inkscape fixture
  parity under `target/validation/issues-20261008/ton013-zero-circle-parity/`
  rasterizes an ordinary circle with `r=0` and `r=-0`, then the same SVG after
  only those zero circles are removed; decoded RGBA is identical. Both immutable
  sample SVG outputs were also rasterized by Inkscape as recorded in the round
  report. This is bounded consumer evidence; GUI selection/union was not tested
  and no causal claim is made about the reported crash.

## TON-014 — SVG font discovery can reject available system fonts

- Status: Fixed and user-accepted (2026-10-08); not yet committed.
- Observed failure: The unchanged published `b819fd4` AppImage rejects
  `vector-sample.svg` on a clean Debian 13.7 / Xfce 4.20 / X11 VM during stock
  file-chooser import and a separate direct-file launch:
  `source.svg.font_policy: no usable system sans-serif font is available`.
  DejaVu fonts are installed and `fc-match sans` succeeds. `fontdb`'s original
  generic SansSerif query can still fail when the configured preferred family
  alias names a font that is not installed; `fc-match` alone does not establish
  that this query succeeds.
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
  reference, not Toniator output fidelity. It is not a workaround required by
  the implemented fallback.
- Implementation: The sampler keeps the existing generic SansSerif query as
  first choice. Only if it fails, it tries exact installed family matches in
  deterministic order: DejaVu Sans, Noto Sans, Liberation Sans, and FreeSans.
  It updates the generic mapping only after an actual fontdb family match; the
  existing no-usable-font diagnostic remains when no candidate exists.
- Verification: Focused tests retain a successful original generic-family
  selection, resolve a broken FreeSans alias to available DejaVu Sans, reject an
  empty database, and decode the immutable text SVG. The immutable PNG source
  field remains unchanged. A parent-verified CLI inspection in a fresh Debian 13
  container without a FONTCONFIG_FILE override succeeded with glyph coverage
  true; details are in `target/validation/issues-20261008/report.md`.

## TON-015 — Advanced sidebar entry has conflicting disclosure behavior

- Status: Fixed and user-accepted (2026-10-08); not yet committed. The existing Advanced
  Settings button now opens its dialog directly; the orphan expander was removed.
- Report: The main sidebar presents a single button under a collapsible
  Advanced heading. The user considers this organization confusing.
- Verification: Blueprint/resource source and app checks pass. Private Sway
  screenshots in `.codex-work/evidence/ui-run-20261008-134656-111906/`
  show the Advanced Settings button without an orphan disclosure heading and
  its directly opened dialog. This is automated Sway visual evidence, not
  GNOME/Mutter or human desktop acceptance.

## TON-016 — Shared appearance baseline should survive channel-model changes

- Status: Fixed and user-accepted (2026-10-08); not yet committed. A stored last-ALL
  baseline now survives replacement channel models.
- Report: Appearance settings applied to All compatible channels may lose their
  shared baseline when Color & Channel Model changes.
- Implementation: `DocumentAppearanceBaseline` stores optional Start values
  for density, density aspect, pattern rotation, and X/Y translation in both
  `Document` and reusable `DocumentConfiguration`. ALL assignments update only
  specified fields; named/End edits do not. Model changes seed canonical
  channels from this authority rather than old channels. ALL Pattern replacement
  clears recipe-relative density/aspect/rotation entries and keeps translations.
  Project schema 12 and Document Preset configuration schema 11 persist the
  baseline; older schema 11/configuration 10 are rejected with no inference.
- Verification: Focused tests cover all five values, partial updates, named and
  End edits, RGB→CMYK→Source color + alpha→RGB model changes, ALL versus named
  Pattern replacement, baseline-only `None` invalidation, exact Undo/Redo, and
  project/Preset round trips plus obsolete/invalid rejection. App-level witnesses
  cycle both immutable sample workspaces through the real model selector and
  export native PNGs under `target/validation/issues-20261008/ton016-model-witnesses/`.
  Private Sway screenshots `10-ton016-all-five-baseline.png` through
  `15-ton016-returned-r-baseline.png` show all five values retained across RGB,
  CMYK, SourceColorAlpha, and RGB return after a named-channel override. AT-SPI
  checks confirm untouched Tab/focus-leave creates no baseline history, Enter
  on the same visible value records it once, and focus leave does not add a
  second entry. Exact Undo/Redo and changed-value commit readbacks are in
  `target/validation/issues-20261008/report.md` and its linked evidence.

## TON-017 — Applying a built-in Pattern should not require a review step

- Status: Fixed and user-accepted (2026-10-08); not yet committed.
- Report: Selecting a built-in Pattern in Pattern Wizard presents Review Pattern
  as an action before applying the familiar Pattern.
- Expected behavior: The selected built-in offers exactly Cancel, Customize,
  Create new, and Apply Pattern. Apply Pattern directly applies the familiar
  built-in without another review step; this is a behavior change, not only a
  label change. Preserve Cancel, Customize, and Create new semantics. Customize
  remains the way to inspect and step through settings. This issue does not
  authorize redesigning the custom-Pattern workflow.
- Implementation: Built-in gallery entries expose exactly Cancel, Customize,
  Create new, and Apply Pattern. Apply Pattern uses the captured current target
  directly, publishes one atomic undoable replacement, and shows affected channel
  names in the gallery. Personal Patterns retain Review Pattern; Customize,
  Create new, and Cancel retain their separate paths.
- Verification: Eight focused app tests cover built-in-only admission, gallery
  action ownership across preview refresh, Create new candidate markers, target
  disclosure, personal/stale rejection, direct apply, delta clearing, and
  named/ALL Undo/Redo. Private Sway evidence confirms direct Apply, Undo/Redo,
  Cancel, Customize, and Create new flows. After Create new → Back → selecting
  One Guide Lines, live AT-SPI readback reported the card toggle pressed and the
  Apply Pattern action correctly named/described; direct Apply closed the wizard
  and rendered the recipe. The selected card was offscreen in the saved frame;
  see the round report for the saved-versus-live evidence boundary. This is not
  GNOME/Mutter or human desktop acceptance.

## TON-018 — Calibrate RGB and CMYK advanced settings for source-tone fidelity

- Status: In progress as of 2026-10-13. Included with the accepted TON-013–17
  repairs in the published 0.4.1 prerelease at checkpoint `f3605cd`.
  The verified completion round is included in the 0.4.2 release checkpoint;
  see [release notes](docs/releases/v0.4.2.md).
  TON-018 itself is not accepted as complete; the remaining work below stays open.
- Current behavior: Production stores one calibration per finished structural
  Pattern revision, persists it with the Pattern and in applied projects and
  Document Presets, reuses it for artwork, Feature size, whole-Pattern rotation,
  seed variation, and response edits, replaces it after completed structural
  edits, and offers manual Recalibrate in Advanced. Structural calibration jobs
  publish atomically and reject stale completion. Advanced owns response
  settings; the Wizard preview stays neutral and structural. Variable retains
  artwork-weighted density. Uniform targets 99.9% statistical geometric
  coverage; strict Visible Mark Margin requires Variable. Fixed authored
  spacing remains fixed; Scatter population derives from artwork area, with
  Feature size scaling placement and marks together.
- Persistence: Project schema 17, media container 2, Document Preset format 3
  with configuration schema 16, and standalone Pattern format 8 are current.
  Supported older Document Presets have deterministic conversion that
  preserves original bytes; ordinary projects remain current-only.
- Verification: The exact photo's prior Straight RGB Feature 0.25 failure now
  exports natively in 11.12 s at 1.33 GiB peak memory on the tested machine.
  Focused evidence covers both immutable sample assets and includes [three-guide
  seam repair](target/validation/ton018-three-guide-seam-20261011/README.md),
  [outline simplification](target/validation/ton018-outline-relative-20261011/README.md),
  and [area-raster coverage](target/validation/ton018-area-raster-20261011/README.md).
  The [private Sway screenshot](.codex-work/evidence/ui-run-20261011-184949-688907/01-guide-preview.png)
  was reviewed and the session stopped. Verification passed: `cargo build -p toniator-app -p toniator-cli --offline`.
  Scoped strict Clippy for render/app/CLI, the focused off-canvas circle
  witness, and `git diff --check` also pass. Automated Sway/wlroots evidence
  does not establish human review or GNOME/Mutter acceptance. This earlier
  export-cap limitation is superseded by the streaming milestone below.
- Final SVG milestone (2026-10-13): Opaque eligible runs join within each
  channel, and final canvas intersection retains original/restricted cubics,
  analytic circle arcs, winding, and sampled/translucent ordering. Canonical
  scenes, calibration, native raster output, and animation are unchanged.
  Guide SVG size falls 11.07% (38,031,722 → 33,822,698 bytes); vector-source SVG
  falls 26.02% (13,802,174 → 10,211,410 bytes). A straight-rail encoding that
  introduced three opaque faint-line spikes was removed. Thirteen integration
  tests, two clipping tests, actual app export/cancellation checks, final app/CLI
  builds, and renderer strict Clippy pass; parent inspected native/consumer
  artifacts. See the [final report](target/validation/ton018-svg-canvas-20261013/README.md).
  Two inherited Inkscape alpha-1→255 pixels remain at `(284,10)` and `(504,556)`;
  this is not exact consumer parity or full TON-018 acceptance. Prior private
  AT-SPI export-action evidence does not establish destination completion;
  the chooser did not appear and the private session is stopped.
- Calibration and personal Preset milestone, verified 2026-10-13: all three
  blockers are fixed with unchanged accuracy criteria. Convex insets no longer
  reappear after collapse; HexGrid fits within the new 144-knot bound; authored
  open-guide shape stays stable across Feature size with endpoint coverage.
  All fifteen personal Presets are converted, installed, and reopened, with
  exact original-byte backups and hash guards. Both-source/default/fine and
  representative native output checks pass (38 inspected renders across the
  repair/conversion stages). See the [conversion report](target/validation/ton018-personal-presets-20261013/README.md).
- Native regression milestone (2026-10-13): the partial-contour omission
  accounting defect is fixed. All 64 current default/fine family cases render;
  all 63 available prior outputs remain decoded-RGBA identical. The fine-vector
  Two-Guide Maze rounding loop is repaired with bounded checked retries and
  cooperative cancellation, retaining the existing fill-error budget. Focused
  tests, independent review and a clean app/CLI build pass. See the
  [native repair report](target/validation/ton018-checked-boolean-20261013/README.md)
  for resource-policy and synchronous-cancellation limits.
- SVG-derived PNG remains unselected: tests lose faint coverage and reveal an
  ordered SourceColorAlpha SVG composition discrepancy against required native
  linear-light equations. Supersampling does not resolve those differences.
  This consumer-fidelity finding remains open; see the
  [comparison report](target/validation/ton018-svg-raster-comparison-20261013/README.md).
- Built-in Preset milestone (2026-10-13): fourteen complete configurations are
  embedded with tidy names, existing confirmation and one-Undo application.
  Both-source tests and private GTK load/Undo/Redo pass; the reported rotated
  RGB photo renders at Feature .25. A disposable scatter-preview spacing defect
  is corrected. See the [report](target/validation/ton018-builtin-document-presets-20261013/README.md).
- The edited Poisson/weighted-Lloyd fit succeeds after 157.78 seconds in the
  diagnostic run; cumulative fitting work, not a union hang, explains the old
  120-second test failure. One combined union/intersection yields the same
  accepted profile and knot positions in an observed 118.03 seconds, with
  witnessed numerical equivalence and unchanged product accuracy/storage rules.
  Fresh structural calibration remains slow; internal Boolean cancellation
  gaps remain. Real GTK publication/history verification is recorded below. See the
  [calibration-union report](target/validation/ton018-calibration-union-20261013/README.md).
- Static GTK completion (2026-10-13): real edited Poisson/Lloyd Apply finishes
  in 116.24 seconds and publishes once. Fixed stale Review admission after
  companion attachment, recoverable action states, and neutral Wizard visibility
  across RGB/CMYK channels. Native history, both-source saved-companion reuse,
  Advanced and seed workflows pass; parent inspected final frames. Build/Clippy
  pass and private services are stopped. See the
  [report](target/validation/ton018-gtk-static-completion-20261013/README.md)
  for actual-fit versus saved-companion replay evidence and remaining limits.
- Animation response milestone (2026-10-13): static/animated Shape rotation and
  continuous frames retain exact saved assignments; named Recalibrate updates
  only its channel and All updates every channel plus its baseline. End displays
  the rotation warning, including full turns. Actual fitting/history/cancellation,
  persistence, both-source native output, focused tests and clean builds pass.
  Parent inspected the output and UI. Physical keyboard delivery remains
  unverified in the private headless seat. See the
  [report](target/validation/ton018-animation-response-20261013/README.md).
- Authored-mark SVG rejection repaired: exact point-collapsed contours now
  contribute no fill. Both failing projects export; positive subgrid/hole
  guards remain. Focused tests, builds and parent visual inspection pass;
  existing SVG-viewer/native color differences are unchanged. See the
  [report](target/validation/ton018-svg-empty-20261013/README.md).
- Large PNG headless foundation verified: exact pixel parity, atomic failure
  handling, and actual 16384×9312 photo export pass. The photo uses about 2 GiB
  peak process memory; bounded band buffers do not bound global geometry. See
  the [report](target/validation/ton018-png-streaming-20261013/README.md).
- Large PNG app/CLI integration verified: ordinary and zero-cleanup prepared
  still exports stream atomically, retaining exact requested dimensions/DPI.
  Preview and advisory Review limits no longer prevent valid large export;
  nonzero global cleanup retains an explicit 32,000,000-pixel limit. Invalid
  current fields cannot export a stale valid selection. Actual GTK photo and
  CLI 16384×9312 outputs fully decode, with 300-DPI metadata for prepared output.
  Both-source parity, cancellation, preview recovery, focused tests, independent
  review, strict Clippy and final app/CLI build pass. Parent inspected output
  and UI; private services are inactive. See the
  [report](target/validation/ton018-png-export-integration-20261013/README.md).
- Remaining: user review/acceptance of this completion round. Known limits are
  fresh structural calibration time, dense geometry memory, SVG-consumer color
  differences, documented cancellation gaps and private-seat keyboard evidence.
  Video/sequence policy and the nonzero-cleanup limit are unchanged.

## TON-019 — Preview panning stalls during drag and appears to discard rendered areas

- Status: Open; user-reported (2026-10-08), not independently reproduced;
  tracking only. No implementation is authorized by this entry.
- Report: Click-and-drag panning frequently stops to render newly exposed
  canvas areas, interrupting the active drag. Previously rendered areas that
  move off screen appear to require rendering again when the user pans back;
  the cache behavior and cause have not been verified.
- Expected behavior: Keep panning responsive throughout the drag. Defer new
  rendering for exposed areas until the user releases the drag, then render
  the final viewport. Reuse valid previously rendered areas when panning back
  instead of discarding them merely because they moved off screen. Cache reuse
  must respect document/render changes and zoom or resolution changes, with
  bounded memory use.
- Verification: At a zoom level with canvas overflow, drag continuously across
  uncached areas and confirm no new viewport render interrupts the gesture.
  Release and verify the final exposed area updates. Pan away and back over
  rendered areas without editing or changing zoom, and inspect render activity
  to verify reuse. Also check invalidation after edits or zoom changes, rapid
  successive drags, stale-result rejection, memory bounds, and both immutable
  sample artworks. Keep this interaction/cache issue distinct from TON-003's
  dense-Pattern construction and full-resolution preview latency.

## TON-020 — Creating a new grid after a customized Grid Voronoi exposes confusing retained guide state

- Status: Open; user-reported (2026-10-08), not independently reproduced;
  tracking only. No implementation is authorized by this entry.
- Reported workflow: Select **Grid Voronoi**, customize its two grid-defining
  curves into wave-shaped guides, and apply the Pattern. Later, reopen Pattern
  Wizard and try to create a new grid-family Pattern using only a single guide.
  The user reports that the new grid cannot be defined correctly: subsequent
  wizard pages have confusing layouts and unexpectedly many options, appearing
  to retain multiple stale grids from the earlier Pattern.
- Uncertainty: Retained wizard/grid state is the suspected cause, not a
  confirmed diagnosis. The intervening actions implied by "after a while" and
  the exact affected pages/options have not yet been captured.
- Expected behavior: Creating a new Pattern starts a coherent draft for the
  chosen family. A single-guide configuration is straightforward to define;
  prior grids, curve edits, and guide collections do not silently populate or
  complicate it. Subsequent pages reflect the active configuration and expose
  relevant options clearly, while preserving access to supported capabilities.
  Customizing the existing Pattern remains distinct from creating a new one.
- Verification when authorized: Reproduce the two-wave-guide Grid Voronoi
  workflow, apply it, reopen the wizard, and create a single-guide Pattern.
  Inspect the draft's guide/curve identities and counts, generated page route,
  visible controls, preview, and applied result for unintended retained state.
  Compare with creating the same Pattern from a fresh document; also check
  Back/Next and Cancel/reopen behavior without changing the main document
  before Apply. Record the exact intervening actions if immediate reopening
  does not reproduce the report. Keep this separate from TON-017's built-in
  direct-Apply and gallery-selection fixes.

## TON-021 — Reduce off-canvas geometry construction while preserving complete canvas coverage

- Status: Open; user-requested (2026-10-13); tracking only. No implementation
  is authorized by this entry.
- Report: Current SVG inspection exposes substantial geometry beyond the
  visible canvas. The engine may construct more off-canvas geometry than is
  necessary, increasing geometry counts, compute work, and memory use. This
  is a potential optimization, not a confirmed algorithm diagnosis. Historical
  expansion beyond the canvas ensured complete coverage and must remain
  sufficient. The user's manual SVG intersection demonstrates output-size
  savings; it does not establish an engine runtime or memory improvement.
- Expected behavior when authorized: Generate only the geometry required for
  complete canvas coverage, including edges and corners. Bounds must account
  for rotation, stretch, offsets, mark sizes, stroke widths, and the required
  neighbor/topology halo. Preserve authored spacing and density, seed
  statistics, calibration, determinism, and complete off-canvas structural
  neighborhoods wherever visible output depends on them.
- Verification when authorized: Compare generated geometry counts, elapsed
  time, and peak RSS against the existing implementation, alongside native
  output parity. Exercise default and fine Feature sizes with both immutable
  sample artworks, multiple Pattern families, and rotated, stretched, offset,
  large-mark/stroke, and edge/corner boundary cases. Reduced work is acceptable
  only with preserved complete coverage and current geometric semantics.
- Scope: Keep this future construction optimization separate from TON-003's
  general preview latency and TON-018's current final-SVG clipping. This entry
  changes neither engine construction nor canonical geometry.
