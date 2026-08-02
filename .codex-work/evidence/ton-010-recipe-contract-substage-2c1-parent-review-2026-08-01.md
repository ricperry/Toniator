# TON-010 declarative recipe contract — Substage 2C1 parent review

- Repository: `/home/ricperry1/projects/Toniator`
- Timestamp: 2026-08-01
- Git HEAD: `262c7e857446ded100d4a90fd23d651e52460665`
- Branch: `TON-010-Stage5-Framework-Restart`
- Producing agent: `desktop_implementer`
- Parent review: inspected parameter/constraint wire types, scoped instance
  validation, deterministic helpers, exports, tests, and worker evidence;
  reran focused pattern-definition tests and `git diff --check`.

## Accepted findings

- Definition parameters now have stable keys, creator-facing label/help,
  explicit scope/type/default, and strict type-appropriate constraints.
- Declarative integer values and constraints use `u64`; `u64::MAX` round-trips
  exactly for existing Weighted Voronoi seed semantics.
- The separate v1 instance payload uses ordered lists so duplicate parameter or
  channel records remain detectable instead of last-write-wins JSON map data.
- Validation requires exact pattern-wide and supplied output-channel keys,
  matching scopes/types, finite and bounded numeric values, valid steps,
  declared choices, bounded text, known semantic channel IDs, known embedded
  asset digests, and bounded value/channel counts.
- New-current instance construction from definition defaults is explicit and
  separate from strict parsing. Existing or obsolete payloads are not silently
  defaulted or migrated.
- Canonical instance serialization sorts parameter and channel entries. No
  document/preset integration or TON-011 per-channel pattern selection exists.

## Verification

- Writer: full `cargo test --locked` (178 library, 48 binary/UI), formatting,
  locked check, strict all-target Clippy, and diff checks passed.
- Parent: `cargo test --locked pattern_definition::tests` passed 5 focused
  tests; `git diff --check` passed.

## Safe handoff and invalidation

Substage 2C1 is accepted. Graph execution, native operation implementations,
bundled definitions, schema persistence, filesystem library, and UI remain
later work. Invalidate if parameter/instance wire types, constraints, channel
identity parsing, asset validation, deterministic serialization, public
exports, HEAD, or recorded dirty state changes.
