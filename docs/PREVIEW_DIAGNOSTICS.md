# Preview recovery and local diagnostics

This guide describes the user-accepted preview reliability and automatic local
logging changes for the replacement Toniator 0.4.5 prerelease on 2026-10-18.
Release attachments identify the exact source checkpoint and package checksums. See the
[0.4.5 release notes](releases/v0.4.5.md) for the combined scope.

## When a preview fails

The main canvas shows **Preview unavailable** and keeps the last accepted pixels.
**Showing the last preview.** means those pixels may precede the current settings;
**No preview yet.** means no preview has been accepted. Save and other operation
errors retain their own identity.

- **Retry preview** admits one attempt for the current failed frame and target.
  It recreates an affected unavailable main-preview worker when necessary.
  Another failure remains settled until an explicit retry or a real
  document/frame/view change; there is no automatic same-intent retry loop.
- **Details…** opens **Preview failure details** with cause-specific guidance and
  selectable, read-only **Technical details**. It is also available for failures
  in Pattern Wizard, Pattern Editor and Advanced Settings.
- **Copy details** copies the captured report and shows **Details copied.**
  **Close** or Escape closes the modal and restores focus to the invoking button.
- **Dismiss preview error** hides the main overlay. It preserves failed intent
  and accepted pixels and schedules no work.

Private previews have no new Retry button. Close and reopen a private preview
when its worker has stopped. In Advanced Settings, source preparation belongs
to the dialog lifetime: after a source-preparation failure, close and reopen it
to rerun preparation; editing settings does not rerun that step.

No recovery action automatically changes artwork, calibration, precision or
file format. Optional reductions in Details depend on the actual failure:

| Failure cause | Suggested action and scope |
| --- | --- |
| Geometry memory | Increasing Feature size may reduce generated geometry with a coarser appearance. It does not simplify authored guides. For complex authored guides, simplify a copy or free memory. |
| Source memory | A smaller source-artwork copy or more free memory may help. Fixed private source proxies suggest freeing memory; Advanced Settings explicitly requires reopening for preparation. |
| Preview-pixel memory | A smaller main preview window may reduce viewport pixels. Private previews use fixed targets, so their guidance is to free memory. |
| Worker stopped | Retry recreates the affected main worker; close/reopen recreates a private worker. |
| Software rendering error | Keep the technical report for diagnosis. Count, addressability and numerical failures are software failures, without artwork-reduction advice. |

## Local failure reports

Every genuine current terminal preview failure automatically captures an
immutable report and attempts to append one schema-1 NDJSON record to `preview-failures.log`, beside the platform's
default Recent state. On Linux this is
`$XDG_STATE_HOME/Toniator/preview-failures.log` when the state base is absolute,
otherwise `$HOME/.local/state/Toniator/preview-failures.log` with an absolute HOME.
Windows resolves the base through GLib's user state directory. The exact path
and write outcome appear in Details where that route is available. Logging does
not depend on opening Details or on an OOM classification.

The primary log is bounded to 2 MiB, with two rotated backups (`.1` and `.2`).
Each entry is bounded to 32 KiB and serialized context to 8 KiB. Existing
nonregular destinations and symlinks are rejected before log/rotation mutation;
new Unix log files use mode `0600`. Logging failure is nonfatal and appears in
the immutable report as **Could not write local log** or an unavailable path.
Copy still supplies the captured technical report.

Records include app version, platform/architecture, timestamp, diagnostic
origin/code/message and bounded context for the actual preview surface, source
and request. Captured metadata excludes source pixels/bytes/hashes, project
names/paths and the full recipe. This is local logging, with no telemetry.
Raw producer messages and the displayed log path may contain local paths:
review the copied report before sharing it.

Coverage includes main and private previews, startup failures before a private
surface exists, active Print/PNG preview failures, and positive checked viewport
refusals. Print has no new Details control. Requested startup failures can be
recorded even when no preview surface opens; main bridge startup failure is
recorded when a preview is actually requested.

An unsaved/dirty document still logs a genuine current failure. Actual owned
cancellation or supersession after an edit/restart, stale or closed work, and
absent, invalid-form or unallocated intent remain quiet. Details, Copy, Close,
Dismiss and presentation refresh do not create entries; a failed explicit retry
may create one new attempt record. Read-only target projection does not log.
A positive viewport refusal is recorded once for the actual frame/allocation;
returning to the same valid size admits a fresh request.

