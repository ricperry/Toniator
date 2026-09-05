# Startup screen follow-up

User-authorized follow-up to Gate 21B-4, **Complete and user-accepted** on
2026-09-04 at implementation checkpoint
`4ed29d4f5e3733ab487ae433139b302c27a80c44`, recorded in `ProgressTracker.md`. Acceptance
reuses the verified implementation evidence below; no new application run is
claimed for documentation/Git closeout. Document Presets remain planned Gate 21B-5.

## Behavior

Launching without a file shows the startup screen based on
`assets/Stage21D_Mockup/SplashMockup.png`. Its unchanged banner artwork is clipped
at display time; the cards, text, and controls are native GTK widgets. The left
card has one **Start New Project** call-to-action and a hint explaining that it
also opens existing projects. System light/dark colors are inherited, and the
cards stack below 800 logical pixels.

Recent Files holds up to 12 successfully opened source images or projects and
successfully saved projects. Each row shows the filename, folder, and last-used
time; the accessible name identifies the file and its description contains the
full path. Failed loads keep startup usable and identify the unavailable file.
Clear List removes metadata only, leaving source/project files untouched.

History lives at `$XDG_STATE_HOME/Toniator/recent-files.json`, defaulting to
`~/.local/state/Toniator/recent-files.json`. The IO crate owns bounded, versioned
metadata and atomic writes. Metadata failure does not turn a successful document
open/save into a failed operation. Document contents and formats are unchanged.

Close (Ctrl+W) resolves Save/Discard/Cancel, then clears the workspace and returns
to startup. Exit (Ctrl+Q) and window X use the same decision and quit the app.
Cancelled dialogs and failed saves retain the document. Competing lifecycle
actions are disabled during I/O, the save decision, and deferred quit. Closing
detaches private editors and cancels preview work; initial preview waits for
workspace allocation before submitting. Repeated activation presents the existing
window. Forwarding a second process's requested file is tracked as TON-004.

## Verification

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
