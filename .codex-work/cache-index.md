# Toniator evidence cache index

This index points agents to reusable evidence under `.codex-work/`. It is
checkout-aware and must not be treated as authoritative over current files.

Add one entry per reusable cache record with:

- Cache key and relative entry path
- Repository absolute path, Git HEAD, and relevant dirty files
- Producing agent, task/subsystem, and timestamp
- Validity status or last validation
- Short scope note and invalidation conditions

Read the linked entry and validate it against the current checkout before use.
The parent thread records read-only-agent updates here after persisting them.

## Current entries

- `ton-010-preservation-checkpoint-audit-2026-08-02` — authoritative current
  completion-status audit on dirty base HEAD
  `262c7e857446ded100d4a90fd23d651e52460665`. Bundled Shapes, Curves, and
  Weighted Voronoi plus embedded custom Shapes definitions execute through the
  recipe runtime, with strict document v9 / `.tntr` v6 persistence. TON-010 is
  not complete: authoring is Shapes-specific and hard-coded by preset index,
  Save As is write-only in the UI, the layered application registry and graph
  editor are absent, compatibility dispatch remains, Stage 6 proof recipes are
  incomplete, and human Stage 5 gates remain open. Earlier 2026-08-01 slice
  entries are implementation history, not current closeout authority.
  Evidence:
  `evidence/ton-010-preservation-checkpoint-audit-2026-08-02.md`. Invalidate
  after changes to `src/{ui,render,model,pattern_definition,pattern_definition_registry,shapes_native,shapes_recipe,curves_native,curves_recipe,weighted_voronoi}.rs`,
  bundled pattern assets, persistence versions, or the referenced docs.

- `ton-010-channel-settings-pattern-presets-2026-08-01` — on dirty HEAD
  `262c7e857446ded100d4a90fd23d651e52460665`, Channel Settings now places the
  edit-channel selector below Visible Inks, exposes an editable Channel HEX
  value field, and propagates coverage/detail/color and site-constructor edits
  into custom embedded recipe instances. The visible pattern selector now
  installs X/Y, triangular 60-degree, math-function, and random-dispersion
  recipes; site density changes lattice population and triangular placement is
  native. Spiral is a dedicated centered Archimedean connected path with curve
  spacing controlling turn pitch; it extends past the rectangular corners for
  canonical artboard clipping, ignores channel sampler overrides for its site
  topology, and splits network paths after thresholded samples instead of
  bridging gaps. Custom embedded mark/network recipes now project the active
  RGB/CMYK channel controls and HEX entry; stale deferred GTK syncs are ignored,
  connected RGB layers use Screen blending, square-wave deformation grows
  symmetrically around the canvas center, and editor-authored rotated grids wrap
  sites into the artboard while compatibility geometry keeps its parity seam.
  Curve and Math Function editor selections now default to connected/full-curve
  geometry without rewriting saved drafts on reopen.
  Connected network outputs now carry one smooth variable-width cubic contour
  per continuation run; raster and SVG share that filled outline, while
  topology edges remain inspectable and threshold gaps never reconnect.
  Spiral sites are sampled by deterministic Archimedean arc length in both the
  production recipe and editor preview, avoiding uniform-radian center
  oversampling while retaining corner-overflow clipping.
  Full tests (258 library, 54 binary/UI),
  export exercises for all math and random presets, strict Clippy/format checks,
  and a GTK screenshot launch passed. The pattern dialog also releases its
  AlertDialog extra child on close, allowing repeated editing without parent
  assertions. Human
  GNOME/Wayland acceptance remains pending. Evidence:
  `evidence/ton-010-channel-settings-pattern-presets-2026-08-01.md`.

  Latest random-placement follow-up adds the channel-scoped uniform-to-source
  mark-size response slider in Channel Settings Advanced, verifies
  sampling-detail-driven random site counts,
  preserves weighted clustering by avoiding a second independent offset, and
  tests uniform versus source-responsive extents. Random placement now maps
  the neutral Grid sampler to source-weighted sites; explicit Uniform Random
  remains available per channel. The latest complete suite is
  261 library tests and 56 binary/UI tests. Revalidate after changes to
  `src/ui.rs`, `src/model.rs`, `src/pattern_definition.rs`,
  `src/shapes_recipe.rs`, `src/shapes_native.rs`, the bundled Shapes recipe,
  or the pattern-editor Blueprint; Sampling Detail now also scales the native
  triangular-grid population, the reported Random · Gaussian channel edit is
  covered by a focused regression, and the realized UI regression proves the
  selected-channel control changes canonical mark count. Human
  slider/clustering inspection remains pending.

- `ton-010-pattern-editor-placement-controls-2026-08-01` — on dirty HEAD
  `262c7e857446ded100d4a90fd23d651e52460665`, Pattern Editor now exposes
  placement, grid/curve/math/random controls, dispersion, point definition,
  render/connection choices, jitter, and local curve authoring with irrelevant
  controls disabled. Custom embedded recipes return to the normal channel
  styling stack so channel fill/opacity/rotation/mark plus sampler and seeds
  remain reachable after Apply/Save As. Connected Points now emits canonical
  network nodes/edges through the bounded native operation. Full tests,
  realized GTK regression, strict Clippy/release/format checks, and a GTK
  screenshot launch passed; manual GNOME acceptance remains pending. Evidence:
  `evidence/ton-010-pattern-editor-placement-controls-2026-08-01.md`.

- `ton-010-pattern-editor-ia-preview-2026-08-01` — on dirty HEAD
  `262c7e857446ded100d4a90fd23d651e52460665`, Pattern Editor no longer exposes
  mark or channel sampling controls; it now provides a neutral live pattern
  preview, a Cancel-safe local curve-shape editor, persisted authored curve
  paths, and truthful curve/mark terminology. Legacy main-window curve
  authoring is hidden while runtime compatibility remains wired. Full tests,
  strict Clippy/release/format checks, and a real GTK screenshot launch passed;
  GNOME/Wayland modal interaction remains pending. Evidence:
  `evidence/ton-010-pattern-editor-ia-preview-2026-08-01.md`.

