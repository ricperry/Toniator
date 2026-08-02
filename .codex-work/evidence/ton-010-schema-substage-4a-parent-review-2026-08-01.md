# TON-010 schema — Substage 4A parent review

Date: 2026-08-01
Repository: `/home/ricperry1/projects/Toniator`
Git HEAD: `262c7e857446ded100d4a90fd23d651e52460665` (dirty worktree preserved)

## Accepted boundary

The current document format is now v9 and the current `.tntr` treatment
preset format is v6. All six bundled runtime presets carry v6. Header checks
reject v8 documents, v5 presets, and future v7 presets before semantic parsing
or rendering; no migration or fallback was added.

## Parent checks

- `persistence::tests::current_project_roundtrips_and_rejects_pre_release_versions`
  passed.
- `preset::tests::current_v6_rejects_obsolete_and_future_versions_strictly`
  passed.
- `preset::tests::every_runtime_bundled_preset_is_current_and_applicable`
  passed.
- `cargo test --locked` passed (244 library + 48 binary/UI tests).
- Release check, strict Clippy, format, diff checks, and bounded startup smoke
  passed.

## Limits

Custom recipe embedding, user library resolution, and the pattern editor are
not claimed here. Manual GNOME/Wayland acceptance remains pending.

## Invalidation

Invalidate if current version constants, bundled preset bytes, strict header
checks, or dirty-checkout assumptions change.
