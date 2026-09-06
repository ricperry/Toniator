# Startup screen follow-up

The original startup follow-up to Gate 21B-4 was **Complete and user-accepted** on
2026-09-04 at implementation checkpoint
`4ed29d4f5e3733ab487ae433139b302c27a80c44`, recorded in `ProgressTracker.md`. Acceptance
reuses the verified implementation evidence below; no new application run is
claimed for that documentation/Git closeout.

The user-authorized 2026-09-06 revision separates welcome from the editor canvas.
The user confirms both the fixed splash and the subsequent main-window sizing
correction. The sizing defect came from a long personal-library warning wrapping
inside the narrow canvas toolbar, forcing an enormous minimum window height.
The user includes these corrections in the combined acceptance on 2026-09-06;
`ProgressTracker.md` records the resulting source checkpoint.

## Behavior

Launching without a file shows a separate, nonmodal, nonresizable welcome window
above the main editor, based on
`assets/Stage21D_Mockup/SplashMockup.png`. Its unchanged banner artwork is clipped
at display time; the cards, text, and controls are native GTK widgets. The left
card has one **Start New Project** call-to-action and a hint explaining that it
also opens existing projects. System light/dark colors are inherited, and the
cards stack below 800 logical pixels. Welcome is 880×680 logical pixels, reduced
when needed to leave 96 logical pixels of margin within the smallest attached
monitor. Overflow remains scrollable. It is presented after the editor's first
frame to prevent asynchronous window mapping from putting the editor on top.
Clicking the editor behind welcome, pressing Escape, or using welcome's X creates
an empty Untitled document through the existing New command. The first background
click is consumed. Chooser focus changes and pending file operations do not dismiss
welcome. Open, sequence import, and Help dialogs use welcome as their parent while
it is visible. A dismissed welcome window is destroyed; Close creates a fresh
toplevel around the detached startup content.

Recent Files holds up to 12 successfully opened source images or projects and
successfully saved projects. The list scrolls within a fixed 280-logical-pixel
viewport, so additional entries do not increase the startup window height.
The heading and Clear List button remain outside that scrolling area.
Main-editor messages occupy a separate full-width scrolling row capped at 96
logical pixels. Complete selectable warning text and the Dismiss message button
remain available without allowing a long warning to enlarge the main window.
Each row shows the filename, folder, and last-used
time; the accessible name identifies the file and its description contains the
full path. Failed loads keep startup usable and identify the unavailable file.
Clear List removes metadata only, leaving source/project files untouched.

History lives at `$XDG_STATE_HOME/Toniator/recent-files.json`, defaulting to
`~/.local/state/Toniator/recent-files.json`. The IO crate owns bounded, versioned
metadata and atomic writes. Metadata failure does not turn a successful document
open/save into a failed operation. Document contents and formats are unchanged.

Close (Ctrl+W) resolves Save/Discard/Cancel, then clears the workspace and returns
to startup. Exit (Ctrl+Q) and the main window X use the same decision and quit the app.
Cancelled dialogs and failed saves retain the document. Competing lifecycle
actions are disabled during I/O, the save decision, and deferred quit. Closing
detaches private editors and cancels preview work; initial preview waits for
workspace allocation before submitting. Repeated activation presents the existing
window. File forwarding retains the existing guarded lifecycle route.

## Verification

The separate-window checks are recorded in
`.codex-work/evidence/ui-run-20260906-104956-51999/` (stacking, one-click dismissal,
Open/Cancel and welcome X), `ui-run-20260906-105204-52932/` (Escape and Ctrl+W),
and `ui-run-20260906-105856-55184/` (fresh welcome after recent Open and Close/Discard).
The release build, strict app Clippy, and focused logical-size bounds test pass.
The initial hide-and-reuse implementation encountered a GTK AT-SPI enumeration
crash while a window disappeared; dismissed toplevels are now destroyed instead.
The generic one-shot key helper sent keys before the private keyboard attached;
the retained-keyboard script supplies the successful Escape/Ctrl+W evidence.
All these runs use private Sway/Cairo at 200% scale; desktop confirmation comes
from the user's subsequent report, not from the private compositor.

The user's GNOME Wayland trace requests a minimum height of 13,731 logical
pixels despite a 1920×1048 usable desktop. A copied personal library reproduces
the long-warning defect in `ui-run-20260906-110749-61056/`, requesting 19,725
pixels and failing Cairo surface creation. With the bounded message row, the
same warning profile in `ui-run-20260906-110919-61556/` has a 480×360 minimum
and fits an 800×600 logical desktop. Its screenshot is inspected and the
stage-owned `assert-window-bounds.py` passes. The user reports “That fixed it.”
Print Screen recovery has not been separately confirmed. Renderer defaults and
the user's personal library files are unchanged.

The 2026-09-06 scrolling follow-up is verified in the rebuilt release executable
with Cairo under private Sway at 200% scaling. All 12 entries remain accessible;
keyboard navigation reveals the seventh and final rows while the startup window
stays bounded. Screenshots and semantic readbacks are in
`.codex-work/evidence/ui-run-20260906-100531-45599/`; strict app Clippy passes.
This earlier scrolling check alone did not establish resolution of the later
diagnosed warning-layout defect or the Print Screen report.

- Three focused IO tests cover MRU order, deduplication, bounds, round-trip,
  malformed/obsolete metadata, XDG fallback, and metadata-only clearing.
- Focused app tests cover save-decision routing, deferred window-close state,
  lifecycle names/filters, and the registered splash resource. App/IO strict
  Clippy and the architecture validator pass.
- Private GTK checks exercise native Open, recent reopening, dirty Close Cancel
  and Discard, Save before Close, cancelled Save As, project-save recents, Exit
  cancellation, and saving through window X. Recent persistence, missing-file
  recovery, and Clear List are checked against files and metadata.
- Native roles, names, enabled states, actions, and readback cover the startup
  button, recent rows, Clear List, and existing save/close controls. Keyboard
  Ctrl+W/Ctrl+Q exercise the exact lifecycle actions. Light/dark, wide/narrow,
  save prompts, and final startup screenshots are inspected.

Final single-button, missing-file, and cleared-list screenshots are in
`.codex-work/evidence/ui-run-20260904-213613-236312/`; that run has no matching
panic/error/critical/warning diagnostics in app stderr. Ctrl+Q on empty startup
exits the process. The generic shortcut script's subsequent controls readback
fails because the application has exited; process absence confirms the result.

Scripts and readbacks are in `target/validation/startup-screen/`; screenshots and
logs are in `.codex-work/evidence/ui-run-20260904-*`. Earlier hamburger focus
automation failed and emitted a GTK focus assertion; those attempts are retained
as failed harness evidence, not successful menu interaction. Native file dialogs
run with test-only `GDK_DEBUG=no-portals` in private Sway. This is automated
Sway/wlroots evidence, not human review or GNOME/Mutter/portal acceptance.

The separately requested **Test Pattern - Diagonal Dots** remains in the actual
default personal library, `~/.local/share/Toniator/presets/`, with ID
`user-00000000-0000-4000-88d2-47251ec575f6`. Real Save as New, reopened confirmed
update, and listing after restart were verified before the isolated startup tests.
