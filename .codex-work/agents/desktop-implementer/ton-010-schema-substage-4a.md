# TON-010 schema — Substage 4A implementation evidence

Date: 2026-08-01
Repository: `/home/ricperry1/projects/Toniator`
Git HEAD inspected: `262c7e857446ded100d4a90fd23d651e52460665`
Producing agent: `desktop-implementer`

## Scope completed

Advanced the strict current-format boundary required before custom pattern
editor work:

- Toniator documents now write and accept only version `9`.
- `.tntr` treatment presets now write and accept only version `6`.
- The six runtime bundled presets were advanced from v5 to v6.
- Obsolete v8 documents and v5 presets are rejected at their current header
  boundary with the existing unsupported-pre-release error; no migration,
  defaulting, or compatibility parser was added. Future v7 presets are also
  rejected strictly.

Pattern state, current parameter schema/generator versions, artwork pipeline,
rendering, persistence semantics, preset scope semantics, and UI were not
otherwise altered.

## Exact files changed

- `src/model.rs` — `DOCUMENT_VERSION` 8 → 9.
- `src/persistence.rs` — v9 test names and explicit serialized/current-v8
  rejection evidence.
- `src/preset.rs` — `CURRENT_PRESET_VERSION` 5 → 6, current DTO/validator
  names, v6 test names, and explicit v5/future v7 rejection test.
- `assets/presets/Chunky Fingerprints.tntr`
- `assets/presets/ComicBook.tntr`
- `assets/presets/Motif Ladder.tntr`
- `assets/presets/Polygon Six.tntr`
- `assets/presets/Skinny Curve.tntr`
- `assets/presets/Tiled Stacked Motif Stress Test.tntr`
- `.codex-work/agents/desktop-implementer/ton-010-schema-substage-4a.md`

## Existing abstractions reused and decisions

- Reused `DocumentHeader` plus `DOCUMENT_FORMAT`/`DOCUMENT_VERSION` header
  validation and `PresetHeader` plus `CURRENT_PRESET_VERSION` validation.
- Reused the established current-only `CurrentPresetV*` DTO and strict nested
  field validators; renamed them v6 to prevent stale-current terminology.
- The version increment itself is the compatibility boundary. Existing strict
  checks occur before document/preset semantic application, so obsolete inputs
  cannot reach projection, normalization, or rendering work.

## Verification and artifacts

- Focused tests passed:
  - `persistence::tests::current_project_roundtrips_and_rejects_pre_release_versions`
  - `preset::tests::current_v6_rejects_obsolete_and_future_versions_strictly`
  - `preset::tests::every_runtime_bundled_preset_is_current_and_applicable`
- `cargo test --locked` — 244 library tests and 48 binary/UI tests passed.
- `cargo check --locked --release` — passed.
- `cargo clippy --locked --all-targets -- -D warnings` — passed.
- `cargo fmt --check` and `git diff --check` — passed.
- `timeout 12s cargo run --locked` — built and launched `target/debug/toniator`
  without a startup failure.

No screenshot, export, or generated image artifact applies to this strict
schema/preset boundary. Startup smoke is not manual GNOME/Wayland acceptance.

## Limitations, review targets, documentation, and invalidation

- No custom pattern editor, pattern library, schema fields for editor data,
  migration, adapter removal, or UI work is included.
- Existing v8 `.toniator` projects and v5 `.tntr` presets are intentionally
  unrecoverable by the current parser unless users recreate/export them in the
  current format; that strict rejection is the requested policy.
- Durable documentation likely affected: Stage 5 architecture/restart notes
  that still describe the v9/v6 gate as pending, plus any user-facing format
  compatibility guidance. Reconciliation is a separate documentation
  milestone.
- Invalidate this evidence if current document/preset versions, header parsing,
  bundled preset inventory, strict no-migration policy, HEAD, or working-tree
  assumptions change.

## Working-tree assumptions

The repository was materially dirty on the HEAD above from accepted TON-010
work and unrelated user edits. Those changes were preserved and not staged.
No reset, clean, commit, push, publication, deployment, or destructive
operation was performed.
