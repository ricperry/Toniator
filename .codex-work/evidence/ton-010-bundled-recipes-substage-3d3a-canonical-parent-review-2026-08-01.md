# TON-010 bundled recipes — Substage 3D3A canonical parent review

- Repository/HEAD: `/home/ricperry1/projects/Toniator` at dirty `262c7e8`
- Date: 2026-08-01
- Status: accepted canonical-equivalence slice; consumer parity still pending
- Producer: `desktop_implementer`; parent inspected the case matrix, custom
  geometry, assignment assertions, cancellation, tests, and evidence.

## Accepted findings

- Recipe-orchestrated Shapes output equals the retained oracle as a complete
  canonical structure and repeats deterministically across automatic RGB/CMYK,
  Value AllChannels, ActiveChannel, and Crosshatch compatibility assignments.
- Explicit fixtures cover opaque/translucent alpha, mixed disabled channels,
  every current primitive, shared and different per-channel nontrivial cubic
  paths, resolution/grid transforms, threshold/nonzero minimum/maximum size,
  local deformation, color, and opacity.
- Crosshatch layers use the single compatibility color while bundled recipe
  bytes contain no Crosshatch parameter. Disabled channels perform zero native
  and provider-cache work while their stable disabled layers remain present.
- Equal lattice dimensions share one provider resolution; distinct resolutions
  resolve independently. Pre-start and provider-triggered mid-execution
  cancellation return no canonical output; the pure executor installs no state.
- Live document dispatch remains compatibility=1 and recipe orchestration=0.

## Verification and remaining gate

- Parent full canonical matrix, deterministic cancellation, and disabled
  zero-work tests passed.
- Parent locked release check, strict all-target Clippy, format check, and diff
  check passed. Writer full suite passed: 205 library and 48 binary/UI tests.
- 3D3A as a whole remains pending only preview/PNG and established editable
  mark-SVG consumer parity with transparent/opaque export backgrounds.

Invalidate when Shapes oracle/orchestration, pipeline assignments, custom SVG
projection, provider caching, cancellation, canonical Marks semantics, HEAD,
or dirty state changes.
