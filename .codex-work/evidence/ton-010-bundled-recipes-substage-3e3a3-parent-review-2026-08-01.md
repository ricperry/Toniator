# TON-010 bundled recipes — Substage 3E3A3 parent review

- Repository/HEAD: `/home/ricperry1/projects/Toniator` at dirty `262c7e8`
- Date: 2026-08-01
- Producer: `desktop_implementer`; parent reviewed consumer parity for the
  non-dispatched Curves orchestrator.

## Accepted findings

- Retained and recipe canonical Paths feed the same preview raster, canonical
  PNG, generic canonical-SVG boundary, and editable Curve-SVG byte seam.
  Consumers do not regenerate pattern geometry.
- Preview pixels and deterministic PNG bytes are exactly equal for CMYK, RGB,
  Crosshatch, transparent/white backgrounds, and semantic channel filtering.
- The new reusable `curve_svg_bytes_cancellable` seam is the sole editable
  Curve serializer used by the existing atomic file exporter. Retained and
  recipe bytes match exactly, parse as SVG, preserve editable path IDs/layers,
  clips, model blend modes, Crosshatch labels, and optional background order,
  with no image, mask, or obsolete cell-sizing construct.
- The public generic algebra SVG helper intentionally remains a no-op for Paths;
  Curves use the established editable Curve serializer rather than silently
  broadening that boundary.
- No-op fields remain consumer-neutral. Pre-cancelled consumers and over-64MP
  raster requests fail before work. Live Curve file export still records zero
  recipe orchestration calls.
- No live dispatch or preview/export runtime routing changed.

## Verification

- Parent `cargo test --locked svg_export::tests::bundled_curves_recipe_consumers_match_retained_canonical_paths_without_dispatch`: 1 passed.
- Parent `git diff --check` passed. Writer full suite passed: 241 library and
  48 binary/UI tests, locked release check, strict all-target Clippy, format,
  and diff checks.
- Writer launch smoke used an intentional timeout and is not human Stage 5
  GNOME/Wayland, Krita-reference, or Inkscape acceptance.

This closes non-dispatched Curves consumer parity. Invalidate when Curve SVG
presentation/file routing, canonical consumers, recipe output, live dispatch,
HEAD, or relevant dirty state changes.
