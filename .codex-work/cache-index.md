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
