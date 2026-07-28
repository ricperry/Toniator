# TON-010 Stage 1 pattern-registry implementation evidence

- Repository absolute path: `/home/ricperry1/projects/Toniator`
- Git HEAD: `f9c138c493a9d687b5300abddf14e78281f2ad63`
- Producing agent: `desktop_implementer`
- Timestamp: 2026-07-28T13:00:05-04:00
- Task: bounded TON-010 Stage 1 non-visual pattern framework groundwork.

## Working-tree assumptions

- The starting worktree contained broad, preserved TON-013 changes in
  `ISSUES.md`, `docs/UI_ARCHITECTURE.md`, `src/ui.rs`, deleted legacy UI
  resources, and untracked UI/resource/documentation files. None were edited.
- `src/lib.rs` was clean and `src/pattern.rs` did not exist before this task.
- The user declared this implementation pass the sole writer. No active-writer
  marker was present under `.codex-work/` during inspection.

## Exact files changed

- `src/pattern.rs` (new)
- `src/lib.rs`
- `.codex-work/agents/desktop-implementer/ton-010-stage-1-pattern-registry-implementation.md`

## Verified implementation decisions

- `PatternId` has two stable dotted serde values:
  `compat.shapes.v1` and `compat.curves.v1`. Parsing first validates dotted
  identifier syntax, then rejects unknown IDs.
- `PatternMetadata` records ID, family, output kind, parameter schema version,
  generator version, and an explicit legacy-render compatibility declaration.
- The static `PATTERN_REGISTRY` contains only the existing Shapes and Curves
  compatibility entries. It validates duplicate IDs and offers stable lookup.
- `VersionedPatternParameters` is a serializable object envelope. Validation
  requires a registered ID and exactly supported schema and generator versions;
  member values intentionally remain opaque until a later schema-owning slice.
- `CanonicalPatternOutput` keeps `MarkPatternOutput(MarkSet)` and
  `PathPatternOutput(CurveGeometry)` distinct. These public existing canonical
  geometry types were directly reusable, so no opaque serialized geometry
  surrogate or render-module change was needed.
- Nothing reads this registry yet: `RenderVariant`, document state,
  persistence, preview/export dispatch, and Shapes/Curves behavior remain
  unchanged. No UI, visible patterns, plug-in execution, or schema bump was
  introduced.

## Existing abstractions reused

- `render::MarkSet` for existing discrete Shapes geometry.
- `curve_render::CurveGeometry` for existing Curves path geometry.
- Existing crate serde/serde_json facilities for durable identifier, metadata,
  and parameter-envelope serialization.

## Tests and checks

- `rustfmt --edition 2024 src/pattern.rs src/lib.rs` — passed. A standalone
  formatter attempt without the edition flag could not parse the Rust 2024
  let-chain already present in `src/artwork_pipeline.rs`; it made no changes.
- `cargo fmt --check` — passed.
- `cargo test --lib pattern::tests` — passed: 6 focused tests covering IDs,
  registry lookup/uniqueness, family/output serde, version rejection, unknown
  ID rejection, and separate outputs.
- `cargo clippy --lib --tests -- -D warnings` — passed.
- `cargo test --lib` — passed: 123 tests.
- `git diff --check -- src/pattern.rs src/lib.rs` — passed.

## Artifacts and limitations

- No screenshots, exports, GTK launch, or runtime UI artifacts were produced:
  this slice is intentionally non-visual and has no runtime routing.
- `MarkSet` and `CurveGeometry` are transient canonical geometry and do not
  derive serde. The wrapper boundary therefore stays typed runtime state rather
  than persisted geometry, which is appropriate until a later generation and
  persistence slice defines durable pattern-instance state.
- Durable documentation likely affected at milestone review: the pattern
  generation/persistence boundary and eventual stable ID catalog.

## Follow-up review targets and invalidation

- A later Stage 1 slice must add model-owned pattern-instance persistence and
  document compatibility/migration rules before saved documents can select
  registry entries. It should validate the exact metadata versions through
  load/save without bumping a document format prematurely.
- Later routing work should adapt `RenderVariant::WebShapeV1` and
  `RenderVariant::WebCurveV1` deliberately, preserving existing preview/export
  semantics and output-model behavior while making registry dispatch live.
- This evidence is invalidated by changes to `src/pattern.rs`, `src/lib.rs`,
  `src/model.rs`, `src/render.rs`, `src/curve_render.rs`, `src/persistence.rs`,
  `src/preset.rs`, Git HEAD, or the recorded dirty-worktree assumptions.