Print's existing retained-preview guard is 256 MiB. A valid output selection
that exceeds it records a software capacity refusal once per consumed intent,
while preserving “Full-size preview unavailable; export retains the requested
dimensions and DPI.” Valid dimensions recover the preview. This guard is not
OOM, and neither its bound nor the main preview's existing pixel guard changes.
A process killed by the OS for OOM, or a dependency
allocator abort, may terminate before any report or log can be written.

## Developer contract and verified scope

Typed producer diagnostics remain authoritative before presentation. Memory
guidance is restricted to exact audited actual reserve-failure codes in
[`preview_diagnostics.rs`](../crates/toniator-app/src/preview_diagnostics.rs).
Do not classify generic historical allocation aliases, count overflow,
addressability, numeric errors or ticket exhaustion as OOM. The shared GTK
projection is in
[`preview_diagnostics_view.rs`](../crates/toniator-app/src/preview_diagnostics_view.rs);
[shared native GTK controls](../crates/toniator-app/src/preview_diagnostics_view.rs)
implement its selectable report, Copy feedback and modal focus behavior.

Normal intersection defaults now use checked, fallible, cancellable growth in
place of fixed artistic pair/result ceilings. Explicit finite caller policy and
its existing family-cache admission remain intact. Existing predicates,
tolerances, deterministic subdivision depth, authored paths and stable contact
order remain authoritative. Two natural non-OOM reproductions are corrected:
a 513-segment authored path with one crossing previously exceeded 262,144 pairs,
and 10,000 true contacts previously exceeded the 4,096-result ceiling. This is
bounded evidence, not exhaustive proof for every possible valid recipe.

The prior reliability stage verified 15 focused core and 17 focused app checks, strict named
Clippy and release builds, both immutable source fixtures in all compatible
color modes, exact cold/cached output, 14 unchanged small control artifacts,
six unchanged original native artifacts and bit-exact decoded source channels.
Its private GTK checks cover all four Details routes, clipboard readback, log IO
failure, explicit retry, dismissal, focus/Escape, and narrow large-text scrolling.
Manufactured GTK routing failures are explicitly marked
`injected_routing_witness`; they prove presentation/routing, not natural OOM.

Those prior 480×360 HighContrast/Sans 20 checks establish that the three main
actions are reachable through focus scrolling and modal Copy/Close stay pinned.
The preexisting main-window minimum width still exceeds that output: its canvas
allocation was 545×233 and header/status/short-message content could clip.
Automated private Sway evidence does not establish human assistive-technology,
GNOME/Mutter, portal, native Windows or user acceptance.

The automatic-logging follow-up passes 19 focused/current-foundation tests,
strict named Clippy and a release build. Twenty useful private GTK runs (plus
one excluded selector probe) capture 18 automatic records: 17 marked injected
routing witnesses and one natural Print capacity refusal at 8193×8192. An actual
unsaved CMYK revision logs its injected current failure, accepts Retry without
saving, and settles for 133.6 seconds without extra events or records. Normal
PNG/SVG fixtures accept without logs. Positive huge-viewport refusal and
same-valid-target recovery have source/unit evidence, not a huge-viewport GTK
witness. No natural OOM or OS-kill capture is claimed.

The follow-up adds no controls or layout. Prior output/decoded-source evidence,
shared Details view and accepted overlay builder remain exact by audit; the
prior keyboard/clipboard/narrow checks are retained evidence rather than fresh
checks of every interaction. All follow-up owned sessions are stopped.

The focused verification above covers the accepted implementation. Internal raw
reports, runtime manifests and source audits remain local development evidence;
they are not required to use the application. Public implementation references
are the [checked intersection kernel](../crates/toniator-geometry/src/curves/intersections.rs),
[capacity regression checks](../crates/toniator-geometry/tests/intersection_capacity.rs),
[engine reliability checks](../crates/toniator-engine/tests/preview_reliability.rs),
[diagnostic/logging authority](../crates/toniator-app/src/preview_diagnostics.rs)
and [main preview lifecycle](../crates/toniator-app/src/temporal_preview.rs).