- `ton-010-pattern-control-boundary-2026-08-01` — on dirty HEAD
  `262c7e857446ded100d4a90fd23d651e52460665`, Pattern Settings now presents a
  single preset selector and structural editor entry point while Channel
  Settings owns channel scope, unified/per-channel seeds, and channel-targeted treatment panels. Legacy
  mode buttons are hidden, the output model remains the RGB/CMYK mode selector,
  and the editor persists a typed Sine/Square/Spiral curve-function parameter.
  Full tests and a real GTK screenshot launch passed; manual GNOME/Wayland
  interaction and broader per-channel recipe/site-constructor work remain
  pending. Evidence:
  `evidence/ton-010-pattern-control-boundary-2026-08-01.md`.

- `ton-010-pattern-editor-channel-boundary-2026-08-01` — on dirty HEAD
  `262c7e857446ded100d4a90fd23d651e52460665`, Pattern Editor now owns only
  structural grid/mark/deformation state while main Channel Settings owns
  per-channel sampler, random seed, and weighted-site influence. Embedded
  recipes receive those authoritative channel values; editor numeric defaults
  and nonzero jitter use continuous bounded validation; editor random sampling
  reuses the neutral distribution service without the old 8,192 editor cap.
  Focused/full tests, strict Clippy/release/format checks, and real GTK
  resource/screenshot launches passed. Manual GNOME modal/channel interaction
  and canvas-response acceptance remain pending. Evidence:
  `evidence/ton-010-pattern-editor-channel-boundary-2026-08-01.md`.

- ton-010-pattern-editor-point-modes-rgb-2026-08-01 — corrected native point
  definition behavior so intersections, curve spacing, and full curves produce
  distinct bounded deterministic placements; active RGB/CMYK channel labels and
  sampler/seed reopen persistence are now model-aware. Focused binary tests,
  full library tests, strict Clippy/release/format checks, and GTK smoke passed;
  freehand curve editing, continuous full-curve paths, and human modal
  acceptance remain pending. Evidence:
  evidence/ton-010-pattern-editor-point-modes-rgb-2026-08-01.md.

- ton-010-pattern-editor-expanded-controls-2026-08-01 — fixed custom draft
  reopen reset and added persisted X/Y grid controls, per-channel samplers,
  unified/per-channel seeds, deterministic jitter, and editor lattice
  operation v1; full tests, Clippy/format, and GTK smoke passed. Numeric curve
  bends are implemented; freehand curve editing/full continuous paths remain
  pending. Evidence:
  evidence/ton-010-pattern-editor-expanded-controls-2026-08-01.md.

- `ton-010-pattern-editor-modal-lifecycle-fix-2026-08-01` — fixed the
  repeated-entry Adwaita critical by detaching the Blueprint-owned modal draft
  controls on every response; focused/full tests and GTK startup passed.
  Human Save As → reopen acceptance remains pending. Evidence:
  `evidence/ton-010-pattern-editor-modal-lifecycle-fix-2026-08-01.md`.

- `ton-010-pattern-editor-final-runtime-2026-08-01` — final parent runtime
  gate on dirty HEAD `262c7e8`: full tests, strict lint/format/release checks,
  and a real GTK `--demo --show-controls --screenshot` launch exposing the
  `Edit Pattern…` entry point; human modal interaction and accessibility
  acceptance remain pending. Evidence:
  `evidence/ton-010-pattern-editor-final-runtime-2026-08-01.md`.

- `ton-010-pattern-editor-substage-4c-parent-review-2026-08-01` —
  parent-accepted minimum usable Pattern Editor on dirty HEAD `262c7e8`:
  accessible modal draft, cancel-safe construction, one-edit Apply with
  production preview/autosave, atomic `.tnpattern` Save As, and a custom
  summary panel; focused resource/UI/runtime checks and full lib/bin/clippy
  gates passed. Live draft preview, graph editing, broader library parity, and
  manual GNOME acceptance remain pending. Evidence:
  `evidence/ton-010-pattern-editor-substage-4c-parent-review-2026-08-01.md`.

- `ton-010-custom-runtime-substage-4b-parent-review-2026-08-01` —
  parent-accepted project-embedded custom Shapes runtime on dirty HEAD
  `262c7e8`: strict embedded definition/instance authority, one-edit install,
  save/reopen and missing-selection rejection, and live bounded canonical
  dispatch before the legacy adapter. GTK editor/library and custom consumer
  parity remain pending. Evidence:
  `evidence/ton-010-custom-runtime-substage-4b-parent-review-2026-08-01.md`.

- `ton-010-schema-substage-4a-parent-review-2026-08-01` — parent-accepted
  current-only document/preset version boundary on dirty HEAD `262c7e8`:
  documents are v9, `.tntr` presets are v6, bundled presets are updated, and
  obsolete/future headers are rejected before semantic work. Custom
  definitions/editor remain pending. Evidence:
  `evidence/ton-010-schema-substage-4a-parent-review-2026-08-01.md`.

- `ton-010-bundled-recipes-substage-3e3b-parent-review-2026-08-01` —
  parent-accepted live Curves recipe dispatch on dirty HEAD `262c7e8`: the
  document path reads authoritative pattern state, invokes the bundled
  orchestrator, preserves cancellation, and keeps retained whole-generation
  work test-only; focused live, consumer, native, and diff checks passed.
  Manual GNOME/Wayland and reference-artifact acceptance remain pending.
  Evidence:
  `evidence/ton-010-bundled-recipes-substage-3e3b-parent-review-2026-08-01.md`.

- `ton-010-bundled-recipes-substage-3e3a3-parent-review-2026-08-01` —
  parent-accepted non-dispatched Curves consumer parity on dirty HEAD
  `262c7e8`: exact preview/PNG/editable-SVG output equality, transparent and
  opaque background behavior, cancellation/limits, and atomic file-export seam.
  Live routing remains pending. Evidence:
  `evidence/ton-010-bundled-recipes-substage-3e3a3-parent-review-2026-08-01.md`.

- `ton-010-bundled-recipes-substage-3e3a2-parent-review-2026-08-01` —
  parent-accepted exhaustive non-dispatched Curves canonical parity on dirty
  HEAD `262c7e8`: complete 38-parameter manifest, CMYK/RGB/Crosshatch/layout/
  alpha matrix, deterministic full-structure equality, no-op proof, and native
  resource rejection. Consumer/live dispatch remains pending. Evidence:
  `evidence/ton-010-bundled-recipes-substage-3e3a2-parent-review-2026-08-01.md`.

