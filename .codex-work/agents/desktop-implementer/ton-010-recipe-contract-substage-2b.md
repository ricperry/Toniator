# TON-010 Substage 2B — definition resolution implementation

- Repository: `/home/ricperry1/projects/Toniator`
- Timestamp: `2026-08-01T10:32:15-04:00`, corrected at `2026-08-01T10:35:31-04:00`
- Git HEAD / branch: `262c7e857446ded100d4a90fd23d651e52460665` / `TON-010-Stage5-Framework-Restart`
- Producing agent: `desktop_implementer`

## Working-tree assumptions

2A source changes and its evidence were already present and remain accepted.
The checkout also contained user/parent-owned `ISSUES.md`, asset PNGs,
`nextPrompt.md`, manual evidence, `.codex-work/cache-index.md`, and the 2A
parent-review evidence; none were edited by this substage. No commit, bundled
recipe file, I/O/XDG library path, project/document/preset schema change, UI,
or runtime recipe execution was started.

## Implementation

- `src/pattern_definition_registry.rs` (new): immutable in-memory
  `PatternDefinitionRegistry`, source/provenance enum, SHA-256 content
  fingerprint, resolved record, and typed errors.
- `src/lib.rs`: exports 2B registry APIs.

`build(bundled, user_library, project_embedded)` validates every supplied
definition through 2A canonical serialization before hashing its canonical
`.tnpattern` bytes. IDs are stored in a `BTreeMap`, so lookup/list order is
stable. Matching ID/content definitions deduplicate while retaining all
provenance. A project-embedded custom definition with differing user-library
content becomes the resolved authoritative definition, and its
`PatternDefinitionResolutionDiagnostic` records the ID, shadowed user source
and fingerprint, and authoritative project source and fingerprint; the
registry lists those diagnostics deterministically for later actionable UI.
Differing content against a bundled definition remains
`BundledDefinitionImmutable`, and differing duplicate definitions in the same
source layer remain `Conflict`. Missing lookup and invalid input are separately
actionable typed errors. No legacy `PatternRegistry` metadata is consulted.

## Verification

- Focused `cargo test --locked pattern_definition_registry::tests --lib` — 5 passed.
- `cargo test --locked` — passed: 176 library and 48 binary/UI tests.
- `cargo fmt --check` — passed.
- `cargo check --locked` — passed.
- `cargo clippy --locked --all-targets --all-features -- -D warnings` — passed.
- `git diff --check` — passed.

Focused coverage proves project authority over matching and differing user
content, same-content dedupe/provenance, deterministic surfaced diagnostics,
bundled immutability, fatal same-layer conflicts, missing/invalid definitions,
deterministic ID order, and stable/different canonical SHA-256 fingerprints.

## Limitations and follow-up targets

The registry accepts already-materialized `PatternDefinition` values only;
filesystem/XDG discovery, project persistence, conflicting-ID rename/import,
bundled `.tnpattern` resources, selector UI, and operation execution remain
future work. Review source precedence and error handling before adding those
loading workflows.

## Invalidation conditions

Invalidate this evidence if the 2A definition/serialization/digest contract,
`src/pattern_definition_registry.rs`, `sha2`, public exports, source precedence
rules, Git HEAD, or stated dirty-worktree assumptions change. This is the 2B
safe boundary; do not begin a later substage without explicit parent handoff.
