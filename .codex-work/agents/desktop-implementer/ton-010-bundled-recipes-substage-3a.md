# TON-010 Bundled Recipes — Substage 3A implementation evidence

- Timestamp: 2026-08-01T11:18:48-04:00
- Implementer scope: first immutable bundled Weighted Voronoi definition and validation-only production descriptor vocabulary. No native operation body, render dispatch, resource/library I/O, document/preset migration, UI, or algorithm change.
- Git HEAD assumption: `262c7e857446ded100d4a90fd23d651e52460665` on `TON-010-Stage5-Framework-Restart` before and after this work.
- Working-tree assumption: accepted/user work in `Cargo.toml`, `Cargo.lock`, `src/lib.rs`, `src/model.rs`, `src/pattern.rs`, `src/persistence.rs`, `src/png_export.rs`, `src/preset.rs`, `src/svg_export.rs`, `src/ui.rs`, `ISSUES.md`, `.codex-work/cache-index.md`, all previous declarative evidence, and the documentation-maintainer additions in `docs/` were present and preserved. The untracked declarative-contract sources from 2A–2C2 were extended in place.

## Changed files

- `assets/patterns/weighted-voronoi.v1.tnpattern`
  - Added the tracked immutable v1 source for `weighted-voronoi.v1` with Regions output, six atomic recipe stages, 10 fully bounded output-channel parameters, exact unsigned seed support, all arrangement/placement/polarity choices, enabled-channel state, authoring layout, and quick controls.
- `src/bundled_pattern_definitions.rs`
  - Added compile-time `include_bytes!` bundle loading through exactly `parse_tnpattern` and `PatternDefinitionRegistry`; exposes actionable parse/registry errors and has no filesystem, XDG, import, legacy metadata, or renderer fallback.
- `src/pattern_definition.rs`
  - Replaced provisional generic default descriptors with the production Weighted vocabulary: source sample, response map, site distribution, Voronoi construction, response inset, and region emission.
  - Added exact atomic descriptor input/output ports and parameter types for current Weighted settings; generic descriptor metadata now declares the region emitter capability.
  - Parent correction: response inset now emits `BoundaryDerivedRegionCells`, consumed by region emission, and the runtime value reuses `RegionPatternOutput`; `DeformedSites` remains reserved for truthful future point-set deformation only.
  - Mechanically updated contract/registry test fixtures for the production vocabulary and channel-scoped settings.
- `src/pattern_definition_registry.rs`
  - Reused the bundled loader in resolver fixtures so all registry behavior continues through the production descriptor parser.
- `src/lib.rs`
  - Exported bundle bytes, loader APIs, and bundled-load error type.
- `.codex-work/agents/desktop-implementer/ton-010-bundled-recipes-substage-3a.md`
  - This evidence entry.

## Decisions and reused abstractions

- Used a tracked compile-time asset rather than the GTK GResource manifest. `include_bytes!` produces immutable application bytes while preserving the editable declarative `.tnpattern` source; no Blueprint/GResource manifest changed.
- Reused strict v1 parsing, canonical serialization, SHA-256 fingerprinting, `PatternDefinitionRegistry` bundled provenance/immutability, `OutputChannelId` channel sets, and the accepted 2C2 descriptor model.
- The 3A descriptor graph mirrors the existing authoritative `weighted_voronoi.rs` stages but does not call it. The existing renderer remains authoritative until a later equivalence gate.
- Boundary-derived inset polygons are represented as existing `RegionPatternOutput` data at the typed intermediate boundary, not as deformed sites or a recipe-owned geometry duplicate.
- All current Weighted settings are output-channel scoped because the shipping `WeightedVoronoiChannelSettings` owns them per semantic channel. `enabled` governs emission, placement/distribution controls are bound to site distribution, and response/inset controls are bound to response inset.

## Verification

- `cargo fmt --check` — passed.
- `cargo test --locked bundled_pattern_definitions --lib` — passed (2 matching bundle tests).
- `cargo test --locked pattern_definition --lib` — passed (14 matching declarative/registry/bundle tests).
- `cargo test --locked` — passed (182 library tests, 48 application tests, 0 doc tests).
- `cargo check --locked` — passed.
- `cargo clippy --locked --all-targets -- -D warnings` — passed.
- `git diff --check` — passed.
- GResource/Blueprint compilation: not separately run because `resources/toniator.gresource.xml`, Blueprint sources, and `build.rs` were unchanged; the compile-time asset was validated by normal locked compilation and bundle tests.
- Runtime/GTK screenshot and preview/export validation: not run; no runtime/render/UI path changed.

## Limitations, review targets, and invalidation

- The registry has no production native implementations for these descriptors. Bundle validation is intentionally not render execution or equivalence proof.
- The new bundle is not wired to documents, presets, selector UI, library discovery, or renderer dispatch. Existing Weighted Voronoi rendering and persistence remain authoritative.
- Parent review should inspect the output-channel scope decision, numeric step precision relative to current UI increments, descriptor-stage names/ports, and compile-time asset versus future GTK resource packaging before authorizing operation bodies or integration.
- Durable documentation is already under separate documentation-maintainer ownership; no docs or tracker files were edited by this substage.
- Invalidate this evidence if the `.tnpattern` asset, bundle loader, descriptor vocabulary, instance contract, canonical-output capabilities, registry provenance, HEAD, or recorded dirty-worktree assumptions change; or if later work wires native execution, resources/library I/O, renderer/persistence/UI integration, or alters Weighted algorithm semantics.
