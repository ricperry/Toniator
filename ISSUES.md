# Toniator issues

This ledger records confirmed limitations and deferred reports outside the
current gate. Closing an item requires a reproducer or verification evidence;
an entry does not authorize a later stage or change an accepted contract.

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