- `ton-010-bundled-recipes-substage-3e3a1-parent-review-2026-08-01` —
  parent-accepted non-dispatched Curves multi-channel orchestration on dirty
  HEAD `262c7e8`: retained-order semantic layers, cache-aware field provider,
  disabled zero work, external Crosshatch color, cancellation, and
  representative retained equality. Consumers/live dispatch are not claimed.
  Evidence:
  `evidence/ton-010-bundled-recipes-substage-3e3a1-parent-review-2026-08-01.md`.

- `ton-010-bundled-recipes-substage-3e2d-parent-review-2026-08-01` —
  parent-accepted Curves emit and complete six-body native operation set on
  dirty HEAD `262c7e8`: canonical semantic single-layer Paths, disabled policy,
  final limits, and full generic execution pass. Multi-channel orchestration,
  consumers, and live dispatch are not claimed. Evidence:
  `evidence/ton-010-bundled-recipes-substage-3e2d-parent-review-2026-08-01.md`.

- `ton-010-bundled-recipes-substage-3e2c-parent-review-2026-08-01` —
  parent-accepted partial Curves-native width modulation on dirty HEAD
  `262c7e8`: shared retained/native interpolation-to-outline authority, strict
  provenance, narrow output, cancellation, and native input/command limits.
  Emit and live dispatch are not claimed. Evidence:
  `evidence/ton-010-bundled-recipes-substage-3e2c-parent-review-2026-08-01.md`.

- `ton-010-bundled-recipes-substage-3e2b-parent-review-2026-08-01` —
  parent-accepted partial Curves-native deformation on dirty HEAD `262c7e8`:
  shared retained/native helper authority, full layout/coverage/transform
  fidelity, narrow output, cancellation, and pre-allocation resource limits.
  Modulation, emission, and live dispatch are not claimed. Evidence:
  `evidence/ton-010-bundled-recipes-substage-3e2b-parent-review-2026-08-01.md`.

- `ton-010-bundled-recipes-substage-3e2a-parent-review-2026-08-01` —
  parent-accepted partial Curves-native boundary on dirty HEAD `262c7e8`:
  narrow placement, semantic source sampling, strict graph-aware motif asset
  preflight/selection, and typed runtime values. Downstream bodies and live
  dispatch are not claimed. Evidence:
  `evidence/ton-010-bundled-recipes-substage-3e2a-parent-review-2026-08-01.md`.

- `ton-010-bundled-recipes-substage-3e1-parent-review-2026-08-01` —
  parent-accepted Curves declarative contract on dirty HEAD `262c7e8`: strict
  bundled definition, Curves-only typed graph, authoritative complete adapter,
  digest-backed editable paths, model-faithful bounds, truthful dual ownership
  of output quality, and exclusion of two retained no-op fields. Live Curves
  remains compatibility; native execution and dispatch are not claimed.
  Evidence:
  `evidence/ton-010-bundled-recipes-substage-3e1-parent-review-2026-08-01.md`.

- `ton-010-bundled-recipes-substage-3d3b-parent-review-2026-08-01` —
  parent-reviewed live Shapes recipe dispatch on dirty HEAD `262c7e8`; RGB/CMYK
  documents execute recipe=1/oracle=0, retained whole-generation code is
  test-only, and live preview/PNG/editable-SVG consumers stay canonical. Human
  Stage 5 acceptance remains pending. Evidence:
  `evidence/ton-010-bundled-recipes-substage-3d3b-parent-review-2026-08-01.md`.

- `ton-010-bundled-recipes-substage-3d3a-complete-parent-review-2026-08-01` —
  parent-accepted complete non-dispatched Shapes recipe orchestration on dirty
  HEAD `262c7e8`: exhaustive canonical oracle equality, provider/cache and
  cancellation behavior, RGB/CMYK preview-PNG parity, and byte-identical
  editable mark SVG/file export all pass. Evidence:
  `evidence/ton-010-bundled-recipes-substage-3d3a-complete-parent-review-2026-08-01.md`.

- `ton-010-bundled-recipes-substage-3d3a-canonical-parent-review-2026-08-01` —
  parent-reviewed exhaustive full-structure Shapes recipe/oracle equivalence,
  custom cubic projection, assignment behavior, cache/disabled work, and
  deterministic cancellation on dirty HEAD `262c7e8`. Consumer parity remains
  before 3D3A acceptance. Evidence:
  `evidence/ton-010-bundled-recipes-substage-3d3a-canonical-parent-review-2026-08-01.md`.

- `ton-010-bundled-recipes-substage-3d3a-foundation-parent-review-2026-08-01` —
  parent-reviewed partial, non-dispatched Shapes recipe orchestration on dirty
  HEAD `262c7e8`; recipe-driven field requests, cache behavior, disabled native
  work, stable layers, and initial CMYK canonical equality pass. This is not
  3D3A acceptance; the exhaustive matrix and consumers remain. Evidence:
  `evidence/ton-010-bundled-recipes-substage-3d3a-foundation-parent-review-2026-08-01.md`.

- `ton-010-bundled-recipes-substage-3d2-parent-review-2026-08-01` —
  parent-reviewed six bounded native Shapes operations, recipe-driven semantic
  field requests, exact mapping order, typed family-isolated ports, and generic
  registry preflight on dirty HEAD `262c7e8`. Live Shapes dispatch remains the
  compatibility renderer pending 3D3 equivalence. Evidence:
  `evidence/ton-010-bundled-recipes-substage-3d2-parent-review-2026-08-01.md`.

- `ton-010-bundled-recipes-substage-3d1-parent-review-2026-08-01` —
  parent-reviewed immutable bundled Shapes definition, corrected six-stage
  typed descriptor graph, complete guided metadata, and strict one-way custom
  motif adaptation on dirty HEAD `262c7e8`. Crosshatch remains outside the
  recipe and live Shapes dispatch remains the compatibility renderer. Evidence:
  `evidence/ton-010-bundled-recipes-substage-3d1-parent-review-2026-08-01.md`.

- `ton-010-bundled-recipes-substage-3c-parent-review-2026-08-01` —
  parent-reviewed live Weighted renderer dispatch through the bundled recipe
  executor on dirty HEAD `262c7e8`; the former generator is test-only oracle
  code, release builds cannot call it, and focused RGB/CMYK canonical consumer
  checks pass. Stage 5 human acceptance remains pending. Evidence:
  `evidence/ton-010-bundled-recipes-substage-3c-parent-review-2026-08-01.md`.

