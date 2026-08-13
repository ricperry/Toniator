# Stage 20C Planning Contract: Document-owned Authored Structures

## Status

Accepted on 2026-08-13 as the decision-complete contract for Stage 20C.
The user accepted the verified implementation and separately authorized its
local checkpoint on 2026-08-13 after the explicit start-gate override, bounded
implementation, focused verification, parent audit, and independent read-only
`PASS` recorded below. Stage 20C is complete in the single Stage 20C acceptance
checkpoint containing its implementation and synchronized durable
documentation. The checkpoint is intentionally named rather than
self-referenced by hash and does not authorize a push, Stage 20D, or later
work.

## Start gate evidence

- Branch: `rewrite/greenfield-foundation`.
- Accepted Stage 20B checkpoint: `08d970a6134e1d55d93180910971247e7c7342ec`.
- Direct parent: `e7e2dcadaf9e2900e8b4b65ab14d00745bee8ba4`.
- Upstream matched the accepted checkpoint when this contract was approved.
- The worktree was clean when this contract was approved.
- The Stage 20B validation directory contained its accepted evidence and
  reported `PASS`.
- `assets/HolidayMugs_2024_2025.toniator` had SHA-256
  `717fd7e03cba2c92d2730db05028c39b7a8e8de8e0bcc7054abcb3c56d5e5947`.
- Ignored evidence remains evidence, not durable product documentation.
- Stage 20C was not started before this contract was accepted.
- `semantic-map` 0.1.1 was used only for bounded navigation. Its index was
  stale after Stage 20B, so this contract makes no freshness or absence claim
  from that index.

The implementation start gate must revalidate these facts against the current
checkout before any source edit.

On 2026-08-13, the user explicitly overrode two assumptions that the planning
agent recorded without full checkout visibility: the approved untracked
`Stage20C_planning_contract.md` is permitted preexisting dirt, and the existing
checkout-matching Stage 20B reviewer `PASS` under `.codex-work/` satisfies the
Stage 20B evidence prerequisite without a `target/validation/stage-20b/`
directory. Branch, checkpoint ancestry, upstream, and protected asset hashes
were revalidated and matched before Stage 20C entered `In progress`.

## Goal

Add a document-owned store for reusable authored open paths and closed shapes.
The store has stable document-scoped IDs, explicit validation, authoritative
commands and history behavior, descriptors, deterministic persistence, and an
exact conversion boundary to the Stage 20B `CurvePath` construction geometry.

Stage 20C stops at construction geometry. It establishes no pattern consumer,
guide behavior, shape realization, canonical rendering path, or UI.

## Non-goals

Stage 20C does not add or change:

- pattern consumers, guide evaluation, or along-curve behavior;
- shape realization, strokes, regions, graphs, Voronoi, or composites;
- canonical path, preview, render, PNG, or SVG behavior;
- engine caches or derived-state persistence;
- CLI or GTK/libadwaita behavior;
- a schema-version or container-version bump;
- compatibility for obsolete pre-release formats;
- fixtures, immutable assets, protected specifications, or `ToniatorLegacy/`;
- presets or preset semantics;
- Stage 20D or later work;
- commits or pushes.

## Domain API and ownership

The domain model owns these public concepts:

```rust
pub struct AuthoredStructureId(pub u64);

pub struct AuthoredPoint2 {
    pub x: f64,
    pub y: f64,
}

pub enum AuthoredCurveSegment {
    Line {
        start: AuthoredPoint2,
        end: AuthoredPoint2,
    },
    CubicBezier {
        start: AuthoredPoint2,
        control_1: AuthoredPoint2,
        control_2: AuthoredPoint2,
        end: AuthoredPoint2,
    },
}

pub enum AuthoredStructureKind {
    OpenPath,
    ClosedShape,
}

pub struct AuthoredStructureDraft {
    // Private validated kind and segments.
}

pub struct AuthoredStructure {
    // Private stable ID, kind, and segments.
}
```

The public construction and access surface is:

