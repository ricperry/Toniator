# TON-010 Recipe Contract — Substage 2C1 implementation evidence

- Timestamp: 2026-08-01T10:45:47-04:00
- Implementer scope: declarative parameter and instance validation only. No recipe execution, bundled-resource loading, document/preset schema, I/O, UI, or algorithm changes.
- Git HEAD assumption: `262c7e857446ded100d4a90fd23d651e52460665` on `TON-010-Stage5-Framework-Restart` before and after this work.
- Working-tree assumption: the pre-existing tracked modifications in `Cargo.toml`, `Cargo.lock`, `src/lib.rs`, `src/model.rs`, `src/pattern.rs`, `src/persistence.rs`, `src/png_export.rs`, `src/preset.rs`, `src/svg_export.rs`, `src/ui.rs`, `ISSUES.md`, and `.codex-work/cache-index.md`, plus existing untracked 2A/2B/manual/assets/prompt files, belong to prior accepted or user work. They were preserved. `src/pattern_definition.rs` and `src/pattern_definition_registry.rs` are existing untracked 2A/2B work extended in place.

## Changed files

- `src/pattern_definition.rs`
  - Extended `PatternParameterDefinition` with stable key, creator-facing label/help, explicit type-matched constraints, and validated choices/assets.
  - Changed declarative integer literals to `u64`, preserving `u64::MAX` exactly through serde and validation.
  - Added strict, separate `PatternInstanceParameters` payload types, default construction for a current definition plus caller-supplied active channels, complete scope/type/constraint/channel/asset/resource validation, and deterministic instance parse/serialize helpers.
  - Added focused tests for finite/bounded/step values, `u64::MAX`, choices/text, scope mismatch, missing/unknown/duplicate values and channels, asset references, strict unknown fields, resource-adjacent validation, and deterministic round trips.
- `src/pattern_definition_registry.rs`
  - Mechanically updated the 2B definition fixture for the strict parameter fields/number constraints.
- `src/lib.rs`
  - Re-exported the v1 parameter constraints, instance payload, errors, limits, and parse/serialize APIs.
- `.codex-work/agents/desktop-implementer/ton-010-recipe-contract-substage-2c1.md`
  - This evidence entry.

## Decisions and reused abstractions

- Reused the 2A strict serde boundary (`deny_unknown_fields`), `PatternId`, `OutputChannelId` stable-ID parser, asset SHA-256 registry, `PatternDefinition::validate`, and ordered `BTreeMap` definition serialization.
- Instance values are ordered lists, not JSON maps, so duplicate keys and duplicate output-channel records cannot be silently overwritten during deserialization.
- The instance payload has pattern-wide and per-output-channel value sections only. It deliberately does not model per-channel artwork selection (TON-011 boundary).
- Existing payloads are never migrated or defaulted. Defaults are available solely when deliberately constructing a new current-v1 instance from a validated definition and explicitly supplied channels.
- `u64` is used directly for integer literals and integer bounds/steps; no f64/i64 seed conversion is introduced.

## Verification

- `cargo fmt --check` — passed.
- `cargo test --locked pattern_definition --lib` — passed (10 matching tests).
- `cargo test --locked` — passed (178 library tests, 48 application tests, 0 doc tests).
- `cargo check --locked` — passed.
- `cargo clippy --locked --all-targets -- -D warnings` — passed.
- `git diff --check` — passed.
- Runtime/GTK screenshot: not run; this is a non-UI, non-runtime declarative contract change with no application wiring.
- Exports/artifacts: none produced; no graphics/export path changed.

## Limitations, review targets, and invalidation

- This validates recipe-ready values but does not execute recipe graphs or bind this new payload to documents/presets/resources; those are later authorized substages.
- Per-output-channel completeness is enforced for each supplied valid channel. Selecting active artwork channels remains intentionally outside this payload until TON-011.
- Parent review should inspect the public schema naming, v1 wire shape, and the `u64` integer decision before authorizing any execution or persistence integration.
- Durable documentation likely affected only after this contract becomes wired into a user-visible authoring/persistence milestone; no documentation was changed here.
- Invalidate this evidence if `src/pattern_definition.rs`, `src/pattern_definition_registry.rs`, or `src/lib.rs` changes; if HEAD or the dirty-worktree assumptions above change; or if later work introduces execution, resource loading, document/preset, UI, or TON-011 selection semantics.
