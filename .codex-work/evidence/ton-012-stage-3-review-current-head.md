# TON-012 Stage 3 review evidence

- Repository absolute path: `/home/ricperry1/projects/Toniator`
- Git HEAD: `bac55f70e7a77ec638b8033d7801fa07141d4d7e`
- Scope: independent GTK/state regression review of the uncommitted Stage 3 implementation.
- Reviewer: `test_reviewer`, 2026-07-26
- Files inspected: `src/artwork_pipeline.rs`, `src/model.rs`, `src/ui.rs`, `src/preset.rs`, `src/persistence.rs`, and renderer entry points.
- Verified major finding: the first uncached CMYK to RGB transition uses `Document::new_rgb_treatment()`, which hard-codes `FullColor` plus automatic RGB and loses a scalar source, alpha policy, assignment, and active channel. `Document::switch_output_mode()` must transition the current semantic pipeline before installing the target cache.
- Verified major finding: `DocumentEditor::exit_crosshatch_treatment()` replaces the current Curves settings with `WebCurveSettings::default()`, discarding ordinary Curves geometry, channel settings, visibility, and colors. Exit must preserve or restore the pre-Crosshatch ordinary Curves state.
- Verification gap: realized semantic callbacks cover source, alpha, assignment, channels, invalid positions, output transitions, and Crosshatch, but do not cover semantic undo/redo, non-default project/preset round trips, or equivalent Curve/cache assertions.
- Required follow-up: correct both major findings, add focused scalar CMYK/RGB/cache and Crosshatch state tests, then rerun the relevant suite.
- No files were edited by the reviewer.
- Invalidation: changes to output-cache transitions, Crosshatch exit semantics, reviewed UI callbacks, or Git HEAD.
