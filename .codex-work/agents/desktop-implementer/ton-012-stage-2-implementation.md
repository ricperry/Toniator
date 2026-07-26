# TON-012 Stage 2 implementation evidence

- Repository absolute path: `/home/ricperry1/projects/Toniator`
- Git HEAD: `32022df28e6e746b44fb4f5db4427fd197ee2739`
- Relevant working-tree assumptions: before implementation, only `.gitignore`,
  `ISSUES.md`, `docs/ARTWORK_PIPELINE.md`, `docs/ARTWORK_PIPELINE_AUDIT.md`,
  untracked `.agents/`, `.codex/`, `AGENTS.md`, and `nextPrompt.txt` were
  dirty/untracked. They were preserved. This implementation additionally
  modifies only `src/artwork_pipeline.rs`, `src/render.rs`,
  `src/curve_render.rs`, and `src/svg_export.rs`.
- Producing agent: `desktop-implementer`
- Task: implement bounded TON-012 Stage 2 canonical prepared source and
  resolved channel-field pipeline; exclude Stage 3/4 and unrelated TON issues.
- Subsystems inspected: Stage 1 pipeline vocabulary and legacy projection;
  source decode/SVG rasterization; Shapes and Curves geometry; preview/PNG/SVG
  paths; cancellation and generation handling; existing renderer/export tests.
- Exact files and symbols changed:
  - `src/artwork_pipeline.rs`: `PreparedSource`, `ResolvedChannelField`,
    `ResolvedChannelFields`, `FieldBounds`, normalized sampler formulas,
    alpha policy handling, CMYK/RGB/scalar/Crosshatch assignment resolution,
    and focused unit tests.
  - `src/render.rs`: document-driven Shapes resolution/cache, document preview
    and output preparation, retained legacy façade adapter, and integration
    tests for cache/cancellation and Shapes/Curves consumption.
  - `src/curve_render.rs`: Curves consumes resolved fields for interpolated
    widths while retaining all path/lattice geometry and a legacy adapter.
  - `src/svg_export.rs`: SVG export uses the same document pipeline and
    prepared source/Curve geometry path.
- Important implementation decisions:
  - Source decode, SVG rasterization, long-edge cap, artboard bounds,
    cancellation checkpoints, and generation semantics stay in their existing
    owners. `PreparedSource` is an immutable normalized encoded-sRGB snapshot
    suitable for worker sharing.
  - Resolved fields keep scalar content and coverage separate. Values are
    finite/clamped `0..=1`, with endpoint epsilon `1e-6`; final consumers
    multiply them once.
  - `Preserve` uses alpha once as coverage; `Ignore` samples straight stored
    RGB even at alpha zero and gives full coverage; `Alpha` exposes alpha as
    content with full coverage, preventing double application.
  - `LegacyCurrentV1` preserves established reachable output: transparent
    samples are empty, CMYK/scalar ink remains opaque for nonzero alpha, and
    RGB/scalar RGB retains source-alpha coverage.
  - Automatic CMYK resolves C/M/Y/K; automatic RGB resolves R/G/B only.
    Active/All assignment uses stable `OutputChannelId`; Crosshatch remains the
    K/C/M/Y progressive compatibility assignment using legacy brightness.
  - Document rendering/export now treats `ArtworkPipelineSettings` as
    semantic authority. The `ValueMode` adapter remains only for old direct
    renderer entry points and compatibility tests.
- Existing abstractions reused: `decode_source`, `decode_svg`,
  `calculate_web_grid`, existing Shape and Curve placement/geometry,
  `CancellationToken`, `RenderGate`, `render_document_output_cancellable`,
  and atomic SVG/PNG export paths.
- Verified findings:
  - Shapes cache one resolved field set per matching grid across enabled
    channels; distinct grids retain distinct fields and cache hits checkpoint.
  - Curves use the corresponding resolved field for cubic sampling; they no
    longer independently sample/separate source pixels.
  - Preview, PNG (through document output), and SVG share document-driven
    geometry. Existing SVG raster parity and preview/export tests pass.
  - The direct contradictory-facade integration test proves Shapes and Curves
    render `Red + Ignore + Active K` source data despite a legacy CMYK facade.
- Commands run:
  - `cargo fmt --check`
  - `cargo check --locked`
  - `cargo test --locked artwork_pipeline::tests` (11 library tests)
  - `cargo test --locked render::tests` (36 library tests, including linked
    Curve tests selected by the module prefix)
  - `cargo test --locked curve_render::tests` (12 library tests)
  - `cargo test --locked` (98 library tests, 44 binary tests, 0 doc tests)
  - `cargo run --locked -- --demo --screenshot /tmp/toniator-ton-012-stage2.png`
  - `git diff --check`
- Artifacts produced: inspected GTK screenshot
  `/tmp/toniator-ton-012-stage2.png` (914 KiB; outside the worktree). No new
  repository binary artifacts were created.
- Known limitations:
  - Stage 3 UI does not yet expose the new Stage 2 sources/alpha policies.
  - Stage 4 preset redesign is intentionally untouched; legacy projection
    remains required for UI/preset compatibility actions.
  - Prepared sources/field caches are reused within a render operation and
    across same-grid enabled channels; there is no cross-request cache because
    current request/generation cancellation ownership must remain unchanged.
- Follow-up review targets: Stage 3 control vocabulary/accessibility and any
  Stage 4 preset schema work should remove the compatibility projection only
  after all UI/preset callers have migrated. Recheck alpha-policy visual UX
  once those controls exist.
- Documentation likely affected: `docs/ARTWORK_PIPELINE.md` and
  `docs/ARTWORK_PIPELINE_AUDIT.md` should describe the finalized Stage 2
  sampling contract after milestone review; they were intentionally not edited
  by this implementation task.
- Invalidation conditions: any change to the four changed source files, source
  preparation/resampling behavior, alpha policy decisions, assignment order,
  pipeline projection ownership, cancellation behavior, Git HEAD, or the
  dirty-file assumptions above requires revalidation.
- Timestamp: `2026-07-26`
