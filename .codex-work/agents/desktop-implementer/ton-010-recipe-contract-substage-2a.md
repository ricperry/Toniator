# TON-010 Substage 2A — declarative recipe contract implementation

- Repository: `/home/ricperry1/projects/Toniator`
- Timestamp: `2026-08-01T10:21:58-04:00`, corrected at `2026-08-01T10:26:14-04:00`
- Git HEAD / branch: `262c7e857446ded100d4a90fd23d651e52460665` / `TON-010-Stage5-Framework-Restart`
- Producing agent: `desktop_implementer`

## Working-tree assumptions

The checkout started at the stated HEAD with user-owned changes in `ISSUES.md`,
`assets/CMYKexpected.png`, `assets/RGBexpected.png`, `nextPrompt.md`, and
`.codex-work/evidence/ton-010-stage5-manual/`. They were not edited. This
substage adds the listed source changes plus this evidence file; no commit,
recipe bundle, preset/document version, UI workflow, renderer dispatch, or
site-distribution/Voronoi algorithm change was made.

## Implementation

- `src/pattern.rs`: replaced the closed `Copy` enum with a validated string
  newtype. The existing built-in values remain exactly `compat.shapes.v1`,
  `compat.curves.v1`, and `weighted-voronoi.v1` on the wire. Parsing now
  validates stable dotted IDs and registry validation remains responsible for
  deciding whether a legacy pattern is registered.
- `src/pattern_definition.rs` (new): format-v1 `PatternDefinition`, typed
  graph/ports/arguments, scoped parameters, quick controls, authoring layout,
  embedded SVG assets, deterministic JSON helpers, resource bounds, strict
  validation, and the bounded native `OperationRegistry`. Registered native
  descriptors cover placement, sampling, mapping, deformation, weighted
  Voronoi construction, and canonical geometry emission only.
- `Cargo.toml` / `Cargo.lock`: added maintained `sha2` 0.10 for in-process
  SHA-256 verification; every declared `sha256:<lowercase hex>` digest now
  must match the exact UTF-8 bytes of its embedded SVG. The explicit aggregate
  SVG bound is `MAX_TOTAL_EMBEDDED_SVG_BYTES`, equal to the intentional
  `MAX_PATTERN_ASSETS * MAX_EMBEDDED_SVG_BYTES` maximum.
- `src/lib.rs`: exposes the contract and registry types/helpers.
- `src/model.rs`, `src/preset.rs`, `src/persistence.rs`, `src/png_export.rs`,
  `src/svg_export.rs`, and `src/ui.rs`: mechanical non-`Copy` ID call-site and
  test adaptations; no behavior/format changes beyond accepting valid future
  IDs until the existing registry rejects them at its authority boundary.

The contract rejects unknown fields and unsupported versions, unknown or wrong
operation versions, invalid/missing/incompatible ports, duplicate destinations,
cycles, orphan nodes, invalid parameter/control/layout references, limits,
unsafe/external SVG references, malformed SVG, invalid digest syntax, and
digest/content mismatches, and unknown asset-digest references. Serialization uses fixed struct ordering and
ordered maps for stable UTF-8 JSON bytes.

## Verification

- `cargo fmt --check` — passed.
- `cargo test --locked` — passed: 171 library and 48 binary/UI tests.
- `cargo check --locked` — passed.
- `cargo clippy --locked --all-targets --all-features -- -D warnings` — passed.
- `git diff --check` — passed.
- Focused `pattern_definition::tests` covers IDs/roundtrip, strict parse,
  operation versions, typed ports, cycles, reachability, controls/scopes,
  asset safety, correct digest acceptance, malformed/uppercase digest rejection,
  unchanged-digest content tampering, and limits.

No GTK launch, screenshot, preview/export artifact, or runtime dispatch check
was required: 2A deliberately has no user-visible or executable-recipe change.

## Limitations and follow-up targets

Recipe execution, built-in recipe migration/bundling, UI authoring/import
paths, and renderer dispatch are explicitly deferred to later substages.
Review `PatternDefinition::validate` and the operation descriptor vocabulary
before treating external recipe files as a distribution/import workflow.

## Invalidation conditions

Invalidate this evidence if the pattern-ID persistence boundary, pattern
registry, `src/pattern_definition.rs`, serde JSON configuration, operation
registry vocabulary, SVG safety policy, SHA-256 dependency/version, relevant
source files, Git HEAD, or
the stated user-owned dirty state changes. This is a safe 2A handoff only; do
not begin 2B without explicit parent direction.
