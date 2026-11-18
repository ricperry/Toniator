# Accessibility

Toniator targets native GTK4 keyboard and assistive-technology support through
GtkAccessible and AT-SPI2. WCAG 2.2 AA, interpreted for desktop software through
WCAG2ICT, informs contrast, keyboard access, focus visibility, names, state and
error handling. This is an engineering target, not a certification or a claim
of complete conformance.

The implementation follows the [GTK accessibility guidance](https://docs.gtk.org/gtk4/section-accessibility.html),
[WCAG 2.2](https://www.w3.org/TR/WCAG22/) and
[WCAG2ICT](https://www.w3.org/TR/wcag2ict-22/).

## Supported interaction

Native controls expose roles, names, enabled state, values, selections and
supported actions. Labels and tooltips use the same product vocabulary as the
visible interface. Channel selection includes text as well as color. Hidden
sections leave keyboard traversal; disabled actions retain truthful applicability.
The system theme supplies fonts, focus indicators and high-contrast controls.

The [keyboard table](INTERFACE_CONVENTIONS.md#keyboard-conventions) and
**Main Menu → Help → Keyboard Shortcuts** describe navigation. F6 moves between
the Canvas and Settings. Escape dismisses information windows and
cancels the unsaved-work prompt. Native text controls keep text editing; the
Canvas provides focused arrow-key panning and keyboard zoom/Fit.

In Toniator 0.4.7, **Fast preview** follows Zoom/Fit in the canvas
footer. Its visible label identifies the native GTK switch, which exposes checked
state and a toggle action. Tab/Shift+Tab reaches it and Space toggles it; no new
global shortcut is assigned. Source view and absent artwork/document disable the
switch truthfully while retaining its checked preference. **Accepted preview
mode** exposes the mode of accepted pixels, remaining blank outside Preview.
These names and states come from the visible GTK controls, not a separate
accessibility model.

## Limits and testing

The artwork canvas is a spatial visual surface. With Canvas focused, arrow keys
pan zoomed artwork without changing the document. Generated dots, curves and
other marks are not exposed as thousands of separate accessibility nodes.
Preview identity and controls are exposed, but screen-reader users cannot
inspect the full rendered artwork through AT-SPI. Canvas panning is available with arrow keys after F6 focuses Canvas; detailed
visual judgments still require visual inspection.

Private automated Sway/Wayland checks inspect AT-SPI state/actions, native
keyboard input, theme/font variants, screenshots and logs. Such checks do not
establish screen-reader-user acceptance, GNOME Shell/Mutter behavior, desktop
portal behavior or complete keyboard coverage of every application dialog.
File choosers and external link handling also depend on the installed desktop.

Development verification covers Fast preview label relations, semantic
toggle/readback, native focus/Space and Tab/Shift+Tab checks, affected neighboring
controls, screenshots and logs.
High-contrast narrow-layout checks include a measured 24px CSS typography stress
case; this does not establish GNOME text-scaling or human screen-reader acceptance.

Report an accessibility issue with the app version, installation method,
distribution, desktop, assistive technology and steps to reproduce. Include
which control had focus and what happened after the key or action.
