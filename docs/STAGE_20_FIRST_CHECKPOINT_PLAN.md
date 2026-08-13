# Stage 20A implementation record: Structural Site Interchange

## Status, goal and first dependency boundary

**Complete at implementation checkpoint `b7fbd81`.** User acceptance and
parent review are complete. This document records the executed Stage 20A
contract and verified outcome; it does not authorize Stage 20B. Stage 19B is
complete at `b0b84e4`, and the remaining Stage 20+ work remains planned.

**Executed goal.** Establish `FamilySiteSet` as the one deterministic,
truthful derived-site authority for typed family results, while retaining
accepted circle, cache, PNG and SVG behavior. `TypedFamilyOutput` now publishes
an opaque shared site set: generalized along-guide and random results retain
their actual provenance. A private compatibility adapter is used only by the
current circle realizers and is not inherited by topology or regions.

This is the first boundary because it unlocks curves, graphs, Voronoi and shape
marks without selecting a curve language, graph algorithm, region treatment or
GTK workflow. It changes one derived authority, not persisted user intent.

**Verified non-goals.** No domain schema, commands, descriptors, invalidation
levels, path/curve/graph/region algorithm, canonical primitive,
renderer/export behavior, artistic preset/name behavior, container/DTO
change/migration, or GTK workflow changed. The checkpoint was headless-only;
no GTK/Sway evidence was required.

## Exact schema/domain/evaluator/persistence/UI boundary

`toniator-geometry` now owns this exact public derived contract:

```rust
pub struct FamilySiteId {
    pub mechanism_id: PatternMechanismId,
    pub ordinal: usize,
}
pub enum FamilySiteProvenance {
    GuideIntersection { contributors: Vec<GuideInstanceId> },
    AlongGuide {
        guide_id: GuideInstanceId, guide_order: usize, sequence: i64,
        absolute_arc_position_bits: u64, local_arc_position_bits: u64,
    },
    Random {
        candidate_ordinal: usize, accepted_ordinal: usize,
        exclusion_neighbor_ordinal: Option<usize>,
    },
}
pub struct FamilySite {
    pub id: FamilySiteId, pub position: Point2, pub scope: SiteScope,
    pub provenance: FamilySiteProvenance,
}
pub struct FamilySiteSet { /* private family_fingerprint and ordered sites */ }
```

`FamilySiteSet::new(family_fingerprint: String,
product_mechanism_id: PatternMechanismId, sites: Vec<FamilySite>) ->
Result<FamilySiteSet, FamilySiteError>` accepts only a nonempty fingerprint.
Every site must use that product mechanism ID and an ordinal exactly equal to
its vector position, establishing contiguous deterministic evaluator emission
order without sorting or renumbering. Its public accessors are exactly
`family_fingerprint(&self) -> &str`,
`product_mechanism_id(&self) -> PatternMechanismId`,
`sites(&self) -> &[FamilySite]`, `iter(&self)`, `len(&self)`, and
`is_empty(&self)`. The supplied family fingerprint is the set identity; there
is no new site-set hash or cache key.

Validation rejects a zero product mechanism ID at
`family_sites.product_mechanism_id`, a site mechanism mismatch at
`family_sites.id.mechanism_id_mismatch`, a non-contiguous ordinal at
`family_sites.id.ordinal`, and a duplicate ID at
`family_sites.id.duplicate`; it rejects non-finite coordinates at
`family_sites.position` and an empty fingerprint at
`family_sites.family_fingerprint`. `GuideIntersection` requires at least
two unique nonzero `GuideInstanceId` contributors at
`family_sites.provenance.guide_intersection.contributors`.
`FamilySiteSet` preserves authored/evaluator contributor order and validates
uniqueness without imposing numeric sorting.
`AlongGuide` requires nonzero `guide_id.dimension_id` and finite values when
the two arc bit fields are decoded with `f64::from_bits`; failures use
`family_sites.provenance.along_guide.guide_id` and
`family_sites.provenance.along_guide.arc_position`. `Random` requires
`accepted_ordinal <= candidate_ordinal` and, when present,
`exclusion_neighbor_ordinal < accepted_ordinal`, at
`family_sites.provenance.random.ordinals`. `SiteScope` is supplied by the
evaluator and is never inferred by the set.
For stable diagnostics, constructor validation checks the fingerprint/product
ID first, then duplicate IDs across the supplied vector, then each site in
emission order for mechanism match, ordinal, finite position and provenance.

`toniator-patterns` changed only the derived pipeline:

- add `TypedFamilyOutput::site_set()`; every existing variant publishes a
  truthful shared set;
- derive it from existing `GridFamilyOutput`,
  `GeneralizedStraightGuideOutput` / `GeneralizedSiteProvenance`, and
  `RandomSiteProvenance`, not fabricated contributors;
