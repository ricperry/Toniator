# TON-010 bundled recipes — Substage 3D3A complete parent review

- Repository/HEAD: `/home/ricperry1/projects/Toniator` at dirty `262c7e8`
- Date: 2026-08-01
- Producer: `desktop_implementer`; parent reviewed orchestration, exhaustive
  oracle equality, cancellation/cache behavior, consumer seam, and evidence.

## Accepted findings

- The non-dispatched bundled Shapes executor owns enabled-channel recipe
  execution and recipe-driven semantic field requests, with stable all-channel
  layer assembly and external Crosshatch assignment.
- Complete canonical structures equal the retained renderer across RGB/CMYK,
  AllChannels/ActiveChannel/Crosshatch, alpha, all primitives, shared and
  independent cubic motifs, transforms, response, styling, and disabled cases.
- Cache reuse/distinct-resolution behavior and zero disabled provider/native
  work are explicit. Pre-start and provider-triggered cancellation return no
  canonical result; the pure executor installs no state.
- Recipe-produced RGB/CMYK preview pixels equal decoded deterministic PNG for
  transparent and white backgrounds. Preview Surface and Export Background
  remain separate.
- The established mark SVG formatter now has one cancellation-aware bytes seam;
  file export atomically writes those bytes. Recipe and oracle MarkSets produce
  byte-identical, `usvg`-parseable editable SVG with stable layers, Screen or
  Multiply blends, cubic paths, no raster images, current no-clip behavior,
  and explicit optional background separation.
- Live Shapes document rendering remains compatibility=1 and recipe=0; 3D3A
  does not switch dispatch.

## Verification

- Parent canonical matrix, cancellation, disabled-work, RGB/CMYK PNG, editable
  SVG, and file-export seam regressions passed.
- Parent locked release check, strict all-target Clippy, format check, and diff
  check passed. Writer full suite passed: 208 library and 48 binary/UI tests.

3D3B may now switch the live Shapes branch, retaining the compatibility path
only as a test oracle until final adapter cleanup. Invalidate when Shapes
orchestration/oracle, canonical rendering, mark SVG serialization, presentation
metadata, dispatch, HEAD, or dirty state changes.
