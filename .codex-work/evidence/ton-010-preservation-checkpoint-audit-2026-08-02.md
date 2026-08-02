# TON-010 preservation checkpoint audit — 2026-08-02

## Checkout identity

- Repository: `/home/ricperry1/projects/Toniator`
- Branch: `TON-010-Stage5-Framework-Restart`
- Audited base HEAD: `262c7e857446ded100d4a90fd23d651e52460665`
- State: dirty checkout containing the accumulated recipe/runtime/editor work
  listed by `git status --short --untracked-files=all`
- Producing roles: parent integration review plus read-only
  `ton010_checkpoint_audit` codebase specialist
- Task: preserve and truthfully document the current expanded TON-010 state
  before beginning a disciplined closeout session

## Files and symbols inspected

- Canonical dispatch: `src/render.rs`,
  `generate_document_pattern_output_cancellable`
- Definition/runtime: `src/pattern_definition.rs`,
  `src/pattern_definition_registry.rs`,
  `src/bundled_pattern_definitions.rs`
- Built-in operations/adapters: `src/shapes_native.rs`,
  `src/shapes_recipe.rs`, `src/curves_native.rs`, `src/curves_recipe.rs`,
  `src/weighted_voronoi.rs`
- Authority/persistence: `src/model.rs`, `src/persistence.rs`, `src/preset.rs`
- Editor and presets: `src/ui.rs`, `pattern_editor_recipe`,
  `PATTERN_PRESET_LABELS`, `apply_named_pattern_preset`,
  `save_pattern_draft_as`
- Bundled assets: `assets/patterns/*.tnpattern`
- Durable status: `ISSUES.md`,
  `docs/TON-010_STAGE_5_FRAMEWORK_RESTART.md`,
  `docs/TON-010_STAGE_5_ARCHITECTURE_MAP.md`

## Verified implementation state

1. `Document.pattern_state` selects the canonical pattern. Embedded custom
   Shapes definitions are checked before bundled Shapes, Curves, and Weighted
   Voronoi routes. Preview, PNG, and SVG consume the resulting canonical
   output.
2. All three bundled definitions are parsed through the strict `.tnpattern`
   loader and executed through bounded registered native operations.
3. Persistence is strict document v9 and `.tntr` v6. Embedded custom
   definitions/instances are persisted; obsolete definitions are rejected.
4. The visible Pattern Editor owns a structural draft; main-window Channel
   Settings owns per-ink styling, sampling, seed, and weighting values. Both
   project into the authoritative selected pattern instance on Apply.
5. Weighted Voronoi still delegates site placement and tessellation to
   `site_distribution.rs` and `voronoi_geometry.rs`; no audit change touches
   those algorithms.

## Verified incompleteness and architectural debt

1. The authoring surface is not a general recipe editor.
   `pattern_editor_recipe` adapts the bundled Shapes definition and mutates
   fixed nodes, operation IDs, and parameter keys.
2. Named options are index-driven. `apply_named_pattern_preset` injects bespoke
   defaults for grid, triangular, math, and random variants. This is the exact
   pattern-specific seam the next session must stop extending.
3. `shapes.lattice-placement-editor` is a bounded but monolithic native
   operation whose Rust branches implement the editor variants. Registered
   native operations are an intentional safety boundary; the problem is the
   lack of a useful composable public operation/parameter surface.
4. Save As writes into the XDG user pattern directory, but no UI path reloads,
   imports, browses, or resolves those files. `PatternDefinitionRegistry` is
   not wired as the application-level bundled/user/project authority.
5. The promised graph editor does not exist. Guided editing is a fixed
   Shapes-specific form rather than schema-driven editing of the authoritative
   recipe graph.
6. `RenderVariant`, NativeBasic, Crosshatch compatibility, and Shapes/Curves
   typed adapters remain live execution seams.
7. Stage 6 proof recipes, portability/recovery, and human Stage 5 acceptance
   remain incomplete.

## Control ownership conclusion

Pattern definitions own reusable construction and topology. Channel instances
own per-ink treatment and source response. The current UI mostly follows this
rule, but legacy names such as `pattern_editor_random_size_response` make the
boundary look less clean than the behavior. The audit did not prove that this
control is a second persisted authority: `pattern_editor_recipe` reads the
selected channel setting. Treat the naming/synchronization seam as a focused
follow-up requiring behavior tests, not as license for a broad rewrite.

## Evidence and commands

- `git status --short --branch`
- `git diff --stat`
- targeted `rg`/`sed` inspection of the files and symbols above
- specialist search confirmed `native_user_pattern_dir` and normalized pattern
  paths are used only by the UI Save As flow; no UI parse/load/import path was
  found
- Existing evidence records the last complete suite at 261 library tests and
  56 binary/UI tests plus strict Clippy, release, docs, and realized GTK checks.
  The parent checkpoint validation is recorded below.

## Checkpoint validation

- `cargo fmt --all -- --check` — passed.
- `cargo test --locked --all-targets` — passed: 261 library tests and 56
  binary/UI tests.
- `cargo clippy --locked --all-targets --all-features -- -D warnings` —
  passed.
- `cargo check --locked --release` — passed.
- `cargo test --doc --locked` — passed: 0 doctests.
- `git diff --check` — passed.
- `timeout 30s cargo run --locked -- --demo --show-controls
  --expand-document --window-size 1200x1400 --screenshot
  /tmp/toniator-ton010-preservation-checkpoint-2026-08-02.png` — passed with
  no GTK/libadwaita critical output. Parent visual inspection confirmed the
  current Pattern Settings and Channel Settings hierarchy and rendered CMYK
  canvas. This is a realized startup/resource/render check, not manual
  interaction acceptance.

## Uncertainty and invalidation

- This audit does not claim manual GNOME/Wayland, Krita, or Inkscape acceptance.
- It does not establish arbitrary external-recipe runtime compatibility.
- Invalidate after changes to the listed source/assets/docs, branch HEAD, or
  the documented dirty-state assumptions.