- `ton-010-bundled-recipes-substage-3b-parent-review-2026-08-01` —
  parent-reviewed six atomic production Weighted recipe operations and exact
  RGB/CMYK canonical equivalence on dirty HEAD `262c7e8`; disabled channels do
  zero field/native work and neutral distribution/Voronoi authorities remain
  unchanged. Live renderer dispatch is still old authority. Evidence:
  `evidence/ton-010-bundled-recipes-substage-3b-parent-review-2026-08-01.md`.

- `ton-010-bundled-recipes-substage-3a-parent-review-2026-08-01` —
  parent-reviewed immutable bundled Weighted `.tnpattern` and production typed
  descriptor graph on dirty HEAD `262c7e8`; common strict loader/registry,
  deterministic fingerprint, RGB/CMYK defaults, and bundled immutability pass,
  with no render/persistence integration yet. Evidence:
  `evidence/ton-010-bundled-recipes-substage-3a-parent-review-2026-08-01.md`.

- `ton-010-recipe-contract-reconciliation-2026-08-01` — documentation
  reconciliation for the accepted declarative `.tnpattern` v1 contract and
  automated-validated Stage 5 status on dirty HEAD `262c7e8`; records changed
  durable docs, stale follow-up claims removed, exact evidence counts, and
  remaining human/integration gaps. Valid until recipe integration, Stage 5
  manual acceptance, schema-version changes, HEAD, or dirty-state assumptions
  change; report is in
  `agents/documentation-maintainer/ton-010-recipe-contract-reconciliation.md`.

- `ton-010-recipe-contract-substage-2c2-parent-review-2026-08-01` —
  parent-reviewed deterministic cancellable recipe executor on dirty HEAD
  `262c7e8`; data-only DAGs call only static registered Rust operations, reuse
  neutral distribution/Voronoi and canonical geometry types, bind strict scoped
  instances, and enforce runtime ports plus declared canonical capabilities.
  Valid until executor/registry/runtime/canonical types, HEAD, or dirty state
  changes; evidence is in
  `evidence/ton-010-recipe-contract-substage-2c2-parent-review-2026-08-01.md`.

- `ton-010-recipe-contract-substage-2c1-parent-review-2026-08-01` —
  parent-reviewed strict creator parameter schema and scoped instance values on
  dirty HEAD `262c7e8`; covers exact `u64` seeds, numeric/choice/text/asset
  constraints, duplicate-aware list payloads, semantic channel keys, explicit
  new-instance defaults, strict parsing, and deterministic serialization.
  Valid until parameter/instance contracts, channel parsing, asset handling,
  HEAD, or dirty state changes; evidence is in
  `evidence/ton-010-recipe-contract-substage-2c1-parent-review-2026-08-01.md`.

- `ton-010-recipe-contract-substage-2b-parent-review-2026-08-01` —
  parent-reviewed deterministic bundled/user/project definition resolution on
  dirty HEAD `262c7e8`; bundled definitions are immutable, same-content entries
  deduplicate, and project-embedded custom content overrides differing local
  user content with an inspectable typed diagnostic rather than substitution.
  Valid until definition serialization/fingerprints, precedence/diagnostics,
  HEAD, or recorded dirty state changes; evidence is in
  `evidence/ton-010-recipe-contract-substage-2b-parent-review-2026-08-01.md`.

- `ton-010-recipe-contract-substage-2a-parent-review-2026-08-01` —
  parent-reviewed open stable `PatternId` and strict `.tnpattern` v1 contract
  on dirty HEAD `262c7e8`; covers deterministic JSON, typed DAG validation,
  bounded native operation descriptors, scoped parameters/quick controls,
  authoring layout, SVG safety, and exact-byte SHA-256 asset identity. Runtime
  execution, bundles, library resolution, schema bumps, and UI remain later
  substages. Valid until the contract, adapted ID call sites, dependencies,
  HEAD, or recorded dirty state changes; evidence is in
  `evidence/ton-010-recipe-contract-substage-2a-parent-review-2026-08-01.md`.

- `ton-010-stage5-framework-restart-substage-a-parent-review-2026-08-01` —
  parent-reviewed direct-positive Weighted Voronoi producer correction on
  dirty HEAD `54a8e37`; final boundary-derived inset polygons are positive
  canonical regions and cell-sizing subtraction is absent. Valid until the
  producer, canonical algebra, response-inset geometry, consumers, HEAD, or
  recorded dirty state changes.

- `ton-010-stage5-framework-restart-substage-b-parent-review-2026-08-01` —
  parent-reviewed model-aware semantic raster compositor on dirty HEAD
  `54a8e37`; isolated channel coverage, RGB/CMYK model composition, local
  subtraction, gap behavior, and preview/PNG parity are covered. Valid until
  render, canonical region, PNG, Weighted producer, HEAD, or dirty state
  changes.

- `ton-010-stage5-framework-restart-substage-c-parent-review-2026-08-01` —
  parent-reviewed compound semantic SVG serializer on dirty HEAD `54a8e37`;
  Weighted output has one final positive compound path per channel, no
  cell-sizing masks, and genuine subtraction retains local masks. Valid until
  SVG, canonical algebra, raster composition, Weighted producer,
  export-background behavior, HEAD, or dirty state changes.

- `ton-010-stage5-framework-substage-d-2026-07-29` — parent correction and
  focused acceptance evidence for the restarted Stage 5 framework on branch
  `TON-010-Stage5-Framework-Restart`; covers the realized Weighted Voronoi UI
  fixture reset, explicit non-adjacent region relationships, fmt/check,
  focused Weighted Voronoi tests, and the realized GTK selector/control test.
  Valid until the Stage 5 implementation or current dirty state changes;
  evidence is in `evidence/ton-010-stage5-framework-substage-d-2026-07-29.md`.

- `ton-010-stage5-framework-final-validation-2026-07-29` — comprehensive
  restart validation: 161 library tests, 48 binary/UI tests, strict Clippy,
  locked release build, formatting, diff checks, and focused realized GTK
  coverage. Blueprint lint parses but remains nonzero on repository-wide
  existing warning policy; production Blueprint compilation and GTK resource
  realization pass. Evidence is in
  `evidence/ton-010-stage5-framework-final-validation-2026-07-29.md`.

