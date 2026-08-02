# TON-010 Stage 5, Substage 4B — project-embedded custom Shapes runtime

- Timestamp: 2026-08-01T17:29:38-04:00
- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `262c7e857446ded100d4a90fd23d651e52460665`
- Producing agent: `desktop-implementer` (`/root/ton010_recipe_contract_2a`)
- Scope: bounded non-GTK custom recipe runtime only.

## Checkout assumptions

The worktree was materially dirty before this substage with the accepted TON-010 recipe-contract, bundled-recipe, schema-v9, TON-013, documentation, asset, and evidence work. I preserved those changes, made no reset/clean/revert/commit/push/deletion, and confirmed no other writing agent was active before editing. The Git diff against HEAD necessarily includes pre-existing work; this entry identifies the 4B-owned changes below.

## Exact files changed for 4B

- `src/model.rs`
- `src/shapes_native.rs` (already an untracked Stage-5 source file; extended in place)
- `src/render.rs`
- `src/persistence.rs`
- `src/lib.rs`
- `.codex-work/agents/desktop-implementer/ton-010-custom-runtime-substage-4b.md`

## Implementation decisions and reused abstractions

- Added `EmbeddedPatternDefinition { definition, instance }` and an `embedded_patterns` map to `PatternDocumentState`. Custom IDs cannot conflict with `PATTERN_REGISTRY`; both embedded IDs must agree with the map key; selected custom IDs must resolve to an embedded entry. Invalid/missing entries fail validation rather than falling back to a built-in or `RenderVariant`.
- Reused `PatternDefinition` and `PatternInstanceParameters` strict serde/validation, `SHAPES_NATIVE_OPERATION_REGISTRY`, `RecipeExecutionContext`, `ShapesRecipeSourceProvider`, `PreparedSource`, and `ArtworkPipelineSettings`. Custom definitions are constrained to the existing finite Shapes registry and mark output only.
- Added public `validate_shapes_definition_instance`, `shapes_instance_artboard`, and `execute_shapes_definition_cancellable`. The executor accepts an arbitrary validated Shapes-compatible definition/instance plus prepared source, semantic pipeline, explicit artboard, and cancellation token; it uses the production RGB/CMYK resolved-field provider and existing lattice/resource bounds.
- `DocumentEditor::install_and_select_embedded_pattern` first validates a candidate in cloned state, then commits it through the ordinary pattern-state edit path. It is therefore one undoable edit and malformed candidates leave document/history untouched.
- Live rendering checks `PatternDocumentState::selected_embedded_pattern` before the derived legacy `RenderVariant`; an embedded custom recipe remains authoritative even if the transient adapter is deliberately contradictory. The adapter is retained only as a benign derived Shapes facade for legacy cache/transition seams.

## Verified behavior and checks

- Focused model tests prove install/select authority, undo/redo, missing-definition rejection, and inert invalid installation.
- Persistence test proves save/reopen persists both definition and instance and that a selected custom ID without its embedded definition fails loading.
- Render test proves non-empty custom Shapes output and dispatch independence from a contradictory `RenderVariant::NativeBasicV1`.
- `cargo test --locked --lib` — passed, 249 tests.
- `cargo test --locked` — passed, 249 library tests and 48 binary/UI tests.
- `cargo check --locked --all-targets` — passed.
- `cargo check --locked --release` — passed.
- `cargo clippy --locked --all-targets -- -D warnings` — passed.
- `cargo fmt --all -- --check` — passed.
- `git diff --check` — passed.
- `timeout 12s cargo run --locked` — application launched and remained running until the bounded smoke timeout; no screenshot was captured because this substage changes no GTK/UI surface.

## Artifacts

No screenshot, PNG, SVG, or export artifact was created. The custom render regression exercises the production canonical render path; export consumers continue to consume that shared canonical output, but custom export parity has not yet received a dedicated fixture.

## Known limitations and follow-up review targets

- This is Shapes-compatible custom runtime only. Curves, Weighted Voronoi, arbitrary graph-family execution, custom pattern editor UI, library import/export, and GTK controls are intentionally out of scope.
- The project runtime currently derives custom Shapes artboard dimensions from required `output-width`/`output-height` instance values. The public executor keeps artboard explicit; future editors must preserve those instance values when changing canvas dimensions.
- Treatment presets can continue to carry the state DTO, but custom channel-preset authoring/import is not implemented because v6 channel records remain built-in typed settings only.
- Review later custom PNG/SVG parity and custom output-mode/cache transitions when the editor/import workflows are added.

## Documentation likely affected

The Stage 5 architecture/runtime documentation may need a durable custom-pattern portability/execution update after the parent accepts this milestone boundary. No durable documentation was changed here; this implementation evidence is not a substitute.

## Invalidation conditions

Reinspect if `PatternDefinition`/instance validation, the Shapes native operation registry, artwork pipeline source-field resolution, `PatternDocumentState`, document serialization/version policy, canonical render dispatch, or output-cache/preset behavior changes. This evidence is checkout-aware only for the HEAD and dirty-worktree assumptions recorded above.
