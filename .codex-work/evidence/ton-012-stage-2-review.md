# TON-012 Stage 2 review evidence

- Repository absolute path: `/home/ricperry1/projects/Toniator`
- Git HEAD: `32022df28e6e746b44fb4f5db4427fd197ee2739`
- Scope: independent regression review of the uncommitted Stage 2 runtime
  implementation.
- Reviewer: `test_reviewer` James, 2026-07-26
- Initial findings: SVG used legacy `Document.output_mode` for metadata/blend
  mode; resolved-field cache key was grid dimensions only; end-to-end alpha and
  overlapping-generation coverage was incomplete.
- Corrections made by parent: SVG metadata/blending now use authoritative
  `Document.artwork_pipeline.output_model`; a contradictory-facade SVG test was
  added; field cache keys include prepared generation/bounds, grid, source,
  alpha, output, assignment/payload, active channel, and enabled semantic
  channels; a semantic cache-separation assertion was added.
- Remaining bounded risk: the new alpha policies are not exposed by the current
  Stage 3-pending UI, and broad translucent-SVG workflow fixtures remain a
  follow-up. Canonical resolver tests cover their numerical semantics.
- Verification after correction: SVG tests 7 passed; cache-separation test
  passed; full required checks were rerun by the parent after the final diff.
- Invalidation: changes to Stage 2 runtime source, cache scope, export semantics,
  or tests.
