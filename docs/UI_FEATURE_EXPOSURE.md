# Toniator UI feature exposure inventory

This is the planning inventory for TON-016. It records which Toniator
capabilities need a discoverable GUI location after the product feature set
has stabilized. It is not a Blueprint source and must not become a second
static UI hierarchy.

TON-013 establishes the editable implementation boundary: static layout lives
in Blueprint, while state, models, callbacks, dynamic content, and custom
surfaces remain runtime-owned. TON-016 decides how the complete feature set
should eventually be organized within that boundary.

## Timing and decision rule

Do not perform broad reorganization while dependent creative features are still
being added. Add new capabilities to this inventory first, then review the
whole hierarchy when the feature boundaries are stable.

Use these exposure states:

* **Exposed** — a user can find and operate it in the current GUI.
* **Exposed, placement pending** — the control exists, but its final grouping
  or prominence is intentionally undecided.
* **Conditional/dynamic** — the feature is exposed when its mode, channel, or
  document state makes it applicable.
* **Runtime-only** — behavior exists, but a deliberate GUI surface is still
  needed or its discoverability is unresolved.
* **Planned** — the feature is tracked elsewhere or does not exist yet.

## Proposed hierarchy

This is a review structure, not a committed widget tree.

```text
Application / Document
├── New, Open, Save, recovery, recent files
├── Undo, Redo
└── Export and document actions
Source
├── Artwork source
├── Source alpha policy
├── Source guidance
└── Future source-processing features
Output
├── Output model
├── Channel assignment
└── Compatibility actions
Channels
├── Aggregate scope
├── Per-channel settings
├── Visibility and channel selection
└── Channel color, opacity, and overrides
Treatment
├── Pattern selection
├── Shapes
│   ├── mark geometry
│   ├── sampling and threshold
│   └── advanced settings
├── Curves
│   ├── curve geometry and editor
│   ├── line weight and spacing
│   └── advanced settings
└── Repeated motifs and layout
Canvas / Presentation
├── Preview surface and export background
├── Fit and zoom
├── Rendered/source comparison
└── Editing overlays and status
Help / Accessibility
├── Contextual help
├── Keyboard and pointer guidance
└── Labels, descriptions, and status feedback
```

## Current exposure inventory

| Area | Capability or setting | Current state | Current surface or owner | Future review |
| --- | --- | --- | --- | --- |
| Application / Document | New, Open, Save | Exposed, placement pending | Header actions; document callbacks | Candidate for one document hamburger menu |
| Application / Document | Undo and Redo | Exposed, placement pending | Header actions; document history | Compare toolbar prominence with menu access |
| Application / Document | Recovery action | Conditional/dynamic | Start page recovery host | Keep visible when recovery exists; do not hide in a generic menu |
| Application / Document | SVG and PNG export | Exposed, placement pending | Export action and dialogs | Decide whether Export stays prominent or joins document actions |
| Source | Artwork source | Exposed | Source expander; document pipeline | Preserve clear distinction between source data and output model |
| Source | Source alpha policy | Exposed | Source expander; document pipeline | Review terminology as source features expand |
| Source | Source guidance and notes | Exposed | Source expander; runtime labels | Keep adjacent to the control it explains |
| Output | Output model | Exposed | Output expander; document pipeline | Keep separate from Artwork Source |
| Output | Channel assignment | Exposed | Output expander; document pipeline | Revisit when per-channel assignment expands |
| Output | Legacy Crosshatch compatibility | Conditional/dynamic | Output action and treatment state | Keep discoverable without making compatibility behavior primary |
| Channels | Aggregate treatment scope | Conditional/dynamic | Channel Settings expander | Distinguish “All Inks” from an individual channel |
| Channels | Per-channel treatment settings | Conditional/dynamic | Reusable channel template | Preserve one reusable hierarchy for each semantic channel |
| Channels | Channel visibility and selection | Conditional/dynamic | Shapes/Curves channel controls | Review whether visibility belongs with Channels or Treatment |
| Channels | Ink color and opacity | Conditional/dynamic | Channel template controls | Decide shared versus per-treatment presentation |
| Treatment | Pattern/treatment selection | Exposed | Treatment Settings expander | Revisit when additional patterns are implemented |
| Treatment | Shapes geometry and sampling | Exposed | Shapes controls | Group basic controls before advanced controls |
| Treatment | Curves geometry and editor | Exposed | Curves controls | Keep direct manipulation discoverable and keyboard-accessible |
| Treatment | Repeated motifs and layout | Exposed | Motif controls and canvas overlay | Review as pattern families expand |
| Canvas / Presentation | Preview surface and export background | Exposed | Appearance / Canvas & Export expander | Keep preview-only and export-only concepts distinct |
| Canvas / Presentation | Fit, zoom, and source comparison | Exposed | Canvas controls | Decide which actions remain directly visible |
| Canvas / Presentation | Editing overlays and render status | Conditional/dynamic | Canvas and runtime drawing surfaces | Preserve immediate feedback during interaction |
| Help / Accessibility | Contextual help and accessible names | Exposed | Help hosts, labels, descriptions | Audit after each hierarchy change |

