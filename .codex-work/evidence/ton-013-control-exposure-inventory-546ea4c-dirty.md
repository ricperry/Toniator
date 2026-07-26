# TON-013 control-exposure inventory

- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `546ea4c5eb1fec8e91c2b307545e33e42331e308`
- Working tree: dirty; preserve `src/ui.rs`, `ISSUES.md`, `.codex-work/`, `resources/`, and `docs/UI_ARCHITECTURE.md` changes/untracked files.
- Producing agent: codebase explorer (read-only)
- Task: inventory remaining Rust-owned static controls and define the practical Builder boundary for TON-013.

## Verified findings

Stage 1 shell and Stage 2 inspector hierarchy are Builder-owned. Most concrete Source, Output, Appearance, and Treatment controls are still Rust-created and appended into Builder hosts. `EditorWidgets` and `AppearanceControlWidgets` provide direct migration seams. Dynamic models, callbacks, synchronization, semantic channel identity, drawing, dialogs, and crash protections remain Rust-owned.

Recommended Builder-owned clusters are the editor shell/layout, Source and Output rows, Appearance sections and control shells, treatment chrome and static row structure, and stable placeholder hosts. Rust should continue to own dropdown model contents, semantic mappings, conditional visibility/sensitivity, mixed-value synchronization, callbacks, custom drawing widgets, dialogs, preview/export workers, undo/redo, and GTK crash protections.

Stable existing IDs must remain unchanged: shell IDs; `editor_inspector_hierarchy`, `source_section`, `source_content_host`, `output_section`, `output_content_host`, `channel_settings_section`, `channel_scope_host`, `channel_panel_stack`, `appearance_content_host`, `treatment_content_host`; real-channel IDs; and aggregate IDs. Recommended new IDs use semantic groups such as `source_artwork_source`, `source_alpha`, `output_model`, `channel_assignment`, `active_channel`, `crosshatch_action`, `preview_surface`, `export_background`, `treatment_pattern_buttons`, `treatment_preset_actions`, `treatment_modes`, and named treatment panel/host IDs.

The reusable real-channel composite should gain only static row shells where lifecycle is identical across real channels; it must remain distinct from the aggregate panel and must not duplicate dynamic treatment editors prematurely.

## Inference and uncertainty

Static row composition and widget shells can move without semantic redesign if existing GTK widget types and field bindings remain stable. Treatment-specific controls should migrate incrementally because their visibility, terminology, mixed-value handling, and custom drawing are tightly coupled to runtime state. Cambalache round-trip behavior for all desired GTK4 properties and whether empty dynamic models should be declared in XML versus attached by Rust remain unverified.

## Inspected paths and commands

Inspected `/home/ricperry1/projects/Toniator/src/ui.rs` (`AppUi`, `EditorWidgets`, `AppearanceControlWidgets`, `build_editor_view`, `build_appearance_controls`, `build_inspector_hierarchy`, `build_channel_controls`, callbacks, synchronization, and row helpers), all current `resources/ui/*.ui` and `Toniator.cmb`, `docs/UI_ARCHITECTURE.md`, TON-013 in `ISSUES.md`, and focused Builder/realized GTK tests. Commands included `rg`, `sed`, `nl`, `wc`, `git status`, `git rev-parse HEAD`, `git diff`, and `sha256sum`.

## Invalidation

Invalidate after changes to `src/ui.rs`, `resources/ui/*`, `resources/ui/Toniator.cmb`, GTK/libadwaita versions, focused tests, relevant docs, Git HEAD, or the dirty-file assumptions.