- make `TypedFamilyOutput` an opaque public struct with private
  `family: FamilyCapability`, `sites: FamilySiteSet`,
  `diagnostics: Option<RandomSiteDiagnostics>` and
  `structure: TypedFamilyStructure`. Its only public
  accessors are `family()`, `family_fingerprint()`, `site_set()` and
  `random_diagnostics()`; remove public `grid()`. The public dedicated
  `evaluate_straight_grid()` / `GridFamilyOutput` diagnostic API remains
  unchanged, but generalized and random typed output cannot expose a fabricated
  grid.
- define private truthful `TypedFamilyStructure` metadata: family coverage,
  guides, support radius, guard steps, antialias margin and generation domain;
  it contains no `IntersectionSite` or fabricated `GridFamilyOutput`.
  Define private
  `fn adapt_family_sites_for_current_circular_marks(&TypedFamilyOutput) -> GridFamilyOutput`
  and call it only inside `realize_typed_mapped_outputs`,
  `realize_typed_source_color_outputs` and
  `realize_typed_diagnostic_outputs`. For straight intersections it may use
  the retained real straight grid. For all other products it builds transient
  compatibility data only. It must reproduce accepted bytes: generalized
  intersections place their first two actual contributors in `SiteId` and
  retain all contributors (they always have at least two; no first-contributor
  duplication fallback is permitted);
  along-guide uses `guide_id`, then
  `GuideInstanceId { dimension_id: guide_id.dimension_id, index: sequence }`,
  and one contributor; random uses product mechanism + accepted index, then
  process mechanism + seed in `SiteId`, and contributors in the present
  process+seed then product+accepted order. This is the sole source of transient
  `CanonicalCircleMark.source_site_id` / contributor bytes.
- make `TypedRealizationProvenance` carry/clone the truthful
  `FamilySiteSet` rather than variant-specific generalized/random vectors.
  Existing realization-fingerprint bytes remain unchanged: diagnostic
  provenance and transient compatibility substitution are explicitly excluded
  from that hash; the realizer consumes only the transient adapter.

No command/descriptors change: existing `PatternDefinition`,
`PatternMechanism`, `PatternOutputLayer`, `DocumentCommand`,
`PropertyDescriptor` and `InvalidationLevel` remain the accepted authority.
No UI change: Pattern Editor and channel controls keep consuming them. No
persistence change: `toniator-io` current-v2 DTOs, v1 parser/migration and
preset records remain untouched. Canonical geometry/render/export consume
existing circles exactly; `FamilySiteSet` adds no canonical mark/path/region.

Changed implementation files are exactly:

- `crates/toniator-geometry/src/lib.rs` and a focused test under
  `crates/toniator-geometry/tests/`;
- `crates/toniator-patterns/src/lib.rs` and focused contract tests under
  `crates/toniator-patterns/tests/`;
- `crates/toniator-engine/tests/document_evaluation.rs`;
- `crates/toniator-engine/tests/scheduler.rs`.

`toniator-domain`, `toniator-io`, `toniator-render`, `toniator-app`,
`toniator-cli`, manifests, fixtures, protected specifications, assets and
Legacy were excluded.

## Compatibility, cache and round-trip contract

`FamilySiteSet` is evaluator-derived family output, not document state. It adds
no cache tier/revision/invalidation level/command/descriptor/persistence/preset
field. Its identity is exactly the existing authoritative family inputs.
Existing first-miss behavior remains: family edits miss family and downstream;
realization retains family; presentation retains family and realization;
artwork-weighted random retains the accepted conditional source identity. No
regenerated site, provenance, adjacency/topology or cache entry serializes.

The persistence/preset requirement is negative plus regression: unchanged
current-v2 documents and presets round-trip through existing tests and produce
identical authoritative evaluation/canonical output. If proving that requires an
IO edit, stop rather than expanding this plan.

## One writer, review and documentation-on-touch

One `desktop_implementer` was the sole writer. A read-only review examined
identity/provenance, adapter isolation and cache regression; parent reviews the
report/evidence. Neither writer nor reviewer updates `ProgressTracker.md`.

For every touched non-trivial Rust function, method and test, add literal
`///` documentation in the format expected by semantic-map: present-tense
responsibility, authority boundary, relevant invariants/bounds, side effects,
and applicable `# Errors`, `# Panics` or `# Safety`. This is touch-only,
not a repository-wide pass. Durable documentation is reconciled here after
review under parent authority.

## Tests and natural-input evidence

The accepted focused tests prove:

1. Deterministic ordering, unique IDs and truthful provenance for straight and
   multiway intersections, along-guide sites, and raw/even/clustered/
   artwork-weighted random sites; duplicate IDs, non-finite positions and
   inconsistent provenance reject with stable paths.
2. Every `TypedFamilyOutput` publishes the common set; random/along-guide
   products never claim synthetic intersections; current private circle
   adaptation stays exact.
3. Existing generalized/random complete-document evaluation, history,
   scheduler/cancellation/stale-ticket/cache acceptance retain scene identity,
   native pixels and SVG text.

