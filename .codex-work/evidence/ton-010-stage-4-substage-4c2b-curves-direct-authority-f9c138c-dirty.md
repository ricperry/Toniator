# TON-010 Stage 4, Substage 4C2b — parent review

- Recorded: 2026-07-28
- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `f9c138c493a9d687b5300abddf14e78281f2ad63`
- Producing writer: `desktop_implementer` / `019faa4b-ec1b-7403-96b0-d16f31cb38ed`
- Parent reviewer: orchestrator

## Boundary and deliverable

4C2b migrated the remaining runtime Curves UI reads in `src/ui.rs`:
artboard sizing, direct path/color helpers, motif arrangement and overlay
geometry/visibility, and editing context. These paths read
`Document.pattern_state.curve_settings()` or `Document.artwork_pipeline`; all
direct writes remain routed through `DocumentEditor::set_curve_settings` via
the accepted 4C2a change. The writer stopped before 4D and did not change
Shapes, schemas/presets, custom-pattern workflows, or Weighted Voronoi.

## Parent review

Reviewed the report and current source. The only remaining `RenderVariant`
references in `src/ui.rs` are under the test module, where they intentionally
construct contradictory transient adapters and assert legacy projection
behavior. The runtime import is test-only (`#[cfg(test)]`); no production UI
path reads Curve selection or parameters from an adapter. The realized GTK
coverage checks authoritative artboard dimensions, path/color, motif state and
overlay, profile persistence, descriptor help, and pipeline-derived
Crosshatch context while the adapter is contradictory.

## Verification

- Parent review: `rg` confirmed no production `RenderVariant` references remain
  in `src/ui.rs`.
- Writer validation: full `cargo test --locked` — 138 library and 46 binary/UI
  tests passed; all-targets check and clippy, fmt check, and `git diff --check`
  passed.
- No manual GTK/Wayland visual/accessibility acceptance or screenshot is
  claimed.

## Safe handoff

Accepted as the 4C2b progress boundary. Stage 4D may now perform bounded
parent-reviewed runtime/regression validation and documentation reconciliation.
No Weighted Voronoi or Stage 5 work may begin during 4D.