- `ton-010-stage-4.5c2-authoring-correction-accepted-f9c138c-dirty` — the
  current export-background authoring/dialog correction was accepted by the
  user on 2026-07-28; `Document.appearance.export_background` remains the sole
  saved authority, with Preview Surface separate. C2B-1 has now completed its
  parent review. Valid until appearance controls, persistence, export, or
  current-stage sequencing changes; evidence remains in
  `evidence/ton-010-stage-4.5c2-export-background-authoring-parent-review-f9c138c-dirty.md`.

- `ton-010-stage-4.5c2b1-adapter-authority-f9c138c-dirty` —
  `evidence/ton-010-stage-4.5c2b1-adapter-authority-f9c138c-dirty.md`; parent-
  reviewed C2B-1 proof that contradictory Shapes/Curves adapters cannot
  override authoritative rendering, save/reopen, undo/redo, or shipped
  selector transitions, plus the Crosshatch source-authority correction. Full
  validation is 143 library and 48 binary/UI tests on dirty HEAD `f9c138c`,
  dated 2026-07-28. Valid until pattern projection, Crosshatch transitions,
  persistence, legacy dispatch, C1 fixtures, or selector synchronization
  changes; paused for approval before C2B-2.

- `ton-010-stage-4.5c2b2a-output-cache-authority-f9c138c-dirty` —
  `evidence/ton-010-stage-4.5c2b2a-output-cache-authority-f9c138c-dirty.md`;
  parent-reviewed CMYK/RGB active/inactive cache authority proof for both C1
  fixtures, covering rendering, transitions, cache restore, save/reopen,
  undo/redo, Preview Surface, and Export Background separation. Full validation
  is 144 library and 48 binary/UI tests on dirty HEAD `f9c138c`, dated
  2026-07-28. Valid until output transition/cache lifecycle, persistence,
  history, fixtures, or presentation ownership changes; paused before C2B2-B.

- `ton-010-stage-4.5c2b2b-realized-output-authority-f9c138c-dirty` —
  `evidence/ton-010-stage-4.5c2b2b-realized-output-authority-f9c138c-dirty.md`;
  parent-reviewed realized Blueprint/GResource `AppUi` authority coverage for
  CMYK/RGB switching, typed Shapes/Curves controls, contradictory active and
  inactive adapters, cache restoration, undo/redo, Preview Surface, and Export
  Background separation. Full validation is 144 library and 48 binary/UI
  tests on dirty HEAD `f9c138c`, dated 2026-07-28. Valid until resources,
  output callbacks, synchronization, cache lifecycle, fixtures, or presentation
  ownership changes; paused before C2C.

- `ton-010-stage-4.5c3a-preview-png-parity-f9c138c-dirty` —
  `evidence/ton-010-stage-4.5c3a-preview-png-parity-f9c138c-dirty.md`;
  parent-reviewed preview/PNG parity for current-format `Polygon Six` and
  `Motif Ladder`, including contradictory adapter resistance, transparency,
  Preview Surface versus Export Background separation, deterministic bytes,
  and four inspectable artifacts under `test-artifacts/ton-010-stage-4.5c3a/`.
  Full validation is 145 library and 48 binary/UI tests on dirty HEAD
  `f9c138c`, dated 2026-07-28. Valid until fixture parsing, preview/PNG
  composition, presentation ownership, adapter projection, or artifact routing
  changes; paused before C3-B SVG parity.

- `ton-010-stage-4.5c3b-svg-parity-parent-review-f9c138c-dirty` —
  `evidence/ton-010-stage-4.5c3b-svg-parity-parent-review-f9c138c-dirty.md`;
  parent-reviewed SVG parity for the current-format C1 fixtures, with editable
  groups/path IDs, cubic geometry, clipping, deterministic projection,
  transparency, Export Background separation, and contradictory-adapter
  resistance. The writer hit a usage-limit blocker after leaving valid partial
  work; the parent preserved and reviewed it. Full validation is 146 library
  and 48 binary/UI tests on dirty HEAD `f9c138c`, dated 2026-07-28. Valid until
  SVG exporter, canonical projection, fixture, presentation, or artifact
  routing changes; 4.5D is active.

- `ton-010-stage-4.5d-integrated-readiness-parent-review-f9c138c-dirty` —
  `evidence/ton-010-stage-4.5d-integrated-readiness-parent-review-f9c138c-dirty.md`;
  parent-owned final reconciliation of 4.5A through C3-B, including authority,
  persistence, adapters, CMYK/RGB, preview/PNG/SVG artifacts, current-schema
  rejection, and the preserved C3-B delegation blocker. Final validation is
  146 library and 48 binary/UI tests on dirty HEAD `f9c138c`, dated 2026-07-28.
  Valid until any 4.5 evidence, current schemas, adapter boundaries, fixtures,
  or Stage 5 sequencing changes; Stage 5 remains untouched and gated.

- `ton-010-stage-4.5c2-png-export-background-parent-review-f9c138c-dirty` —
  `evidence/ton-010-stage-4.5c2-png-export-background-parent-review-f9c138c-dirty.md`;
  parent-reviewed diagnosis and accessible PNG-dialog correction proving saved
  `ExportBackground::None` intentionally yields transparency, with 140+47
  validation on dirty HEAD `f9c138c` dated 2026-07-28. Valid until PNG
  composition, appearance state, dialog summary, accessibility, or the dirty
  baseline changes; paused before C2B.

- `ton-010-stage-4.5c2a-parent-review-f9c138c-dirty` —
  `evidence/ton-010-stage-4.5c2a-parent-review-f9c138c-dirty.md`; parent-reviewed
  current-document save/reopen and authoritative undo/redo coverage for both
  C1 production fixtures, with 140+46 validation on dirty HEAD `f9c138c` dated
  2026-07-28. Valid until persistence, preset application, editor history, or
  fixture contracts change; paused before C2B.

- `ton-010-stage-4.5c1-parent-review-f9c138c-dirty` —
  `evidence/ton-010-stage-4.5c1-parent-review-f9c138c-dirty.md`; parent-reviewed
  C1 current-format fixture foundation with production bundled `Polygon Six`
  Shapes and `Motif Ladder` Curves presets, authoritative schema assertions,
  and 139+46 validation on dirty HEAD `f9c138c` dated 2026-07-28. Valid until
  preset schema, registry versions, bundled inventory, or the dirty baseline
  changes; paused before C2.

