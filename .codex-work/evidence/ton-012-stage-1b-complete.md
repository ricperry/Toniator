# TON-012 Stage 1B completion evidence

- Repository absolute path: `/home/ricperry1/projects/Toniator`
- Git HEAD: `32022df28e6e746b44fb4f5db4427fd197ee2739`
- Relevant working-tree state: application source, tests, README, CHANGELOG,
  and `src/` were clean at verification; this task intentionally modified
  `ISSUES.md`, `docs/ARTWORK_PIPELINE.md`, and
  `docs/ARTWORK_PIPELINE_AUDIT.md`. Unrelated existing state includes modified
  `.gitignore` and untracked Codex guidance/configuration files plus
  `nextPrompt.txt`.
- Producing agent: parent Codex thread with workspace-write access
- Task or question: determine whether TON-012 Stage 1B is complete and record
  its durable tracking location
- Subsystems inspected: artwork-pipeline domain model, document state,
  persistence/migration, presets, renderer projection, SVG export, issue ledger,
  architecture contract, and audit record
- Exact files and symbols inspected:
  - `src/artwork_pipeline.rs`: `ArtworkPipelineSettings`, stable IDs,
    validation, legacy conversion/projection, output transitions
  - `src/model.rs`: `Document.artwork_pipeline`, saved treatment pipeline
    snapshots, `OutputTreatmentCache`, `sync_legacy_projection`, undo/redo
  - `src/persistence.rs`: current v6 round-trip, strict validation, rejection
    tests, atomic persistence
  - `src/preset.rs`: current v3 round-trip, stable-ID validation, projection
    and bundled preset checks
  - `src/render.rs`, `src/svg_export.rs`: compatibility projection and retained
    renderer/export behavior
  - `ISSUES.md`, `docs/ARTWORK_PIPELINE.md`,
    `docs/ARTWORK_PIPELINE_AUDIT.md`: milestone tracking and stale status text
- Verified findings:
  - Commit `32022df` is titled `TON-012: complete Stage 1 artwork pipeline`.
  - The semantic pipeline is authoritative on the document and paired across
    active, saved Shapes/Curves, and inactive CMYK/RGB treatment state.
  - Project schema v6 and preset schema v3 require validated stable pipeline
    IDs; invalid/missing/mismatched state is rejected.
  - Current verification passes `cargo fmt --check` and `cargo test --locked`:
    93 library tests and 44 binary tests passed.
- Reasonable inferences: the repository’s “Stage 1” implementation commit
  corresponds to the user’s Stage 1B completion claim; TON-012 itself remains
  open because resolved channel fields, broader UI/preset cleanup, and final
  preview/export parity are later stages.
- Unresolved uncertainty: no separate external issue tracker or release note
  was found in this checkout; `ISSUES.md` is the durable issue ledger.
- Commands run: `pwd`; `git status --short --branch`; targeted `rg`/`sed` reads;
  `git log`; `git show 32022df`; `cargo fmt --check`; `cargo test --locked`.
- Artifacts produced: this cache entry and synchronized issue/architecture
  documentation; no application artifacts.
- Conditions that invalidate this entry: relevant source or documentation
  files change, HEAD changes without revalidation, the listed dirty-file
  assumptions change, or later work alters Stage 1B persistence/authority
  semantics.
- Timestamp: `2026-07-26`
