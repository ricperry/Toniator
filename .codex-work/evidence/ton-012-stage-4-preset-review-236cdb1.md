# TON-012 Stage 4 preset review evidence

- Repository: `/home/ricperry1/projects/Toniator`
- Git HEAD: `236cdb190a091029c1e7436d65716bf889b31010`
- Scope: independent preset/persistence review of the uncommitted Stage 4
  implementation, 2026-07-26.
- Reviewer: `test_reviewer` (read-only).

## Verified

- Explicit scope controls section parsing and candidate application.
- Candidate replacement is atomic and creates one undo entry; UI sync follows
  the apply and requests one preview on a changed load.
- All four runtime bundled presets are included in the production menu and
  parse/apply through production code.

## Findings sent for correction

- Nested unknown fields were not comprehensively rejected because serde could
  discard them inside treatment/settings/channel payloads.
- Active-channel omission needed explicit transition-matrix coverage.
- Treatment/channel isolation needed sentinel tests, especially same-kind and
  cross-kind treatment behavior.
- Minor follow-ups were Crosshatch scope coverage, broader bundled screenshot
  evidence, and an instrumented one-render-request check.

- Invalidation: changes to `src/preset.rs`, `src/model.rs`, `src/ui.rs`, the
  bundled assets, or the Stage 4 format contract.