```rust
AuthoredStructureDraft::new(kind, segments)
AuthoredStructure::new(id, kind, segments)
AuthoredStructureDraft::kind()
AuthoredStructureDraft::segments()
AuthoredStructure::id()
AuthoredStructure::kind()
AuthoredStructure::segments()
Document::authored_structures()
Document::authored_structure(id)
Document::with_source_and_authored_structures(...)
Document::with_source_topology_and_authored_structures(...)
```

Existing document constructors produce an empty authored-structure store.

An authored structure has no resource name or UI label. Stage 20C does not add
a second reference identity, a separate serialized geometry type, per-segment
IDs, knots, smoothing flags, winding, fill, stroke, style, content fingerprint,
or cache identity.

## Validation and bounds

Validation is authoritative in the domain model and applies before a document
mutation is committed.

- Maximum authored structures per document: 4,096.
- Maximum segments per authored structure: 4,096.
- Maximum total authored segments per document: 65,536.
- IDs are nonzero and unique within their own document-scoped namespace.
- Store order is creation order.
- Every authored structure contains at least one segment.
- Every point and control coordinate is finite.
- Adjacent segments have exact C0 continuity: the previous end equals the next
  start component-for-component.
- An `OpenPath` remains open by declared kind even when its endpoints coincide.
- A `ClosedShape` has an exact seam: its final end equals its first start.
- Validation never manufactures a closing segment.
- Duplicate points, zero-length segments, stationary cubic segments, zero-area
  shapes, one-segment closed shapes, and fully coincident shapes are accepted.
- A closed shape does not imply interior, winding, region, fill, render, or
  realization semantics in this stage.
- Bound, validation, lookup, and command failures are checked and atomic.

Stable validation paths are:

```text
authored_structures.limit
authored_structures.segment_limit
authored_structures.id
authored_structures.segments.empty
authored_structures.segments.limit
authored_structures.segments.coordinates
authored_structures.segments.continuity
authored_structures.closure
authored_structures.reference
authored_structures.edit.stale
authored_structures.edit.noop
authored_structures.remove.missing
authored_structures.remove.referenced
```

Focused tests lock the associated messages as well as the paths.

## Stage 20B geometry boundary

Geometry adds this exact conversion boundary:

```rust
CurvePath::from_authored_structure(
    structure: &AuthoredStructure,
) -> Result<CurvePath, CurveError>
```

The conversion maps points bit-for-bit, preserves line and cubic variants and
declared closure, and delegates to the accepted Stage 20B constructors and
validation. It does not introduce a second duplicate-point policy or relax a
Stage 20B invariant.

The result remains construction geometry. It is not a canonical path, rendered
path, export path, guide, realized shape, graph, region, or cache entry.

## References, identity, and copying

- A reference is the raw `AuthoredStructureId` and resolves only within its
  owning document.
- Persistence and history preserve IDs exactly.
- Replacing content preserves the ID and store position; all future consumers
  of that ID observe the shared edit.
- Duplicating allocates a fresh ID, appends the copy, and retargets nothing.
- Removal is allowed only when no document-owned object references the ID.
- Stage 20C introduces no live reference-bearing pattern field.
- Stage 20D may introduce an `OpenPath` guide reference.
- Stage 20E may introduce `ClosedShape` marks.
- Copying a future owner and deciding whether to share or duplicate and retarget
  its structure is deferred to the stage that introduces that owner.

## Authoritative commands and history

The domain command surface is:

```rust
AddAuthoredStructure {
    draft,
}

DuplicateAuthoredStructure {
    structure_id,
}

ReplaceAuthoredStructure {
    base_structure,
    replacement,
}

RemoveUnreferencedAuthoredStructure {
    structure_id,
}
```

Command behavior is fixed as follows:

- Addition allocates `max(existing ID) + 1` using checked arithmetic and
  appends the new structure.
- Duplication allocates the same way, copies kind and segments exactly, appends
  the result, and returns or otherwise exposes the fresh ID through the
  established command-result convention.