## Stable implementation references

These references make the planning inventory actionable without making the
document a second hierarchy. Add or update the reference when a feature gains
or loses a GUI surface.

| Area | Representative IDs or runtime owner |
| --- | --- |
| Application / Document | `new_project_button`, `open_button`, `save_button`, `undo_button`, `redo_button`, `export_button`, `controls_toggle` |
| Source | `artwork_source`, `source_alpha`, `source_dynamic_host` |
| Output | `output_mode`, `channel_assignment`, `crosshatch_action` |
| Channels | `channel_scope`, `channel_panel_stack`, `toniator-channel-controls.blp`, `toniator-aggregate-channel-controls.blp` |
| Shapes | `dots`, `squares`, `lines`, `web_*` controls, `motif_*` controls |
| Curves | `curves`, `curve_*` controls, `curve_editor` |
| Canvas / Presentation | `canvas`, `canvas_content`, `picture`, `fit`, `zoom`, `zoom_entry`, `rendered_view`, `compare`, `preview_surface`, `export_background` |
| Help / Accessibility | Named help hosts, `help_handle`, accessible properties, and runtime contextual-help callbacks |

## Exposure backlog

Add an entry here before implementing or hiding a feature that changes the
user-facing surface.

| Feature | Related issue | Desired hierarchy location | Exposure decision | Dependency / notes |
| --- | --- | --- | --- | --- |
| Source-sampled mark colors | TON-014 | Treatment or Appearance, pending design | Planned | Must remain independent from scalar source sampling |
| New pattern families and per-channel assignment | TON-010, TON-011 | Treatment and Channels | Planned | Do not finalize grouping until the pattern registry is stable |
| Additional source-processing capabilities | TON-012 deferred capabilities | Source | Planned | Add only when the underlying behavior exists |
| Application/document hamburger menu | TON-016 | Application / Document | Planned | Candidate grouping for New, Open, Save; validate discoverability first |
| Future feature not yet assigned | Add issue ID | Add proposed hierarchy path | To decide | Record owner and dependency before UI implementation |

## Reorganization questions

Resolve these together rather than one control at a time:

1. Which actions must remain one click away for a creative editing workflow?
2. Should New, Open, and Save share an application/document hamburger menu?
3. Should Undo, Redo, Export, Controls, or Help join that menu, or remain
   prominent?
4. Which controls are document semantics, which are channel semantics, and
   which are treatment parameters?
5. Which advanced controls should be progressively disclosed without hiding
   essential feedback?
6. How should the hierarchy adapt when the selected output model changes the
   available channels?
7. Which actions need keyboard shortcuts, direct manipulation, or persistent
   status feedback even if their buttons move into a menu?

## Completion gate for TON-016

Do not close this issue until:

* dependent feature issues have either landed or had their GUI boundaries
  explicitly decided;
* every current and planned user-facing capability has an inventory entry;
* a reviewed hierarchy and menu grouping decision exists;
* the proposed hierarchy is tested with representative creative workflows;
* the implementation plan preserves Blueprint editability and Rust behavior
  ownership;
* any moved actions have updated accessibility labels, shortcuts, help, and
  documentation.
