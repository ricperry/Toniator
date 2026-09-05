# Toniator issues

This ledger records confirmed limitations and deferred reports outside the
current gate. Closing an item requires a reproducer or verification evidence;
an entry does not authorize a later stage or change an accepted contract.

## TON-004 — A second launch does not forward a file to the running app

- Status: Open; application activation follow-up.
- Evidence: The app parses its initial path locally and registers activation,
  without a GApplication file-open handler. A second launch presents the existing
  window, but its requested file is not forwarded to that window.
- Next step: Route forwarded files through the existing dirty-document guard
  and asynchronous load coordinator. Verify cancellation preserves the open
  document and successful loading updates Recent Files.

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
