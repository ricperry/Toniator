# TON-013 remaining Rust widget inventory

This inventory is intentionally scoped to widgets still constructed by Rust
after the Blueprint migration. Static main-window hierarchy and ordinary
controls are not exceptions: they live in `resources/toniator-window.blp`.

| Remaining construction | Why it remains dynamic | Static replacement status |
| --- | --- | --- |
| `PreviewIndicator` drawing area | Custom animated status visualization with a draw function and tick callback. | Its host is Blueprint; only the custom surface is runtime-owned. |
| `CenterStage` | Custom layout widget that reports zoom-sized minimums and centers the canvas without losing scroll overflow. | `canvas_stage_host` and `canvas_content` are Blueprint-owned. |
| Motif and curve drawing/controller behavior | Drawing callbacks, pointer gestures, keyboard handling, and selection state are runtime behavior. | `motif_overlay` and `curve_editor` surfaces are Blueprint-owned. |
| Help buttons and help popovers | The catalog entry and popover content are selected from the active semantic control at runtime. | Each Blueprint layout exposes a named help host. |
| Recovery action | It exists only when a recovery artifact is present. | `start_recovery_host` is a Blueprint placeholder. |
| Per-channel and aggregate panel instances | Seven semantic channel instances and one aggregate scope instance are created/reused from their separate Blueprint templates. | Repetition and page names are dynamic; template layout is declarative. |
| File chooser, export, save, preset, and color dialogs | Transient windows need runtime filters, paths, document state, and response callbacks. | Dialog contents are not ordinary main-window layout. |
| Runtime gesture/event controllers | GTK controllers carry callbacks, cancellation, keyboard, and drag behavior. | They are behavior, not static widget hierarchy. |
| Test fixtures | Tests deliberately construct isolated widgets to test GTK allocation, focus, and controller behavior. | They are not production UI sources. |

Any new static widget needed by the main window belongs in
`toniator-window.blp`. A new Rust construction requires a reason in this table
and a focused test or runtime check.

Production construction sites covered by the inventory are:

- `AppUi::open_menu`, `open_preset_dialog`, `browse_preset_dialog`, and
  `save_treatment_dialog` for transient action/popover contents;
- `open_artwork_dialog`, `open_document_dialog`, and `open_shape_editor` for
  runtime file filters and the custom shape editor window;
- `export_svg_dialog` and `export_png_dialog` for export-only options windows;
- `help_handle` and the `labeled_combo_row*` helpers for runtime help content
  and transient dialog rows;
- `build_start_view` only for the conditional recovery action; all other
  start-page widgets are Blueprint-owned;
- `PreviewIndicator::new`, `CenterStage::new`, and the drawing/controller
  installation in `AppUi::connect_actions` and the curve/motif connection
  methods for custom surfaces and behavior.

The remaining `gtk::Box`, `gtk::Button`, `gtk::Scale`, `gtk::DropDown`, and
similar constructions under `#[cfg(test)]` are isolated GTK allocation,
focus, and controller fixtures. They are not reachable production UI and are
kept so the declarative hierarchy can be tested without weakening runtime
coverage.