- Replacement requires an exact `base_structure` match, rejects stale edits,
  preserves the target ID and ordering, and rejects an exact no-op.
- Removal rejects missing or referenced IDs.
- Every failure leaves document state and history unchanged.
- Successful commands participate in the existing authoritative history.
- Undo and redo restore exact store order, IDs, kind, segments, and coordinates.

## Descriptors

Descriptors remain separate from values, validation, persistence, and UI:

```rust
pub enum AuthoredStructureFieldId {
    Kind,
    Segments,
}

pub enum AuthoredCurveSegmentKind {
    Line,
    CubicBezier,
}

pub struct AuthoredStructureFieldDescriptor {
    // Target, field, value kind, choices, bounds, shared-edit semantics,
    // and invalidation authority.
}

authored_structure_field_contracts()
Document::authored_structure_descriptors()
```

Descriptors define the editing contract and expose stable field identity,
value kind, allowed choices or bounds, shared-edit behavior, and invalidation.
They contain no current values, resource labels, UI layout, fallback ownership,
or duplicate validation authority. They are not serialized.

## Invalidation and future cache identity

There are no Stage 20C consumers, so successful commands report an empty
affected-consumer set.

- Add, duplicate, and remove have `Family` invalidation.
- Replacing an `OpenPath` has `Family` invalidation.
- Replacing a `ClosedShape` has `Realization` invalidation.
- A kind transition uses the earliest affected level; it is `Family` if either
  side is `OpenPath`, otherwise `Realization`.
- Existing revision tokens continue to reject stale operations.
- Stage 20C does not change an engine cache or cache schema.

The future identity contract is recorded now: open-path content participates in
family identity, closed-shape content participates in realization identity, and
the resource ID alone is insufficient. If a future resource is used in more
than one role, the earliest affected level wins. No derived identity is
persisted in Stage 20C.

## Persistence

Keep container schema 1, document schema 2, the existing v1 parser and
migration path, and preset schema 1.

The v2 document DTO gains:

```rust
#[serde(default, skip_serializing_if = "Vec::is_empty")]
authored_structures: Vec<AuthoredStructureDtoV2>
```

The DTO records authored structures in store order with explicit ID and kind,
tagged line or cubic segments, and `f64` point coordinates.

- Existing v2 documents missing the field load with an empty store.
- v1 migration creates an empty store.
- Saving an empty store omits the field and preserves the accepted old-v2
  serialized bytes and hash.
- A populated store round-trips IDs, order, kind, segment variants, and numeric
  coordinates exactly and deterministically.
- No migration report entry is added for the defaulted empty store.
- Unknown document or container versions remain rejected.
- History, descriptors, resolved geometry, caches, drafts, and UI state are not
  serialized.
- Presets remain unchanged.

## Implementation allowlist

The parent may update these coordination files after the explicit implementation
start request:

- `Stage20C_planning_contract.md`, only if an explicitly approved contract
  correction is required;
- `docs/GREENFIELD_REWRITE_PLAN.md`, Stage 20C status only;
- `ProgressTracker.md`, Stage 20C status only;
- checkout-local reviewer evidence/cache files.

The single implementation writer owns only:

- `crates/toniator-domain/src/lib.rs`;
- `crates/toniator-domain/tests/authored_structures.rs`;
- `crates/toniator-geometry/src/curves/mod.rs`;
- `crates/toniator-geometry/src/curves/authored.rs`;
- `crates/toniator-geometry/tests/authored_structures.rs`;
- `crates/toniator-io/src/lib.rs`;
- the focused Stage 20C persistence test file selected from the existing test
  layout;
- checkout-local Stage 20C implementation evidence/cache files;
- `.codex-work/semantic-map/USAGE_EVALUATION.md` only when a demonstrated
  semantic-map inefficiency requires an evidence-backed entry.

No Cargo manifest or lockfile edit is permitted. Everything else is excluded,
including pattern, engine, render, CLI, app, asset, fixture, specification,
legacy, preset, canonical-path, and earlier validation files.

