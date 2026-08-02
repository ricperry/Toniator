# TON-010 bundled recipes — Substage 3E3A1 parent review

- Repository/HEAD: `/home/ricperry1/projects/Toniator` at dirty `262c7e8`
- Date: 2026-08-01
- Producer: `desktop_implementer`; parent reviewed the non-dispatched Curves
  orchestrator, provider cache, equality tests, and live retained guard.

## Accepted partial boundary

- `execute_bundled_curves_recipe_cancellable` adapts authoritative typed
  settings and executes the common six-node recipe once per enabled semantic
  output channel, then merges exact one-layer canonical Paths results in
  retained order.
- Legacy-compatible assignment selects CMYK; other assignments select the
  pipeline output-model channels. Disabled channels are skipped before source
  resolution or native work and omitted from final geometry.
- The source provider resolves complete semantic field sets and caches them by
  requested dimensions, ordered enabled IDs, source/resolved generations. It
  preserves RGB/CMYK identity and produces one miss for shared grids and
  separate misses for distinct resolution grids.
- Legacy monochrome/Crosshatch color is applied after recipe emission as
  external assignment policy. No pipeline or Crosshatch state entered the
  definition or native operations.
- Representative CMYK/RGB, both-layout, shared/per-channel motif, disabled,
  and Crosshatch cases are byte-for-structure equal to retained canonical
  geometry and deterministic across repeats.
- Production `src/render.rs` still uses retained Curves. The live-document test
  records zero Curves orchestration calls.

## Verification

- Parent `cargo test --locked curves_native --lib`: 21 passed.
- Parent `cargo test --locked live_curves_document_render_stays --lib`: 1
  passed. Parent static production scan and `git diff --check` passed.
- Writer full suite passed: 236 library and 48 binary/UI tests, locked release
  check, strict all-target Clippy, format, and diff checks.
- Writer launch smoke used an intentional timeout and is not human Stage 5
  GNOME/Wayland, Krita-reference, or Inkscape acceptance.

This is a non-dispatched orchestration foundation. Invalidate when Curves
recipe/adapter/runtime, output assignment/order, provider/cache generations,
merged geometry, consumer parity, live dispatch, HEAD, or dirty state change.
