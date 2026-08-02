# TON-010 declarative recipe contract — Substage 2B parent review

- Repository: `/home/ricperry1/projects/Toniator`
- Timestamp: 2026-08-01
- Git HEAD: `262c7e857446ded100d4a90fd23d651e52460665`
- Branch: `TON-010-Stage5-Framework-Restart`
- Producing agent: `desktop_implementer`
- Parent review: inspected registry source precedence, fingerprints,
  diagnostics, errors, tests, exports, and worker evidence; reran the focused
  registry tests and `git diff --check`.

## Accepted findings

- `PatternDefinitionRegistry` validates already-materialized definitions and
  fingerprints canonical `.tnpattern` bytes before resolution.
- Stable-ID lookup and listing are deterministic. Same-ID/same-content entries
  deduplicate while retaining provenance.
- Bundled content is immutable. Differing definitions within the same layer
  are fatal and actionable rather than insertion-order dependent.
- For a non-bundled custom ID, project-embedded differing content is
  authoritative over local user-library content so a portable project remains
  reproducible. The shadowed local source/fingerprint and authoritative project
  source/fingerprint remain available as deterministic typed diagnostics; no
  local definition is silently substituted.
- Missing and invalid definitions have distinct typed errors. No legacy
  metadata fallback, file I/O, persistence, UI, or recipe execution is present.

## Verification

- Writer: focused registry tests, full `cargo test --locked` (176 library, 48
  binary/UI), formatting, locked check, strict all-target Clippy, and diff
  checks passed.
- Parent: `cargo test --locked pattern_definition_registry::tests` passed 5;
  `git diff --check` passed.

## Safe handoff and invalidation

Substage 2B is accepted. Bundled recipe resources, operation execution,
document/preset embedding, XDG library I/O, and UI remain later bounded work.
Invalidate if the 2A contract, canonical serialization, fingerprinting,
registry precedence/diagnostics, public exports, HEAD, or recorded dirty state
changes.
