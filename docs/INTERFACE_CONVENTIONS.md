# Creatiator Suite interface conventions

Toniator, Chromiator and Bezziator belong to Creatiator Suite and share the local
`creatiator` Rust library. Each source checkout carries a synchronized copy in
`shared/creatiator`, so builds require no sibling checkout or unpublished remote
dependency. Its default feature set contains only double-precision sRGB transfer
functions; only `toniator-app` enables the optional `gtk` feature for the system
theme bridge and embedded `resources/creative-suite.css`.
The theme owns colors, fonts, high contrast and visible keyboard focus. Local
CSS follows the shared baseline and retains application-specific canvas colors
and controls. The common classes cover headers, inspectors, section headings,
status bars, dialog content/actions and icon buttons; they do not redefine GTK
widget semantics.

The header places New, Open, Save, Undo and Redo on the left, and Settings and
Main Menu on the right. Toniator retains New options for its artwork, image
sequence and document Preset workflows. Symbolic GTK icons keep native theme
and high-contrast rendering. Tooltips and accessible names explain icon actions.

Main Menu groups File, Edit, View, Tools and Help. File keeps New/Open,
Save/Save As, application-specific Preset/import/export operations, Close and
Quit in that order. Edit exposes Undo/Redo; View exposes Settings/Fit/zoom.
Tools retains Toniator's animation settings. Help includes Keyboard Shortcuts,
local Help and feedback, and About. F1 opens local information; external links
open only when activated.

## Keyboard conventions

| Action | Shortcut |
| --- | --- |
| New / Open / Save | Ctrl+N / Ctrl+O / Ctrl+S |
| Save As / Export image | Ctrl+Shift+S / Ctrl+E |
| Close / Quit | Ctrl+W / Ctrl+Q |
| Undo / Redo | Ctrl+Z / Ctrl+Shift+Z or Ctrl+Y |
| Main Menu / Settings | F10 / Ctrl+, |
| Canvas / Settings | F6 |
| Fit / Zoom in / Zoom out | Ctrl+0 / Ctrl++ / Ctrl+- |
| Pan zoomed artwork (Canvas focused) | Arrow keys |
| Local help / Keyboard Shortcuts | F1 / Ctrl+? |
| Next / previous control | Tab / Shift+Tab |
| Cancel or dismiss current dialog | Escape |

Native text fields retain their editing and history behavior. The document
history remains separate from uncommitted field text. No unmodified global
canvas shortcut overrides typing. F6 targets Canvas or Settings. With Canvas focused, arrow keys pan zoomed artwork
through the same scroll adjustments used by pointer input.

The Toniator **Fast preview** switch after Zoom/Fit uses native
Tab/Shift+Tab navigation and Space to toggle. It has no dedicated shortcut; its
requested state and the adjacent accepted-pixel mode have distinct meanings.
See [preview modes](USING_TONIATOR.md#fast-preview).

## Dialogs

Dialogs use native GTK controls, transient parenting and modal interaction where
a pending decision requires it. Information windows offer Close and Escape;
the About and Keyboard Shortcuts windows restore the prior live focus widget.
The unsaved-work decision defaults to Cancel; Save and Discard retain explicit
buttons. Existing Apply/Cancel forms retain their authored-state boundaries.
File and color choosers follow the installed GTK/desktop portal presentation.
Wrapping and scrolling keep long content available at larger fonts. Individual
workflow dialogs keep their existing layouts rather than forcing identical
contents across different tools.

See [accessibility support and verification limits](ACCESSIBILITY.md).

## Maintaining the shared stylesheet

The suite's canonical source is
`../CreatiatorSuite/creatiator/resources/creative-suite.css`; the local standalone
copy is `shared/creatiator/resources/creative-suite.css`. Coordinate changes in
the suite source, synchronize the vendored crate, and run
`python3 scripts/check_suite_style.py --check`. The command compares fixed
source locations and prints the shared SHA-256. Missing sibling checkouts are
explicit skips; present differing or missing source copies fail.

Application builds embed their local crate's baseline at application priority
minus one. Toniator's own resource stylesheet follows at application priority.
The library reads GNOME preferences and, inside Flatpak/AppImage, the Settings
portal without changing desktop settings. Core color transfers remain GTK-free;
rendering keeps its explicit output-boundary clipping. UI templates, document
state, history, commands and file formats remain application-owned.
