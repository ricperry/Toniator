# TON-010 declarative recipe contract — Substage 2A parent review

- Repository: `/home/ricperry1/projects/Toniator`
- Timestamp: 2026-08-01
- Git HEAD: `262c7e857446ded100d4a90fd23d651e52460665`
- Branch: `TON-010-Stage5-Framework-Restart`
- Producing agent: `desktop_implementer`
- Parent review: inspected the contract, ID conversion, compatibility call-site
  edits, dependency change, tests, and writer evidence; reran focused contract
  tests and `git diff --check`.

## Working-tree boundary

The pre-existing user-owned `ISSUES.md` TON-021 draft, `nextPrompt.md`,
`assets/CMYKexpected.png`, `assets/RGBexpected.png`, and
`.codex-work/evidence/ton-010-stage5-manual/` remain preserved. This substage
adds the recipe-contract source and mechanical non-`Copy` ID adaptations plus
its evidence. It does not change renderer dispatch, built-in definitions,
document/preset versions, UI resources, or placement/Voronoi algorithms.

## Accepted findings

- `PatternId` is now an owned, validated dotted-string newtype. Existing IDs
  retain their exact serialized spellings, while syntactically valid future IDs
  reach the registry/definition authority instead of being rejected by an enum.
- `.tnpattern` format version 1 has strict serde structures, deterministic JSON
  serialization, typed operation ports and edges, a bounded DAG, scoped
  parameters, quick controls, authoring layout, and embedded SVG assets.
- Validation rejects unsupported format/recipe/operation versions, unknown
  operations, incompatible or missing ports, duplicate destinations, cycles,
  unreachable nodes, invalid parameter/control/layout references, resource
  overages, unsafe or externally referenced SVG, malformed SVG, invalid or
  duplicate digests, digest/content mismatches, and unknown asset references.
- Embedded SVG identity is content-addressed: SHA-256 is recomputed over the
  exact UTF-8 SVG bytes. Aggregate embedded SVG bytes are explicitly bounded.
- The native operation registry is a validation boundary only in 2A. Recipes do
  not execute, and current built-ins still use their existing paths.

## Verification

- Writer: `cargo fmt --check`, full `cargo test --locked` (171 library, 48
  binary/UI), `cargo check --locked`, strict all-target Clippy, and
  `git diff --check` all passed.
- Parent: `cargo test --locked pattern_definition::tests` passed 3 focused
  contract tests; `git diff --check` passed.

## Safe handoff and uncertainty

Substage 2A is accepted for the next bounded integration slice. Runtime recipe
execution, bundled definitions, layered library resolution, document/preset
embedding, UI/editor work, and schema version bumps remain unimplemented.
Human Stage 5 GNOME/Wayland, Krita-reference, and Inkscape Break Apart
acceptance also remains pending.

Invalidate this record if `PatternId`, `pattern_definition.rs`, the SHA-256 or
SVG safety policy, serde JSON handling, operation descriptors, the adapted
compatibility call sites, relevant dependencies, HEAD, or the recorded dirty
state changes.
