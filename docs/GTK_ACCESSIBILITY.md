# GTK accessibility and semantic automation

`toniator-app` exposes its real GTK4 widgets to AT-SPI. This is a projection of
existing UI and descriptor authority, never a second document or inspector
model. GTK widgets remain the interaction surface; the domain remains the
authority for validation, commands, persistence, evaluation, and output.

## Control conventions

Use visible product vocabulary for accessible names. Names must be stable across
layout changes and must not contain row numbers or coordinates. Descriptor-backed
controls derive their name and scalar metadata from the existing
`PropertyDescriptor`; the visible mnemonic label is explicitly related to the
same real control. A descriptor appearing or disappearing therefore follows
the existing applicability projection rather than duplicated accessibility
logic. `Pattern family` is the stable name of the compact pattern candidate
dropdown. The preview is one concise `Toniator preview` image; generated
halftone paths, regions, and marks are intentionally absent from AT-SPI.

New GTK controls should provide a meaningful identity, normal GTK role/state
and value exposure, a label relation where applicable, and normal keyboard
reachability. Use Escape/default/cancel behavior provided by GTK dialogs rather
than inventing global property shortcuts.

Finite descriptor bounds use the real GTK `SpinButton` so AT-SPI exposes its
native Value/range. Open-ended descriptor entries retain truthful text-entry
semantics and must not claim an invented range or spin-button role.

## Private AT-SPI workflow

Start the isolated session described by
`.agents/skills/gtk-wayland-debug/SKILL.md`, build `toniator-app`, then start
the application in that session. Start with the narrow semantic path:

```bash
scripts/ui wait 'Pattern family' --exact --role 'combo box'
scripts/ui controls --subtree 'Pattern family' --depth 4 --json
scripts/ui inspect 'Pattern family' --exact --role 'combo box'
scripts/ui select 'Pattern family' 'Straight Grid Circles' --exact --role 'combo box'
scripts/ui inspect 'Pattern family' --exact --role 'combo box'
scripts/ui inspect Opacity --exact --role 'spin button'
scripts/ui set Opacity 0.75 --exact --role 'spin button'
scripts/ui toggle 'Channel settings' --exact --role 'toggle button'
scripts/smoke-accessibility assets/raster-sample.png
```

Use `tree` only as a diagnostic fallback when the narrow query cannot explain
the hierarchy.

The normal fast path is `wait`, `controls --subtree`, `inspect`, one semantic
operation, then `wait` or `inspect` readback. Use full `tree` only to diagnose
an unknown hierarchy or a failed narrow query.

The helper supports `tree`, `find`, `controls`, `wait`, `inspect`, `focus`,
`activate`, `set`, `toggle`, `select`, and `type`; its existing `action` command
remains available to existing wrappers. It emits compact JSON/text including role, name,
description, enabled/disabled and focus/checked/pressed/selected/expanded state, text,
value/range, actions, and useful relations. Use `--ancestor NAME` or
`tree --subtree NAME` when a repeated product name needs a semantic parent
constraint. Do not use object addresses or screen positions as selectors.

`controls` inventories only interactive or state-bearing real GTK nodes; it
keeps disabled controls and accepts `--subtree NAME`, optional
`--subtree-role ROLE`, `--ancestor NAME`, `--role ROLE`, `--depth N`, `--limit N`,
and `--json`. It chooses the shallowest interactive match for a shared
label/control name; use `--subtree-role` when that remains ambiguous. `wait NAME` waits for one
semantic match to be present (default), absent (`--absent`), or expose one
native state such as `--state enabled`, `disabled`, `focused`, `checked`,
`pressed`, `selected`, or `expanded`. It accepts the normal semantic selector
flags plus bounded `--timeout SECONDS` (default 3, maximum 30) and
`--interval MILLISECONDS` (default 50). It fails on ambiguous present targets
or timeout; successful output contains the compact final record (or an explicit
`present: false` result for absence).

The `select` operation opens the actual GTK dropdown and finds the option only
inside that selector's owned popup subtree. It tries the exposed list selection
and list-box default action before the normal loopback keyboard fallback. It
fails with a diagnostic unless the owning `GtkDropDown` readback changes.

## Semantic versus spatial interaction

AT-SPI is the default for controls, menus, popovers, dialogs, and text/value
editing. Pointer coordinates are reserved for inherently spatial operations:
for example placing or manipulating geometry in the construction canvas.
AT-SPI success proves only exposed widget state. It does not prove a rendered
preview, export bytes, persistence, or visual arrangement. Capture a grim
screenshot through the private harness and inspect it for every visible GTK or
rendering change.

## Troubleshooting

If a widget is absent, first use `tree --subtree` or `find --ancestor` to
confirm that its containing dialog/popover is open and that the descriptor is
applicable. Inspect the control's enabled state and relations before trying an
action. Rebuild the bounded app target after resource changes, restart only the
private harness application, and inspect its AT-SPI and app logs. Do not fall
back to coordinates for an ordinary widget simply because a name is ambiguous;
use a role or semantic ancestor. If GTK/AT-SPI and pixels disagree, retain both
the compact semantic output and screenshot in the evidence bundle.

Before direct AT-SPI focus or bounded Tab/Shift+Tab traversal, `focus` listens
for the standard `object:state-changed:focused` event and accepts only a true
event whose source is the selected node or its real descendant. It returns the
event source's semantic record (with state polling as supporting evidence),
never activation as a substitute. If neither event nor state arrives, record
the private compositor/AT-SPI limitation rather than treating Sway window focus
as widget focus.

## Future-control checklist

Every app diff must account for each new or changed interactive control, even
when no new accessibility code is needed. Confirm its stable product
name/hierarchy; real GTK role, state, value/selection, and actions; visible
label relation where applicable; truthful enabled/applicability state; normal
keyboard path; semantic action plus readback test; and a relevant screenshot
and log check. Reuse visible product or descriptor authority, never a second
accessibility model. Record why an item is unchanged when an existing control
already satisfies the checklist.
