# TON-010 Stage 5 Framework Restart — Substage B implementation

- HEAD/branch: `87b4ce37d633181df485728cb903c4ff15b9470a`,
  `TON-010-Stage5-Framework-Restart`; preserved pre-existing untracked files:
  `nextPrompt.md` and pre-framework preservation evidence.
- Files changed: `src/lib.rs`, `src/model.rs`, `src/pattern.rs`, `src/preset.rs`,
  `src/render.rs`, `src/png_export.rs`, `src/svg_export.rs`, minimal exhaustive
  `src/ui.rs` support, `src/voronoi_geometry.rs`, new
  `src/weighted_voronoi.rs`, and Substage A/B cache records. `src/persistence.rs`
  only received exhaustive compatibility-fixture test arms.

## Decisions and reused abstractions

- Rebuilt the archived monolith as a thin adapter over the Substage A neutral
  services. Reused valid archived half-plane/inset semantics only through the
  neutral geometry owner; no archived renderer/model ownership was retained.
- Persisted settings belong only in `PatternDocumentState`. The render variant
  is derived dispatch. Weighted state is created on explicit selection, so
  existing current Shapes/Curves documents/presets retain their unchanged,
  strict two-instance shape; selected Weighted documents persist a validated
  third instance with generator version 2.
- Existing preview/PNG/SVG canonical consumers are reused. No alternate raster
  or SVG generator, global cache, stale-result path, or perimeter-border output
  was introduced. Metadata identifies source, resolved field, distribution,
  geometry, channel, and view consumption invalidation boundaries.

## Verification and handoff

- Passed formatting, locked check, diff check, 5 site-distribution tests, 4
  geometry tests, 5 Weighted integration/model/preset/persistence/parity tests,
  and 1 bundled-preset applicability test. No GTK launch/screenshot was run:
  no Weighted editor UI or visual layout was added in this adapter substage.
- Follow-up review targets: inspect source-response visual quality in CMYK/RGB
  graphical exports; decide the dedicated inspector/UI handoff separately; and
  assess cache-key integration only when a bounded result cache is scheduled.
- Documentation likely affected later: the durable TON-010 Stage 5 plan and
  pattern API/help material. They remain intentionally untouched.
- Invalidate this report if the listed source modules, archive reference,
  cancellation contract, HEAD, or working-tree assumptions change. Safe
  handoff: Substage B is complete; no broad cleanup or durable architecture
  documentation has started.
