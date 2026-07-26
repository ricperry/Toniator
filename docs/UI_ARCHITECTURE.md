# Toniator UI architecture

## TON-013 GtkBuilder migration stages 1–2

`resources/ui/Toniator.ui` owns the static application shell:

```text
AdwApplicationWindow (main_window)
└─ AdwToolbarView (main_toolbar_view)
   ├─ AdwHeaderBar (main_header_bar)
   └─ AdwToastOverlay (toast_overlay)
      └─ GtkStack (main_stack)
```

`src/ui.rs` loads the resource with `include_str!` and
`gtk::Builder::from_string`. Rust inserts the dynamic `start` and `editor`
pages into `main_stack`; no build script or GResource compiler is required at
this stage.

Stage 2 adds the following editable GTK Builder resources. They are listed by
relative path and SHA-256 in `resources/ui/Toniator.cmb`; the existing main
file hash remains valid.

```text
ToniatorInspector.ui
ToniatorChannelControls.ui
ToniatorAggregateChannelControls.ui
```

`ToniatorInspector.ui` owns static inspector layout and ordering. The actual
top inspector order is `Source` -> `Output` -> `Channel Settings` through
`source_section`, `output_section`, and `channel_settings_section`. Source and
Output are expanded by default; Channel Settings, Appearance / Canvas & Export,
and Treatment Settings are collapsed for progressive disclosure. Artifact
expansion opens the Source, Output, and Channel Settings groups together. Rust
inserts source widgets into `source_content_host`, output widgets into
`output_content_host`, the dynamic Treatment Editing Scope dropdown into
`channel_scope_host`, and cached panels into `channel_panel_stack`.

`ToniatorChannelControls.ui` is the reusable real-channel status/context
composite for exactly one real `OutputChannelId`. Its stable IDs are `channel_controls`,
`channel_heading`, `channel_inclusion_status`, and `channel_content_host`.
Rust creates and caches seven typed instances for the semantic C/M/Y/K and R/G/B
channels, selecting them by semantic stable ID. A dropdown position is
converted only at the callback boundary and is never retained as widget
identity.

`ToniatorAggregateChannelControls.ui` is intentionally separate from that
template. Its IDs are `aggregate_channel_controls`, `aggregate_heading`,
`aggregate_mixed_message`, and `aggregate_content_host`. It owns All
Inks/All Channels wording and explicit mixed/apply-to-all status messaging. It
is a scope-context/status panel, not a second treatment editor. Legacy
Crosshatch uses it as `All Layers`; it is not a fake output channel or a
channel-template instance.

## TON-013 control-exposure stage

`ToniatorEditorControls.ui` adds Builder-owned static Source, Output, and
Appearance rows (including dropdown and color-button shells), treatment chrome
(pattern and preset actions), and the named native/Shapes/Curves stack shells.
The five Builder-owned Source/Output dropdowns have explicit accessible names
and `LabelledBy` relations to their stable labels. The Basic/native Sampling
Detail, Coverage, Contrast, and Screen Angle rows, containers, labels, and
`GtkScale` shells are Builder-owned with semantic IDs so Cambalache can edit
their layout. Rust retrieves each static control by ID, configures the native
scale ranges/formats and existing pointer-scroll behavior, attaches the
adjustment-backed precision spin entries, attaches dynamic source/status/help
content, installs the existing live dropdown models, and retains all callbacks.
The Shapes, Curves, and Motif detail rows are still inserted into their stable
`web_panel_host` and `curve_panel_host` shells because their mixed-value rows,
runtime help, custom drawing, dialogs, and conditional visibility remain
stateful. They are an explicit next exposure target, not a second visible
control set.

## Ownership boundary

Static XML owns layout, labels, stable IDs, expanders, and placeholder hosts.
Rust owns live models, typed semantic bindings, callbacks, visibility,
sensitivity, dynamic Shapes/Curves/Motif detail rows, mixed-value and help
content, custom drawing, dialogs, rendering, and synchronization. Existing
selector controls retain their live
`gtk::StringList` identity, reject `gtk::INVALID_LIST_POSITION`, defer model
sync until idle, and keep `RefCell` borrows bounded. A refresh must not dirty a
document.

The Output `Channel Assignment` and conditional `Active Channel` controls
remain pipeline-authoritative and are the sole scalar-routing controls.
`Treatment Editing Scope` is the sole visible treatment-recipient selector: it
synchronizes the existing Shapes and Curves target models without mutating
`ChannelAssignment` or `active_channel`. The legacy `Adjust Ink`/`Adjust
Channel` rows remain hidden compatibility internals for established callbacks
and mixed-value behavior; they are not a second authoring locus. Full Color
remains editable through the treatment scope, while Crosshatch presents
disabled `All Layers` scope.

The Basic/native Sampling Detail, Coverage, Contrast, and Screen Angle rows
use Builder-owned row, label, control-container, and `GtkScale` shells. Rust
configures their live adjustments and precision entries. Dynamic
Shapes/Curves/Motif detail rows remain Rust-built below the Stage 2 hierarchy in
the intentionally collapsed `treatment_section`; the reusable channel panels
contain runtime scope/context hosts but do not duplicate unsafe callback
models. Future migration can move those dynamic rows into Builder hosts only
after preserving their mixed-value/help content, custom drawing/dialogs,
visibility, and established GTK crash protections.

## Stage 2 verification boundary

The Stage 2 bounded GTK artifact was
`test-artifacts/ton-013/stage2-treatment-scope-correction.png` at 1000x760.
Separate normal and narrow artifact-mode launches were attempted, but the
current Wayland compositor did not provide a GTK render node within the
capture timeout; those failed captures produced no claimed artifacts. No
dedicated assistive-technology tree or focus-order capture was run. Static
review found no visible or normal-keyboard focus artifact from the hidden
compatibility rows; narrow-window, screen-reader, and focus-order behavior
remain follow-up checks.

The current control-exposure correction is separately verified by
`cargo fmt --check`, `cargo test --locked` (117 library and 46 binary/UI
tests), strict Clippy, the locked release build, XML/Cambalache-file parsing,
and `git diff --check`. The current inspected artifact is
`test-artifacts/ton-013/control-exposure-stage-corrected.png` at 1000x980.
Cambalache 1.0.3 is installed, but no round-trip edit was performed. Narrow
window and assistive-technology checks remain follow-up verification.

## Editing and verification rules

Keep `Toniator.cmb` hashes current for every changed UI file. Preserve all
stable IDs above with their focused tests. `gtk4-builder-tool validate` may not
understand libadwaita shell types, so Builder parsing through the application
and XML parsing remain the relevant checks. Before merging a UI change, run the
focused Builder/realized GTK tests, normal Rust checks, and a normal plus narrow
GTK screenshot when the environment permits.

TON-013 remains In Progress: Source, Output, Appearance, treatment chrome, and
the Basic/native Sampling Detail, Coverage, Contrast, and Screen Angle row/
scale shells now expose practical Builder structure, but substantial
Shapes/Curves/Motif detail layout and its runtime content remain Rust-built
inside stable Builder hosts.
