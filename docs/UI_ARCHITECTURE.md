# Toniator declarative UI architecture

TON-013 uses Blueprint as the maintained UI source. The main application
window is reconstructed in `resources/toniator-window.blp`; it is not split
into shell, inspector, and editor fragments. The file contains the complete
static hierarchy:

This is the TON-013 migration boundary, not the final product information
architecture. Broad reorganization and action grouping are intentionally
deferred to TON-016 while the remaining creative features are added. Track
future exposure decisions in `docs/UI_FEATURE_EXPOSURE.md` rather than
fragmenting the maintained Blueprint source prematurely.

```text
Adw.ApplicationWindow main_window
└─ Adw.ToolbarView main_toolbar_view
   ├─ Adw.HeaderBar main_header_bar
   └─ Adw.ToastOverlay toast_overlay
      └─ Gtk.Stack main_stack
         ├─ start_page
         └─ editor_page
            └─ Adw.OverlaySplitView editor_split_view
               ├─ sidebar: inspector_shell
               │  └─ inspector_content / editor_controls
               │     └─ Source, Output, Channel Settings,
               │        Appearance / Canvas & Export, Treatment Settings
               └─ content: canvas_box
                  ├─ canvas and canvas_content
                  ├─ status_row
                  └─ canvas_controls
```

The Source, Output, Appearance, Shapes, Curves, and Motif widgets are all
declared in that same Blueprint hierarchy. Their stable IDs remain unchanged.
Rust retrieves these objects by ID and supplies models, ranges, state,
callbacks, conditional visibility, help popovers, dialogs, and drawing
behavior. It does not construct ordinary static main-window controls.

The only separate Blueprint sources are
`toniator-channel-controls.blp` and
`toniator-aggregate-channel-controls.blp`. The former is instantiated once
for each semantic C/M/Y/K or R/G/B channel; the latter is the distinct
aggregate scope panel. Neither is a fragment of the main window.

## Build and resource pipeline

`build.rs` invokes `blueprint-compiler compile` for every tracked `.blp` file,
writing generated `.ui` files only into Cargo's `OUT_DIR`. It then invokes
`glib-compile-resources` with `resources/toniator.gresource.xml`. Runtime code
registers the generated `toniator.gresource` and loads
`/com/toniator/Toniator/toniator-window.ui` with
`gtk::Builder::from_resource`; the repeatable channel templates use their own
resource paths.

Generated `.ui` files are build artifacts and must not be committed. The
tracked `.blp` files are the only maintained declarative source. A clean
checkout can therefore build without any pre-existing generated UI files.

Useful checks:

```sh
blueprint-compiler lint -r syntax resources/toniator-window.blp
blueprint-compiler lint -r syntax resources/toniator-channel-controls.blp
blueprint-compiler lint -r syntax resources/toniator-aggregate-channel-controls.blp
cargo test --locked
cargo clippy --all-targets -- -D warnings
```

The complete Rust-side widget-construction inventory and the reason each
remaining construction is dynamic or custom is recorded in
`docs/UI_WIDGET_INVENTORY.md`.