- `ton-010-stage-4.5b-polygon-sides-f9c138c-dirty` —
  `evidence/ton-010-stage-4.5b-polygon-sides-f9c138c-dirty.md`; parent-reviewed
  correction of the shipping Blueprint/GResource Regular Polygon side-count
  row visibility and realized GTK authority coverage on dirty HEAD `f9c138c`
  dated 2026-07-28. Valid until `src/ui.rs`, current Blueprint/GResource
  resources, Shapes authority, or the dirty baseline changes; 4.5B was
  accepted by the user before 4.5C began.

- `ton-010-stage-4.5b-shape-editor-f9c138c-dirty` —
  `evidence/ton-010-stage-4.5b-shape-editor-f9c138c-dirty.md`; parent-reviewed
  shipping Blueprint/GResource shape-editor correction, realized GTK authority
  workflow, and four inspected visual artifacts on dirty HEAD `f9c138c` dated
  2026-07-28. Valid until `src/ui.rs`, Blueprint/GResource resources, GTK
  behavior, artifacts, or the dirty baseline changes; 4.5B was accepted by the
  user before 4.5C began.

- `ton-010-stage-4.5a-historical-audit-f9c138c-dirty` —
  `evidence/ton-010-stage-4.5a-historical-audit-f9c138c-dirty.md`; parent-
  reviewed read-only pre-TON-013 comparison, shape-editor inventory, and
  4.5C/4.5D demonstrability matrix on dirty HEAD `f9c138c` dated 2026-07-28.
  Valid until relevant UI/resource/model files or the dirty baseline changes;
  4.5A is complete for review and paused before 4.5B.

- `ton-010-stage-4.5-sequence-insertion-f9c138c-dirty` —
  `evidence/ton-010-stage-4.5-sequence-insertion-f9c138c-dirty.md`; parent
  record of the inserted four-gate baseline-restoration/demonstrability gate,
  explicit Stage 4 preservation, and Stage 5 block on dirty HEAD `f9c138c`
  dated 2026-07-28. Valid until the TON-010 sequence or 4.5 gate contract
  changes; no 4.5 substage has started.

- `ton-010-stage-4d1-validation-f9c138c-dirty` —
  `evidence/ton-010-stage-4d1-validation-f9c138c-dirty.md`; parent-reviewed
  final Stage 4 authority/UI validation, adapter inventory, durable docs, and
  138+46 full-suite evidence on dirty HEAD `f9c138c` dated 2026-07-28. Valid
  until Stage 4 authority/UI/docs or the dirty baseline changes; Stage 5
  Weighted Voronoi is the next paused gate.

- `ton-010-stage-4-substage-4c2a-curves-authority-f9c138c-dirty` —
  `evidence/ton-010-stage-4-substage-4c2a-curves-authority-f9c138c-dirty.md`;
  parent-reviewed Curves scalar/layout/color/visibility authority migration
  and realized GTK contradiction coverage on dirty HEAD `f9c138c` dated
  2026-07-28. Valid until Curves authority/UI files or the dirty baseline
  change; next handoff is Stage 4C2b direct editor/motif/context migration.

- `ton-010-stage-4-substage-4c2b-curves-direct-authority-f9c138c-dirty` —
  `evidence/ton-010-stage-4-substage-4c2b-curves-direct-authority-f9c138c-dirty.md`;
  parent-reviewed removal of all production Curves adapter reads from
  `src/ui.rs`, authority-only editor/motif/context paths, and realized GTK
  contradiction coverage on dirty HEAD `f9c138c` dated 2026-07-28. Valid until
  Curves/UI authority files or the dirty baseline changes; safe handoff is
  Stage 4D validation/documentation.

- `ton-010-stage-4-substage-4c1-shapes-authority-f9c138c-dirty` —
  `evidence/ton-010-stage-4-substage-4c1-shapes-authority-f9c138c-dirty.md`;
  parent-reviewed Shapes parameter authority migration and realized GTK
  contradiction coverage, with Curves intentionally deferred and full 138+46
  validation on dirty HEAD `f9c138c` dated 2026-07-28. Valid until Shapes
  authority/UI files or the dirty baseline change; next handoff is Stage 4C2.

- `ton-010-stage-4-substage-4b-selector-authority-f9c138c-dirty` —
  `evidence/ton-010-stage-4-substage-4b-selector-authority-f9c138c-dirty.md`;
  parent-reviewed authoritative Shapes/Curves selector synchronization and GTK
  regression coverage, with parameter migration intentionally deferred and
  full 138+46 validation on dirty HEAD `f9c138c` dated 2026-07-28. Valid until
  selector/UI authority files or the dirty baseline change; next handoff is
  Stage 4C parameter binding.

- `ton-010-stage-4-substage-4a-authority-schema-f9c138c-dirty` —
  `evidence/ton-010-stage-4-substage-4a-authority-schema-f9c138c-dirty.md`;
  parent-reviewed Stage 4A authority-only model/schema accessors and stable
  control descriptor lookup, with no UI edits and 138 passing library tests on
  dirty HEAD `f9c138c` dated 2026-07-28. Valid until the authority/schema
  surface or relevant dirty files change; next handoff is Stage 4B selector
  synchronization.

- `ton-010-stage-3-canonical-output-f9c138c-dirty` —
  `evidence/ton-010-stage-3-canonical-output-f9c138c-dirty.md`; canonical
  Marks/Paths preservation, typed regions/networks/composition, shared
  preview/PNG/SVG consumers, synthetic parity fixtures, and full validation on
  dirty HEAD `f9c138c` dated 2026-07-28. Invalidate after Stage 4 or any
  relevant pattern, renderer/export, tracker, HEAD, or dirty-tree changes.

- `ton-010-stage-2-authority-implementation-f9c138c-dirty` —
  `agents/desktop-implementer/ton-010-stage-2-authority-implementation.md`;
  Stage 2 authority-first cutover, current document v8/preset v5, adapter
  inventory, current-format rejection, and 130 passing library tests on the
  dirty checkout dated 2026-07-28. This supersedes the paused Stage 2
  implementation notes for authority and validation; invalidate after Stage 3
  or any relevant pattern, persistence, renderer, preset, tracker, HEAD, or
  dirty-tree changes.

