---
name: gtk-wayland-debug
description: Launch and exercise Toniator inside a private headless Sway Wayland session, inspect GTK semantics through AT-SPI, drive input through loopback-only WayVNC, capture through grim, and collect correlated screenshots, accessibility state, process logs, backtraces, protocol traces, and output artifacts. Use for Toniator GTK implementation, regression reproduction, UI-state diagnosis, automated interaction checks, accessibility inspection, and evidence gathering when the real GNOME desktop should remain untouched.
---

# GTK Wayland Debug

Use only the private headless Sway session created by this skill. Never point
these scripts at the user's active GNOME session. Keep WayVNC bound to
`127.0.0.1`, never enable authentication or read login credentials, and stop if
a requested configuration would expose the session beyond loopback.

Read `references/gtk-debugging.md` before first use on a host, after a package
upgrade, or when authentication, AT-SPI, focus, popovers, or coordinates behave
unexpectedly.

## Run the workflow

1. Run `scripts/preflight`. Resolve required dependencies before starting.
2. Build only the exact Toniator target required by the assigned stage. The
   skill never chooses stage scope or performs a broad build implicitly.
3. Run `scripts/session-start`, then `scripts/app-start [PATH]`. The app script
   defaults to `target/debug/toniator-app`, forces GTK's Wayland and AT-SPI
   backends, enables full Rust backtraces, and creates a fresh evidence run.
4. Prefer semantic operations:
   - `scripts/ui-tree --application Toniator`
   - `scripts/ui-find 'Density across X' --application Toniator --role text --exact`
   - `scripts/ui-action 'Density across X' --role text --exact --set-text 60 --commit`
   - `scripts/ui-action Export --application Toniator --activate`
5. Use VNC primitives when the accessibility interface cannot express an
   interaction. Use grim screenshots to verify the rendered result:
   - `scripts/screenshot 01-launch.png`
   - `scripts/click 620 410`
   - `scripts/type 'example text'`
   - `scripts/move 620 410`, `scripts/drag 620 410 700 460`, and
     `scripts/key enter` for deterministic pointer motion, drags, and special keys.
6. After a code change, rebuild the bounded target and run
   `scripts/app-restart [PATH]`. Reproduce the same semantic and visual sequence
   rather than inventing a different check.
7. Run `scripts/evidence-bundle` and inspect the resulting directory. Correlate
   accessibility values, screenshots/raw exports, document or CLI state, and
   logs before localizing a defect.
8. Run `scripts/session-stop` at handoff, on failure, or before changing session
   geometry or port.

## Preserve evidence integrity

Treat AT-SPI state as semantic evidence, grim screenshots as visual evidence,
and logs/backtraces/protocol traces as diagnostic evidence. A conclusion should
name which levels support it and any disagreement between them.

Keep raw screenshots and native RGBA/export artifacts unchanged. Do not flatten
alpha, add a checkerboard, or substitute a viewer composite. Put stage-owned
derived artifacts under the active `target/validation/stage-*` directory; the
skill's transient UI evidence belongs under `.codex-work/evidence/`.

Automated Sway evidence is not human manual visual, keyboard, focus,
assistive-technology, GNOME Shell, or Mutter acceptance. Report that limitation
explicitly. Do not advance `ProgressTracker.md`, claim user acceptance, or
commit/push because an automated run passed.

Enable `WAYLAND_DEBUG=client` only for a focused reproduction because it is
verbose and shares the app's stderr log. Use screenshots to verify presentation,
not to infer authoritative values that AT-SPI or document state can expose.