Add these exact focused test names:

- `family_site_set_contract_rejects_invalid_ids_order_positions_and_provenance`
  in `toniator-geometry/tests/primitives.rs`;
- `typed_family_outputs_publish_truthful_family_site_sets` in
  `toniator-patterns/tests/grid_family.rs`;
- `current_circle_compatibility_adapter_preserves_accepted_site_id_and_contributor_bytes`
  in `toniator-patterns/tests/grid_family.rs`;
- `stage20a_family_site_interchange_preserves_complete_document_cache_and_output_identity`
  in `toniator-engine/tests/document_evaluation.rs`;
- `stage20a_natural_png_and_svg_inputs_preserve_current_circle_outputs` in
  `toniator-engine/tests/document_evaluation.rs`.

The Stage 20A-owned engine test loads immutable PNG and SVG
at 1024×1024 and 900×620 through complete-document evaluation, checks the
documented source hashes plus current circle scene/raster/SVG identities, and
treats SVG live text structurally rather than as a font-dependent raster
golden. It does not treat an old path as sufficient evidence.

The focused verification gate ran after implementation:

```bash
cargo fmt --all -- --check
cargo test -p toniator-geometry --test primitives family_site_set_contract_rejects_invalid_ids_order_positions_and_provenance
cargo test -p toniator-patterns --test grid_family typed_family_outputs_publish_truthful_family_site_sets
cargo test -p toniator-patterns --test grid_family current_circle_compatibility_adapter_preserves_accepted_site_id_and_contributor_bytes
cargo test -p toniator-engine --test document_evaluation stage20a_family_site_interchange_preserves_complete_document_cache_and_output_identity
cargo test -p toniator-engine --test document_evaluation stage20a_natural_png_and_svg_inputs_preserve_current_circle_outputs
cargo check -p toniator-geometry -p toniator-patterns -p toniator-engine --all-targets
cargo clippy -p toniator-geometry -p toniator-patterns -p toniator-engine --all-targets -- -D warnings
bash scripts/validate_architecture.sh
git diff --check
git diff --exit-code -- ToniatorLegacy 'Project Specification'
git status --short --branch --untracked-files=all
```

The new Stage 20A engine test is the required natural-input evidence. PNG may
retain exact existing byte/pixel assertions. SVG live text is structural/
canonical evidence, never a portable font-dependent raster golden. No artifact
is required unless a focused test needs one; it must be under
`target/validation/stage-20a/`.

This is demonstrably headless, so no private GTK/Sway evidence was needed.
Private Sway never substitutes for manual GNOME Shell/Mutter review; none is
needed for this no-UI checkpoint. GTK checkpoints retain manual parent/user
acceptance where applicable.

## Gates and authority

**Historical start gate.** This execution contract was valid only when HEAD and upstream
were both `c95c8d8`, `b0b84e4` was the accepted implementation ancestor of
`c95c8d8`, and the branch is `rewrite/greenfield-foundation`. Parent had
added only the parent-owned Stage 20A **In progress** lines in the durable
plan/tracker; the only allowed preexisting dirt was those lines, these three
approved planning documents, and user-owned
`assets/HolidayMugs_2024_2025.toniator`. The writer rechecked those facts,
Stage 19B complete/20+ planned, protected paths, `AGENTS.md`, roadmap/tracker,
this plan/decomposition, applicable Addendum/PatternSchema/ModuleStructure
sections and checkout-matching evidence. Any mismatch would have stopped the
run. Checkpointing these planning documents before implementation would have
invalidated the Goal and required regeneration against the new HEAD.

**Implementation-review gate.** Focused tests and architecture/protected-path
checks passed; read-only review confirmed a real `FamilySiteSet`, truthful
provenance and private adapter confinement.

**Automated acceptance-evidence gate.** Generation-marked,
checkout-aware evidence under `.codex-work/agents/desktop-implementer/`:
changed files/symbols, commands, baseline/artifacts, verified findings versus
uncertainty, cache/invalidation, HEAD/worktree and invalidation conditions.
Evidence is not acceptance.

**Parent-approval gate.** Parent reviewed the diff/evidence and the user
accepted the implementation. Only parent/user may transition tracker status or
call a stage accepted.

**Checkpoint/commit gate.** The implementation checkpoint is `b7fbd81`.
Documentation closeout remains separate; push is never implied.

**Final stop gate.** Stage 20A is complete at `b7fbd81`; do not begin Stage
20B. Stage 20B is the next planning boundary and remains Planned.

The executed authority allowed only the allowlisted implementation, test, and
evidence changes and iteration on failures. It did not authorize path expansion,
product-decision changes, protected inputs/assets/user-document edits, tracker
transitions, commits, pushes, publication, deployment, or a later checkpoint.
A material ambiguity or essential excluded change would have invoked the
blocked-stop contract in the Goal pack.