- `ton-010-stage-2-authority-pause-and-sequence-revision-f9c138c-dirty` —
  `evidence/ton-010-stage-2-authority-pause-and-sequence-revision-f9c138c-dirty.md`;
  records the paused unverified Stage 2 partial work, removal of dual-authority
  UI scaffolding, the authoritative-pattern cutover rule, expanded canonical
  output requirements, required Weighted Voronoi, and custom-pattern follow-up
  boundary on dirty HEAD `f9c138c` dated 2026-07-28. Invalidate after relevant
  implementation, persistence/preset, UI, renderer, tracker, HEAD, or dirty-tree
  changes.

- `ton-010-stage-2-current-pattern-schema-selector-ui-handoff-f9c138c-dirty` —
  `evidence/ton-010-stage-2-current-pattern-schema-selector-ui-handoff-f9c138c-dirty.md`;
  current Stage 2 implementation boundary for schema descriptors, registry-derived
  stable selection, existing Shapes/Curves RenderVariant paths, and GTK review
  checks against dirty HEAD `f9c138c` on 2026-07-28. Invalidate after relevant
  pattern/UI/source/resource, HEAD, or dirty-worktree changes.

- `ton-010-stage-1-regression-review-f9c138c-corrections` —
  `evidence/ton-010-stage-1-regression-review-f9c138c-corrections.md`;
  corrected Stage 1 review accepted the pre-policy compatibility-cache
  preservation and typed adapters; superseded by the no-backwards-compatibility
  policy and document-definition update to v7.

- `ton-010-stage-1-reviewed-findings-implementation` —
  `agents/desktop-implementer/ton-010-stage-1-reviewed-findings-implementation.md`;
  superseded pre-policy evidence for the output-treatment compatibility
  envelope and typed Shapes/Curves adapters. Current policy rejects obsolete
  definitions and uses document version 7.

- `ton-013-control-exposure-documentation-reconciliation` —
  `agents/documentation-maintainer/ton-013-control-exposure-documentation-reconciliation.md`;
  reconciles current TON-013 control-exposure ownership, routing/scope and
  aggregate/channel semantics, verification counts, the 1000x980 artifact, and
  Cambalache round-trip limits against the dirty checkout on 2026-07-26.
  Invalidate after relevant source/UI/CMB, docs, evidence, artifact, Git HEAD,
  or dirty-worktree changes.

- `ton-013-control-exposure-independent-ux-546ea4c-dirty` —
  `evidence/ton-013-control-exposure-independent-ux-546ea4c-dirty.md`; historical
  pre-correction review that identified the five dropdown relations and
  native-row boundary correction. Superseded for current ownership by the
  control-exposure correction record. Invalidate after relevant UI, source,
  docs, artifact, GTK, HEAD, or dirty-worktree changes.

- `ton-013-control-exposure-stage-implementation` —
  `agents/desktop-implementer/ton-013-control-exposure-stage-implementation.md`;
  pre-correction bounded Builder ownership for Source/Output/Appearance,
  treatment chrome, and stable native/Shapes/Curves panel hosts, verified
  against dirty HEAD `546ea4c` on 2026-07-26. The correction record supersedes
  its accessibility/native-row details. Invalidate after `src/ui.rs`, UI/CMB,
  focused GTK tests, screenshot artifact, Git HEAD, or relevant dirty-file
  changes.

- `ton-013-control-exposure-stage-correction` —
  `agents/desktop-implementer/ton-013-control-exposure-stage-correction.md`;
  corrected five Source/Output accessibility relations and moved the four
  Basic/native scale row shells into Builder, preserving Rust adjustments and
  callbacks. Includes focused realized GTK coverage and the 1000x980 artifact.
  Invalidate after relevant source/UI/CMB/docs/tests/artifact/GTK/HEAD changes.

- `ton-013-control-exposure-inventory-546ea4c-dirty` —
  `evidence/ton-013-control-exposure-inventory-546ea4c-dirty.md`; read-only
  inventory confirming that most concrete Source, Output, Appearance, and
  Treatment controls remain Rust-created, with the practical Builder boundary,
  stable ID groups, and Rust-only dynamic/custom exceptions. Invalidate after
  changes to `src/ui.rs`, `resources/ui/*`, `Toniator.cmb`, GTK/libadwaita,
  focused tests, relevant docs, Git HEAD, or dirty-file assumptions.

- `ton-013-stage-2-documentation-reconciliation` —
  `agents/documentation-maintainer/ton-013-stage-2-documentation-reconciliation.md`;
  reconciled durable UI architecture and TON-013 Stage 2 tracker wording
  against the corrected implementation, CMB/UI resources, final UX review,
  and the 1000x760 screenshot artifact on 2026-07-26. Invalidate after
  relevant Stage 2 implementation, resource, documentation, evidence, or
  working-tree changes.

- `ton-013-stage-2-treatment-scope-correction` —
  `agents/desktop-implementer/ton-013-stage-2-treatment-scope-correction.md`;
  final semantic correction making Treatment Editing Scope the sole visible
  treatment recipient selector while Output routing remains authoritative.
  Includes focused tests and the inspected 1000x760 GTK artifact. Invalidate
  after target callbacks, pipeline controls, UI/CMB changes, or GTK changes.

- `ton-013-stage-2-treatment-scope-final-ux-546ea4c-dirty` —
  `evidence/ton-013-stage-2-treatment-scope-final-ux-546ea4c-dirty.md`; final
  review passes the corrected treatment scope, hierarchy, semantic templates,
  and 1000x760 GTK screenshot. Records focus/accessibility as follow-up.
  Invalidate after relevant Stage 2 changes.

- `ton-013-stage-2-independent-channel-inspector-ux-546ea4c-dirty` —
  `evidence/ton-013-stage-2-independent-channel-inspector-ux-546ea4c-dirty.md`;
  correction-required review: actual hierarchy placement, competing scalar vs
  treatment scopes, structural-only panel hosts, and expanded-state concerns.
  Invalidate after correction changes.

- `ton-013-stage-2-channel-control-architecture-546ea4c-dirty` —
  `evidence/ton-013-stage-2-channel-control-architecture-546ea4c-dirty.md`;
  current code ownership, semantic channel identity, aggregate-panel boundary,
  and safe Stage 2 implementation split.

- `ton-013-stage-2-channel-inspector-ux-546ea4c-dirty` —
  `evidence/ton-013-stage-2-channel-inspector-ux-546ea4c-dirty.md`; settled
  Source/Output/Channel Settings hierarchy, terminology, defaults, and
  aggregate-versus-real-channel acceptance criteria.

