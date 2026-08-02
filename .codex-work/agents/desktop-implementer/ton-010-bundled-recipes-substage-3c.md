# TON-010 bundled recipes — Substage 3C implementation evidence

Date: 2026-08-01

## Scope and baseline

- Git HEAD observed before work: `262c7e857446ded100d4a90fd23d651e52460665`.
- The worktree was intentionally dirty with accepted Stage 5 framework files and unrelated user/parent edits. All were preserved; no commit, push, deletion, persistence/preset format change, UI change, or Shapes/Curves conversion occurred.
- This handoff follows accepted 3B. It changes only the live `RenderVariant::WeightedVoronoiCanonicalV1` generation authority.

## Changed files

- `src/render.rs`
  - Replaces the one Weighted render-branch call to `generate_weighted_voronoi_cancellable` with `execute_bundled_weighted_voronoi_recipe_cancellable`.
  - Leaves source decoding, field resolution, cancellation checkpoints, output validation, and all shared canonical preview/PNG/SVG consumer paths unchanged.
  - Adds a real-document dispatch test using a thread-local seam, proving one recipe execution and zero retained-oracle executions.
- `src/weighted_voronoi.rs`
  - Makes the former whole-generator and its cache metadata test-only oracle code. It is retained only for focused recipe-vs-oracle equivalence tests and cannot be reached by a production build.
  - Records a test-only live-dispatch counter at the recipe executor boundary; no production instrumentation is emitted.
  - Updates the executor contract comment to identify it as live renderer authority.
- `src/lib.rs`
  - Removes the old generator and oracle-only cache types from public production reexports while retaining the bounded production registry and recipe executor APIs.

## Authority and compatibility decisions

- `Document.pattern_state.weighted_voronoi_settings()` remains the sole persisted settings authority. The renderer continues resolving the same selected output-model fields and passes them directly to the recipe executor.
- Preview Surface and Export Background remain presentation-only consumers of the single canonical output. No Weighted-specific consumer/render/export path was introduced.
- The pre-3C cache/fingerprint structs had no production caller outside the old whole-generator return value. Rather than invent a second live cache authority, they are now test-only oracle provenance. Recipe output equality is the live compatibility contract.
- The old generator is deliberately retained only under `cfg(test)` through the transition as a direct geometry/output oracle. Remove it, its test-only metadata, and the equivalence seam after later migration evidence makes duplicate comparison unnecessary.
- No site-distribution or Voronoi geometry algorithm changed.

## Verification

- `cargo fmt`
- `cargo test --locked live_weighted_document_render_enters_recipe_not_test_oracle --lib` — focused live dispatch proof passed.
- `cargo test --locked weighted_voronoi --lib` — 14 passing focused Weighted/bundle/oracle tests, including deterministic live CMYK preview/PNG/SVG canonical consumption.
- `cargo test --locked weighted_svg --lib` — RGB/CMYK editable semantic SVG, no cell-sizing masks, passed.
- `cargo test --locked document_png_uses_saved_export_background --lib` — transparent/opaque document background behavior passed.
- `cargo test --locked weighted_voronoi_state_is_authoritative --lib` — authoritative settings, undo/redo, obsolete generator rejection passed.
- `cargo test --locked current_project_roundtrips --lib` — save/reopen compatibility regression passed.
- `cargo test --locked` — 189 library tests and 48 binary/UI tests passed.
- `cargo check --locked` and `cargo check --locked --release` — passed; release compilation has no old generator path.
- `cargo clippy --locked --all-targets -- -D warnings` — passed.
- `cargo fmt --check` and `git diff --check` — passed.

No interactive GTK launch or screenshot was required: 3C changes no UI behavior and exercised the existing canonical preview, PNG, and SVG paths through unit/integration coverage. No new graphic artifact was created.

## Parent review targets and invalidation conditions

- Confirm the test-only oracle cleanup boundary is acceptable: production compilation no longer includes the duplicate whole-generator, while focused tests still compare the recipe against it.
- Review the live-dispatch test seam: it is thread-local and test-only, so parallel tests are isolated and no source-text dispatch assertion is used.
- Invalidate this evidence if the Weighted recipe graph/ports, `Document.pattern_state` settings adapter, field-resolution contract, canonical consumers, or recipe executor dispatch changes.
