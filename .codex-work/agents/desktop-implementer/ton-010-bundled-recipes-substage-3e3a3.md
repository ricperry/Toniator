# TON-010 bundled recipes — Substage 3E3A3 implementation evidence

Date: 2026-08-01
Repository: `/home/ricperry1/projects/Toniator`
Git HEAD inspected: `262c7e857446ded100d4a90fd23d651e52460665`
Producing agent: `desktop-implementer`

## Task and subsystems inspected

Task: prove non-dispatched Curves recipe consumer parity against the retained
oracle without changing live dispatch.

Inspected and used:

- `src/curves_native.rs`: accepted
  `execute_bundled_curves_recipe_cancellable` candidate.
- `src/curve_render.rs`: retained
  `generate_curve_geometry_for_pipeline` oracle and Paths raster consumer.
- `src/render.rs`: `render_canonical_pattern_output_cancellable` and the
  existing live-Curves non-dispatch guard.
- `src/png_export.rs`: `canonical_pattern_png_bytes[_cancellable]`.
- `src/svg_export.rs`: generic canonical SVG boundary and Curve file serializer.
- 3E3A1/3E3A2 evidence, validated against the same HEAD and materially dirty
  shared worktree before use.

## Scope completed and verified findings

Added one named consumer-parity test covering the same already-generated
canonical `Paths` objects from retained and recipe producers; no consumer
regenerates the pattern or uses an alternate renderer.

- Representative modern CMYK, modern RGB, and legacy Crosshatch cases use an
  alpha prepared source and compare retained and recipe `CanonicalPatternOutput::Paths` exactly.
- For transparent and white/opaque backgrounds, and unfiltered plus one
  semantic channel filter per case, preview raster pixels are exactly equal.
  Transparent outputs retain alpha and white outputs are fully opaque.
- Canonical PNG bytes are exactly equal and deterministic; decoded PNG pixels
  are exactly the preview raster produced from the same canonical object.
- The generic `canonical_pattern_svg_bytes_cancellable` remains the
  region/network algebra serializer. For Paths it has deterministic identical
  outer-document bytes but deliberately no editable `-curve-` geometry. This
  is an existing consumer boundary, not a byte-equivalent Curves export seam.
- Factored the existing Curve document serializer into
  `curve_svg_bytes_cancellable` plus `CurveSvgPresentation`; the file exporter
  now atomically writes those exact bytes. Retained and recipe Curve SVG bytes
  are exactly equal and deterministic, parse in `usvg`, retain editable path
  IDs/layer identity, CMYK/RGB blend mode and Crosshatch labels, artboard clip,
  optional named background ordering, and no source image/mask/obsolete
  cell-sizing construct.
- A live Curve SVG file-export assertion proves the emitted file equals the
  established bytes seam and still records zero Curves recipe orchestration
  calls. Production `src/render.rs` stays non-dispatched.
- `show_background` and `tile_spacing` no-ops leave canonical output, preview
  pixels, and deterministic PNG bytes unchanged.
- Pre-cancelled raster, PNG, Curve-SVG, and generic-SVG operations fail; render
  and PNG reject an over-64-megapixel request before allocation.

Curves Paths have no subtractive geometry. Genuine subtractive masks remain a
region/composite canonical-SVG concern and were not invented for Curves; the
test instead proves the applicable transparent/opaque export-background split.

## Exact files changed

- `src/svg_export.rs`
  - narrow reusable Curve SVG bytes/presentation seam with unchanged file-export routing
  - consumer parity test
- `.codex-work/agents/desktop-implementer/ton-010-bundled-recipes-substage-3e3a3.md`

No change was made to live Curves document dispatch, preview/export runtime
routing, persistence, schema, presets, UI, or Stage 6. No image artifact was
created because consumer evidence is byte/pixel/SVG-structure based and the
production Curves path remains intentionally retained.

## Important implementation decisions and reused abstractions

- Reused the canonical renderer, canonical PNG encoder, retained-oracle
  geometry, accepted Curves orchestrator, `CurveGeometry::CurveOutline` SVG
  serialization, `write_export_background`, and the established atomic write
  path. The extracted bytes helper removes serialization duplication but keeps
  output bytes and routing unchanged.
- The extractor takes explicit presentation authority (title, output model,
  export background, Crosshatch compatibility) so tests can serialize an
  already-produced canonical object without consulting document pattern state.
- The documented generic-SVG limitation is intentionally explicit: extending
  the public algebra serializer to route Curves would broaden consumer behavior
  beyond this non-dispatch substage.

## Commands and artifacts

- Focused:
  - `cargo test --locked svg_export::tests::bundled_curves_recipe_consumers_match_retained_canonical_paths_without_dispatch`
  - `cargo test --locked curves_native::tests::bundled_orchestrator_matches_retained_complete_canonical_recipe_matrix`
  - `cargo test --locked render::tests::live_curves_document_render_stays_on_the_retained_pipeline`
- Full: `cargo test --locked` — 241 library tests and 48 binary/UI tests passed.
- `cargo check --locked --release` — passed.
- `cargo clippy --locked --all-targets -- -D warnings` — passed.
- `cargo fmt --check` and `git diff --check` — passed.
- `timeout 12s cargo run --locked` — built and launched `target/debug/toniator`
  with no startup failure.

No GNOME screenshot/manual visual acceptance, preview/PNG file artifact, or
exported SVG fixture was retained. This was test-only parity work; the startup
smoke is not manual GNOME/Wayland acceptance.

## Uncertainty, review targets, and invalidation

- Review the new Curve SVG bytes helper for exact presentation compatibility if
  future work changes titles, labels, backgrounds, or output-model policies.
- Consumer parity does not authorize live recipe dispatch. Consumer-dispatch
  work must preserve the zero-invocation live guard until that stage explicitly
  changes it, then add actual preview/PNG/SVG artifacts.
- Durable documentation likely affected: Stage 5 architecture/recipe-contract
  material. Milestone documentation reconciliation remains separate.
- Invalidate this entry if Curves recipe/retained output, canonical consumer
  contracts, SVG presentation rules, cancellation/size limits, renderer
  routing, HEAD, or the dirty-worktree assumptions change.

## Working-tree assumptions

The repository was materially dirty on the HEAD above from accepted TON-010
work and unrelated user edits. Those edits were preserved and not staged. No
reset, clean, commit, push, publication, deployment, or destructive operation
was performed.
