# TON-010 bundled recipes — Substage 3E1 parent review

- Repository/HEAD: `/home/ricperry1/projects/Toniator` at dirty `262c7e8`
- Date: 2026-08-01
- Producer: `desktop_implementer`; parent inspected the Curves definition,
  adapter, typed ports, retained dispatch boundary, tests, and evidence.

## Accepted findings

- Immutable `compat.curves.v1.tnpattern` bytes load through the common strict
  parser and bundled registry. Its six-node graph has Curves-specific typed
  ports and complete Placement, Motif, Deformation, Modulation, and Output
  authoring sections.
- The one-way adapter reads authoritative `Document.pattern_state` Curves
  settings, maps every renderer-relevant global and per-channel field, and
  leaves artwork assignment, Crosshatch, and inspector-only `base_channel`
  outside the recipe.
- Operation ownership follows retained dataflow: motif selection owns only
  path/closure semantics; deformation owns layout, sampling, repetition, and
  transforms; modulation owns source widths and simplification. The shared
  `output-quality` parameter is bound to both consuming nodes.
- Automatic motif coverage calls retained `max_curve_width` before modulation,
  so `min-mark`, `max-mark`, `max-size`, and `scale` are likewise bound to both
  deformation's coverage guards and modulation's final width calculation.
- Legacy `tile_spacing` and `show_background` fields have no current render or
  export consumer, so they are explicitly excluded rather than exposed as
  ineffective recipe controls. Their compatibility fields remain untouched
  until the later strict v9 cleanup.
- Shared and per-channel editable cubic paths become digest-verified embedded
  SVG assets. The adapter enforces 1..=64 finite cubic segments and semantic
  `#rrggbb` channel colors.
- Declared numeric/count bounds match current Curves model validation. The v1
  inclusive interval limitation for positive-only resolution and quality is
  explicit: the definition declares `0..=100`, while the Curves semantic
  adapter rejects zero and accepts every positive finite value, including
  subnormals.
- Production Curves rendering remains on the retained
  `curve_render::generate_curve_geometry_for_pipeline` path. No Curves native
  bodies, executor, live dispatch, schema, preset, or UI change is claimed.

## Verification

- Parent `cargo test --locked curves_recipe --lib`: 5 passed, including exact
  parameter-to-operation ownership and no-op exclusion.
- Parent `cargo test --locked bundled_pattern_definitions --lib`: 4 passed.
- Parent retained-dispatch scan found no `curves_native` or bundled Curves
  executor and found the live call only in `src/render.rs`.
- Parent `git diff --check` passed. Writer full suite passed: 214 library and
  48 binary/UI tests, locked release check, strict all-target Clippy, format,
  and diff checks.
- Writer's noninteractive startup smoke used an intentional timeout. It is not
  human GNOME/Wayland, Krita-reference, or Inkscape acceptance.

Invalidate when the Curves model/settings validation, definition or adapter,
typed ports, SVG asset rules, retained renderer, future native executor/live
dispatch, HEAD, or relevant dirty state changes.
