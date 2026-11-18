# Creative Suite interface conventions

Toniator, Chromiator and Bezziator share a native GTK4 presentation baseline.
Each app vendors an identical `creative-suite.css`; builds remain independent.
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

The canonical source is Toniator's
`crates/toniator-app/resources/creative-suite.css`. Review a common change there,
copy its exact bytes to the existing `creative-suite.css` source file in
Chromiator and Bezziator, and run `python3 scripts/check_suite_style.py --check`.
The command only reads files and prints the shared SHA-256. Missing sibling
checkouts are explicit skips; present differing or missing source copies fail.
Application builds embed their own copy and never read sibling repositories.