- `ton-013-stage-1-independent-shell-ux-546ea4c-dirty` —
  `evidence/ton-013-stage-1-independent-shell-ux-546ea4c-dirty.md`; reviewed
  the current GtkBuilder shell, docs, and real GTK screenshot on 2026-07-26;
  pass after correcting runtime ownership wording for the Controls tooltip.
  Invalidate after relevant shell, docs, version, or worktree changes.

- `ton-013-stage-1-documentation` —
  `agents/documentation-maintainer/ton-013-stage-1-documentation-reconciliation.md`;
  reconciled the Stage 1 architecture and TON-013 issue wording against the
  current shell resource, Rust loader/page insertion, review evidence, and
  GTK artifact on 2026-07-26. Invalidate after relevant shell, docs, evidence,
  version, or worktree changes.

- `ton-013-gtkbuilder-migration-seams-546ea4c` —
  `evidence/ton-013-gtkbuilder-migration-seams-546ea4c.md`; valid for HEAD
  `546ea4c` with only untracked `.codex-work/backups/`; safe first GtkBuilder
  boundary and current GTK/model stability seams. Invalidate after relevant
  source, build, packaging, or working-tree changes.

- `ton-012-stage-1b-complete` — `evidence/ton-012-stage-1b-complete.md`;
  validated against HEAD `32022df` on 2026-07-26; authoritative pipeline state,
  schema v6/v3 persistence, migration, and active/saved/inactive snapshots.
- `ton-012-stage-2-render-resolution-paths` —
  `evidence/ton-012-stage-2-render-resolution-paths.md`; validated against HEAD
  `32022df` on 2026-07-26; live decode, sampling, separation, alpha, consumer,
  and SVG export boundaries for Stage 2.
- `ton-012-stage-2-review` — `evidence/ton-012-stage-2-review.md`; validated
  against HEAD `32022df` on 2026-07-26; independent review findings and parent
  corrections for SVG semantic output and field-cache isolation.
- `ton-012-stage-3-implementation` —
  `agents/desktop-implementer/ton-012-stage-3-implementation.md` and
  `agents/desktop-implementer/ton-012-stage-3-correction-implementation.md`;
  validated against working tree based on HEAD `bac55f7` on 2026-07-26;
  shared semantic Document controls, direct callbacks, output-cache transition,
  Crosshatch restoration, and verification evidence.
- `ton-012-stage-3-review-current-head` —
  `evidence/ton-012-stage-3-review-current-head.md`; validated against the
  corrected working tree on 2026-07-26; GTK/state review findings and required
  corrections, now resolved by the implementation pass.
- `ton-012-stage-3-creative-usability-review` —
  `evidence/ton-012-stage-3-creative-usability-review.md`; validated against
  the corrected working tree on 2026-07-26; source/alpha/assignment guidance
  findings, now resolved by the implementation pass.
- `ton-012-stage-3-documentation` —
  `agents/documentation-maintainer/ton-012-stage-3-documentation-reconciliation.md`;
  validated against the corrected working tree on 2026-07-26; durable docs and
  issue-ledger reconciliation.
- `ton-012-stage-4-preset-ownership-current-head` —
  `evidence/ton-012-stage-4-preset-ownership-current-head.md`; validated
  against HEAD `236cdb1` on 2026-07-26; targeted preset ownership, call paths,
  bundled inventory, retained compatibility adapters, and Stage 4 format
  assumption.
- `ton-012-stage-4-preset-review-236cdb1` —
  `evidence/ton-012-stage-4-preset-review-236cdb1.md`; reviewed against the
  uncommitted Stage 4 diff on 2026-07-26; independent atomicity, scope,
  channel-identity, and nested-validation findings.
- `ton-012-stage-4-implementation` —
  `agents/desktop-implementer/ton-012-stage-4-implementation.md`; validated
  against the corrected uncommitted Stage 4 worktree on 2026-07-26; v4 scoped
  format, atomic application, bundled conversion, UI scope chooser, tests,
  and retained adapters.
- `ton-012-stage-4-creative-preset-review-236cdb1` —
  `evidence/ton-012-stage-4-creative-preset-review-236cdb1.md`; reviewed
  against the corrected uncommitted Stage 4 worktree on 2026-07-26; bundled
  naming/output, scope wording, Crosshatch framing, and visual findings.
- `ton-012-stage-5-rendering-parity-audit-current-head-4161635` —
  `evidence/ton-012-stage-5-rendering-parity-audit-current-head-4161635.md`;
  validated against HEAD `4161635` on 2026-07-26; targeted semantic preview,
  PNG/SVG, transition, appearance, and retained-adapter findings. Invalidate
  after relevant Stage 5 implementation edits.
- `ton-012-stage-5-independent-export-parity-review-4161635` —
  `evidence/ton-012-stage-5-independent-export-parity-review-4161635.md`;
  reviewed the Stage 5 implementation on 2026-07-26; found one major RGB
  Crosshatch SVG blend mismatch and minor test gaps. Invalidate after the
  correction changes rendering or export paths.
- `ton-012-stage-5-rendering-parity-implementation` —
  `agents/desktop-implementer/ton-012-stage-5-rendering-parity-implementation.md`;
  validated against the corrected current worktree on 2026-07-26; per-model
  Preview Surface cache, preview/export separation, semantic Curves SVG, and
  focused test evidence.
- `ton-012-stage-5-rgb-crosshatch-svg-correction` —
  `agents/desktop-implementer/ton-012-stage-5-rgb-crosshatch-svg-correction.md`;
  validated against the corrected current worktree on 2026-07-26; RGB-output
  Crosshatch SVG Multiply parity correction and regression evidence.
- `ton-012-stage-5-artifact-creative-output-review-4161635` —
  `evidence/ton-012-stage-5-artifact-creative-output-review-4161635.md`;
  reviewed the ignored Stage 5 visual/serialized artifacts on 2026-07-26;
  no blocker or major finding, with minor dark-viewer Crosshatch friction and
  opaque-source alpha fixture limitations recorded.
- `ton-012-closeout-4161635` — `evidence/ton-012-closeout-4161635.md`;
  final accepted TON-012 architecture, verification, retained adapters, and
  deferred-issue record prepared for the closeout commit.
| `ton-010-stage-3-canonical-output-f9c138c-dirty` | TON-010 Stage 3 canonical output algebra, shared geometry consumers, and parity evidence | `f9c138c` + dirty worktree | 2026-07-28 |
