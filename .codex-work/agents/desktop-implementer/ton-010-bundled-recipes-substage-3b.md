# TON-010 bundled recipes — Substage 3B implementation evidence

Date: 2026-08-01

## Scope and baseline

- Git HEAD observed before work: `262c7e857446ded100d4a90fd23d651e52460665`.
- The worktree was intentionally dirty with accepted TON-010 framework files and unrelated user/parent edits. They were preserved. In particular, `src/pattern_definition.rs`, `src/bundled_pattern_definitions.rs`, and `src/pattern_definition_registry.rs` were accepted untracked 2A–3A work rather than new 3B ownership.
- This substage changes no renderer dispatch, persistence, preset, UI, documentation, pattern-library UI, Shapes, or Curves behavior. No commit, push, or deletion was performed.

## Changed files

- `src/weighted_voronoi.rs`
  - Adds the static production registry and six native v1 bodies: source sample, response map, site distribution, Voronoi construction, response inset, and region emission.
  - Refactors only local adapter glue into shared field conversion, response sampling, and distribution-request helpers. The shipping `generate_weighted_voronoi_cancellable` remains present and remains renderer authority.
  - Adds strict deterministic settings-to-instance adapters, including the one-way document `pattern_state` reader, and an assembled per-selected-channel recipe execution helper for equivalence use only.
  - Adds RGB/CMYK deterministic equivalence, disabled-channel, exact-seed, adapter, operation cancellation, and missing source/channel tests.
  - Corrects per-channel orchestration so disabled fields are skipped before validation, `DistributionField` conversion, and all recipe stages, matching the shipping adapter's early-skip cancellation/stale-work semantics.
- `src/pattern_definition.rs` (accepted 2A–3A untracked file)
  - Extends `RecipeExecutionContext` with transient source/resolved generations plus semantic-channel and visible-layer positions. No metadata is persisted or duplicated in the instance payload.
  - Adds `RecipeVoronoiDiagram`: the existing neutral `VoronoiDiagram` paired with the construction stage's transient ordered sites. The neutral geometry type deliberately does not own placement provenance; the response-inset stage needs the exact construction sites to preserve response sampling and inset equivalence.
- `src/lib.rs` (accepted earlier framework edits preserved)
  - Narrowly reexports the production registry, recipe-equivalence execution helper, and strict adapters.

## Decisions and existing authorities reused

- The six bodies are atomic at the declared recipe boundaries. They use `DistributionField`, `generate_site_distribution_cancellable`, `build_voronoi_diagram_cancellable`, `inset_clipped_cell_for_response`, and `RegionPatternOutput`; they do not wrap the old full generator as one operation.
- Channel identity uses the existing stable channel hash, and semantic-channel / enabled-layer positions reproduce the old region IDs, layers, and layer order when disabled fields are omitted.
- Each selected resolved semantic channel executes the bundled graph with its own source field and strict scoped instance values. `enabled = false` still produces an empty canonical region channel at the emit boundary and contributes no assembled layer or region.
- The correction clarifies that `enabled` is output-assignment orchestration, not geometry control: the strict scoped value remains in the bundled instance and quick-control contract, but disabled channels do not invoke the geometry graph. Test-local instrumentation proves all-disabled selected fields perform zero source conversions and zero native-stage calls; a mixed fixture proves only the enabled channel invokes its six stages.
- Source-weighted/uniform mode, shared/independent arrangement, density polarity/strength, response strength, minimum scale, boundary gap, exact `u64` seed, positive `NonZero` region output, and cancellation all remain delegated to the existing authorities.
- `src/site_distribution.rs` and `src/voronoi_geometry.rs` have no diff in this substage. No placement or tessellation algorithm was copied or changed.

## Verification

- `cargo fmt`
- `cargo test --locked weighted_voronoi --lib` — 13 passing focused Weighted/bundle tests.
- `cargo test --locked pattern_definition --lib` — 14 passing recipe-framework tests.
- `cargo test --locked` — 186 library tests, 48 binary/UI tests, and doc-tests all passing.
- `cargo check --locked` — passing.
- `cargo clippy --locked --all-targets -- -D warnings` — passing.
- `cargo fmt --check` — passing.
- `git diff --check` — passing.
- `git diff -- src/site_distribution.rs src/voronoi_geometry.rs` — empty.

No GTK launch, screenshot, or export artifact was required: this substage is non-UI and does not alter renderer/export dispatch. Existing preview/PNG/SVG parity tests passed in the full suite.

## Parent review targets and limitations

- Review `RecipeVoronoiDiagram` as transient construction provenance: it avoids changing the neutral geometry authority while retaining exact sites for the declared response-inset boundary.
- The assembled recipe helper is intentionally equivalence-only. Current renderer dispatch remains `generate_weighted_voronoi_cancellable` until a later accepted substage explicitly switches authority. Disabled channels are intentionally filtered at this orchestration boundary before recipe entry.
- The recipe helper does not create cache metadata; current cache fingerprints remain owned by the authoritative adapter. Canonical layer/region/geometry equality is asserted directly, and existing adapter fingerprint tests remain green.
- Invalidate this evidence if the 3A recipe ports, `WeightedVoronoiSettings` contract, field semantics, canonical region identity policy, distribution service, or Voronoi geometry service changes.
