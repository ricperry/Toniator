# TON-010 bundled recipes — Substage 3D3B parent review

- Repository/HEAD: `/home/ricperry1/projects/Toniator` at dirty `262c7e8`
- Date: 2026-08-01
- Producer: `desktop_implementer`; parent inspected live dispatch, retained
  oracle compilation, consumers, tests, and evidence.

## Accepted findings

- The sole live `RenderVariant::WebShapeV1` branch now reads authoritative
  `PatternDocumentState` Shapes settings and returns the bundled recipe
  executor's canonical output directly.
- Real RGB and CMYK document generations prove recipe orchestration=1 and the
  retained whole Shapes generator=0. The retained generator and placement/
  primitive helpers compile only under `cfg(test)`; its production re-export
  is removed. Curves' still-needed pipeline facade remains production code.
- Preview, canonical PNG, and established mark SVG/file export consume the one
  live canonical Marks result. Live custom cubic SVG remains editable and the
  established bytes seam is unchanged.
- Crosshatch remains external output-assignment compatibility. `RenderVariant`
  remains only the temporary family selector pending Curves/schema conversion.

## Verification

- Parent live RGB/CMYK dispatch, live SVG-file seam, 11 Shapes-native tests,
  and 17 SVG/export tests passed.
- Parent locked release check, strict all-target Clippy, format check, and diff
  check passed. Writer full suite passed: 208 library and 48 binary/UI tests.
- Writer performed a brief noninteractive Wayland startup smoke with timeout;
  this is not the pending human Stage 5 pointer/focus/screen-reader/reference
  acceptance and no screenshot was captured.

Invalidate when Shapes document dispatch, pattern-state authority, bundled
executor/provider, retained oracle boundary, canonical consumers, HEAD, or
dirty state changes.
