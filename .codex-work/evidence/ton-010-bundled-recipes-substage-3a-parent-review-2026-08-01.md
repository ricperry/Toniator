# TON-010 bundled recipes — Substage 3A parent review

- Repository: `/home/ricperry1/projects/Toniator`
- Date: 2026-08-01
- HEAD: `262c7e857446ded100d4a90fd23d651e52460665`
- Producer: `desktop_implementer`; parent inspected bundled JSON, loader,
  descriptors, tests, evidence, and renderer/persistence non-integration.

## Accepted findings

- `assets/patterns/weighted-voronoi.v1.tnpattern` is an immutable compile-time
  bundled definition parsed through the strict common v1 loader and definition
  registry, with no legacy/filesystem fallback.
- Its six typed stages mirror source sampling/response, existing site
  distribution, existing Voronoi construction, boundary-derived inset, and
  canonical region emission. All ten current per-channel settings have bounded
  types/scopes/defaults; exact seed remains advanced rather than a quick slider.
- Parent correction replaced an inaccurate `DeformedSites` inset port with
  `BoundaryDerivedRegionCells`, backed by existing `RegionPatternOutput` rather
  than a duplicate geometry model. `DeformedSites` remains point-set-only.
- RGB and CMYK default instances validate, canonical serialization/fingerprint
  are deterministic, provenance is bundled, and conflicting content cannot
  override the bundled ID.
- No native operation body, render dispatch, persistence, UI, library I/O, or
  placement/Voronoi algorithm changed.

## Verification

- Writer full suite: 182 library and 48 binary/UI tests; fmt, check, strict
  Clippy, and diff checks passed.
- Parent: bundled-definition tests passed 2 before and after the typed-port
  correction; Weighted-focused tests passed 8;
  `git diff --check` passed.

Invalidate if the bundled bytes/loader, production descriptors, parameter
contract, registry provenance, current Weighted settings, HEAD, or dirty state
changes.