If the smallest coherent implementation requires a file outside this allowlist,
the writer stops and returns the required change for approval.

## Focused tests

The Stage 20C tests use these responsibilities and names:

```text
authored_structures_validate_finite_explicit_open_and_closed_topology
authored_structure_commands_allocate_duplicate_replace_remove_and_history_atomically
authored_structure_descriptors_match_commands_validation_and_invalidation
authored_structure_ids_resolve_stably_without_name_or_position_aliases
document_authored_structures_resolve_to_exact_stage20b_curve_paths
stage20c_authored_structures_round_trip_ids_order_topology_and_coordinates
stage20c_empty_store_preserves_accepted_v2_bytes_and_v1_migration
stage20c_existing_documents_load_with_empty_authored_structure_store
stage20c_invalid_resource_json_rejects_before_document_commit
```

Together they cover bounds, exact topology, accepted degeneracies, stable
validation paths and messages, checked ID allocation, stale and no-op edits,
atomic failure, order, references, history, descriptors, invalidation, exact
Stage 20B conversion, deterministic persistence, old-v2 byte preservation, v1
migration, missing-field defaults, and pre-commit rejection of invalid JSON.

## Verification gate

Run only the focused Stage 20C and directly relevant foundational checks:

```bash
cargo fmt --all -- --check
cargo test -p toniator-domain --test authored_structures
cargo test -p toniator-geometry --test authored_structures
cargo test -p toniator-geometry --test curve_paths
cargo test -p toniator-io --test persistence stage20c_
cargo check -p toniator-domain -p toniator-geometry -p toniator-io --all-targets
cargo clippy -p toniator-domain -p toniator-geometry -p toniator-io --all-targets -- -D warnings
bash scripts/validate_architecture.sh
git diff --check
git diff --exit-code -- Cargo.toml Cargo.lock crates/toniator-domain/Cargo.toml crates/toniator-geometry/Cargo.toml crates/toniator-io/Cargo.toml
git diff --exit-code -- ToniatorLegacy 'Project Specification' assets fixtures
sha256sum assets/HolidayMugs_2024_2025.toniator assets/raster-sample.png assets/vector-sample.svg
git status --short --branch --untracked-files=all
```

Stage 20C is headless-only. It requires no GTK launch, private Wayland session,
visual artifact, or manual desktop acceptance.

## Documentation and review roles

Every touched non-trivial named Rust function, method, and test receives literal
`///` responsibility documentation that records relevant authority boundaries,
invariants, bounds, side effects, and applicable `# Errors`, `# Panics`, or
`# Safety` conditions.

One desktop implementer is the only source writer. One read-only test reviewer
performs the independent review. Any correction returns to the same writer.
The parent alone owns integration, stage documentation, status transitions,
evidence reconciliation, acceptance, and any checkpoint operation.

## Approval gates

1. This file records the accepted planning contract.
2. Implementation starts only after an explicit user request; the parent then
   rechecks the start gate and marks Stage 20C in progress.
3. One writer makes only allowlisted changes and runs focused verification.
4. A read-only reviewer checks the implementation and evidence.
5. After corrections and final verification, the parent may mark Stage 20C
   `Implemented — awaiting review` and must stop with the work uncommitted.
6. Explicit user acceptance is required before marking the stage
   `Accepted — awaiting checkpoint`.
7. A separate explicit authorization is required to commit. Nothing in this
   contract authorizes a push.
8. The final Stage 20C handoff stops without beginning Stage 20D.

## Mandatory stop conditions

Stop, preserve the worktree, and report the decision needed if implementation
would require:

- an excluded path or a Cargo manifest/lockfile edit;
- document schema 3, a new migration, or removal of the v1 parser;
- a change to accepted old-v2 empty-store bytes;
- a pattern, evaluator, cache, canonical-path, render, export, CLI, or GTK
  consumer;
- a live reference-bearing owner before its approved stage;
- an invalidation category outside the existing authority;
- relaxation of a Stage 20B curve invariant;
- a product decision not fixed by this contract.
