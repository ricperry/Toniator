# Toniator private Wayland debugging

## Host prerequisites

Fedora packages:

```bash
sudo dnf install sway wayvnc grim at-spi2-core python3-pyatspi python3-dogtail
```

Install VNCDoTool in an isolated tool environment if Fedora does not provide it:

```bash
uv tool install vncdotool
```

The semantic scripts use `pyatspi` directly for deterministic queries and
mutations. Dogtail remains useful for ad hoc exploration. WayVNC supplies
compositor-native input, while grim captures the Sway output through wlroots'
screencopy protocol.

The VNC helpers expose `click`, `move`, and fixed-step `drag` primitives plus
`key KEY` for special keys such as `enter`, `escape` (normalized to VNC's
`esc`), and `left`, `right`, `up`, or `down`. `drag` always sends move, press, eight bounded interpolation
steps by default, then release; it remains loopback-only through the shared
helper guard.

Run `scripts/preflight` after installation. It checks Sway, WayVNC, grim,
AT-SPI, VNCDoTool, the Toniator debug binary, and local runtime requirements
without installing or changing system packages.

## Isolation and security

`session-start` hardcodes `WLR_BACKENDS=headless`,
`WLR_LIBINPUT_NO_DEVICES=1`, and one `HEADLESS-1` output. It gives Sway a
private mode-0700 runtime directory, a private D-Bus session, a private AT-SPI
bus, a generated minimal config, and no Xwayland. It never invokes a display
manager or reads the user's Sway configuration.

WayVNC reads a generated config with `enable_auth=false` and binds explicitly
to `127.0.0.1`. This is acceptable only for this local development harness.
The scripts refuse a non-loopback VNC target. They do not read a login password
or any `TONIATOR_VNC_PASSWORD` value.

Sway, WayVNC, D-Bus, AT-SPI, and Toniator run as transient systemd user
services. `session-stop` targets only the recorded transient units. Never run
plain `sway` to exercise this skill; doing so may replace the active graphical
session depending on how it is launched.

## Runtime layout

Session state defaults to:

```text
.codex-work/gtk-wayland-debug/
├── session.env
├── app.pid
├── app.unit
├── active-evidence
├── dbus-<run-id>.log
├── atspi-<run-id>.log
├── sway-<run-id>.conf
├── sway-<run-id>.log
├── wayvnc-<run-id>.conf
└── wayvnc-<run-id>.log
```

The live Wayland, Sway IPC, WayVNC control, and D-Bus sockets use a short
mode-0700 directory below the host's existing `XDG_RUNTIME_DIR`. The recorded
session environment contains no credentials.

Each app launch creates:

```text
.codex-work/evidence/ui-run-<run-id>/
├── app.stdout.log
├── app.stderr.log
├── accessibility-current.txt
├── current.png
├── environment.txt
├── session.txt
└── report.md
```

Set `TONIATOR_WAYLAND_STATE_DIR` or `TONIATOR_UI_EVIDENCE_ROOT` to override
these roots. Set `TONIATOR_VNC_PORT`, `TONIATOR_WAYLAND_WIDTH`, and
`TONIATOR_WAYLAND_HEIGHT` before `session-start` to change the defaults of
5901 and 1440x1000.

The app uses GTK's Cairo renderer by default because the compositor session is
hardware-free. This affects GTK shell presentation, not Toniator's canonical
preview/export generation. Set `TONIATOR_GSK_RENDERER=opengl` or `vulkan` only
for a focused backend-specific reproduction, and record that override.

## Semantic selection

`ui-find` and `ui-action` match accessible names case-insensitively by default.
Use `--exact` for an exact name and `--role` to constrain the normalized AT-SPI
role. If more than one node matches, `ui-action` exits 4 and prints candidates;
choose one with `--index` rather than guessing.

Examples:

```bash
scripts/ui-find 'Density across X' --application Toniator --role text --exact --json
scripts/ui-action 'Density across X' --application Toniator --role text --exact --set-text 60 --commit
scripts/ui-action Export --application Toniator --activate
scripts/ui-action Pattern --application Toniator --actions
```

AT-SPI actions and values are authoritative only for the exposed widget state.
In a device-free private seat, disconnecting a one-shot WayVNC client can clear
keyboard focus. A transient AT-SPI focus event does not prove focus survives
that disconnect. If dropdown selection fails despite correct semantic targets,
retain one loopback VNC connection across focus, popup opening, and the complete
Home/Down/Enter sequence; inspect the actual selected text afterward. The same
retained connection is needed when capturing a pointer-hover tooltip. This is
a harness input-lifetime issue, not grounds for coordinate-based widget clicks.
They do not prove that a typed command committed, persistence changed, a cache
invalidated, or the canvas rerendered. Verify those boundaries separately.

## Diagnostic localization

| Semantic state | Document/CLI state | Pixels/output | Likely boundary |
| --- | --- | --- | --- |
| unchanged | unchanged | unchanged | input, focus, or widget action |
| changed | unchanged | unchanged | GTK binding or command dispatch |
| changed | changed | unchanged | invalidation, scheduling, evaluation, or preview acceptance |
| changed | changed | changed preview only | final export consumer |
| changed | changed | changed output only | preview target or presentation path |

Use `WAYLAND_DEBUG=client scripts/app-restart [PATH]` only around a focused
protocol reproduction. Preserve `app.stderr.log` because it will contain both
Wayland trace data and Rust/GLib diagnostics.

Sway is a real Wayland compositor but not Mutter. Reproduce GNOME Shell,
Mutter, portal, global-shortcut, or compositor-policy defects in GNOME after
the isolated run narrows the problem.

For native GTK file-chooser workflow tests, the private app wrapper may set
`GDK_DEBUG=no-portals` when the installed GTK supports it. Verify that the
chooser appears in the private compositor before proceeding. This test-only
fallback exercises the native chooser and application callbacks; it does not
validate the host GNOME portal. If a chooser escapes to the user's desktop,
stop that run immediately.
