# Stage 20E1 — Normalized Mark Fill and Coverage

## Status and authority

**Complete in the Stage 20E1 acceptance checkpoint.** This bounded checkpoint replaces the temporary
absolute-diameter/support-capability model before authored closed shapes consume
mark response in Stage 20E2. It begins from accepted Stage 20D checkpoint
`ba496fd67c2c46ab63a8fa2e77fef98ba2edfee6`.

The accepted implementation and independent repair re-review are complete and
PASS. Current documents accept schema v3 only and presets accept format v2 only;
normalized minimum/maximum fill, rotation offset, and additional-margin controls
are the current persisted and existing GUI/CLI surfaces. No Stage 20E2 work is
included in this checkpoint.

Before relying on semantic-map for a subsequent Stage 20E2 implementation,
refresh it with `semantic-map update --full` and verify the result against the
accepted checkout. This checkpoint's independent read-only repair re-review
reports PASS. Stage 20E2 is a separate approved contract and remains Planned,
not started here.

## Outcome and non-goals

Stage 20E1 gives every ordinary family site a deterministic nominal cell basis
and changes the channel response from absolute document-unit mark diameter to a
normalized fill scale. A value of `1.0` gives a circle the diameter of the
nominal cell's longer diagonal, reaching the nominal corners. The hard range is
`0.0..=2.0`, permitting bounded intentional overlap. New documents default to
minimum fill `0.0`, maximum fill `1.0`, and rotation offset `0.0` degrees.

This checkpoint does not add an authored-shape prototype, closed-path
rasterization, a shape importer/editor, response curves, a second response
polarity, multithreaded realization, fractional progress, or any Stage 20E2+
mechanism. Existing source-mapping inversion remains the only response polarity.
The Stage 20F opening UI slice owns the indeterminate `Updating preview...`
indicator and authored-shape editing.

## Authoritative model and formulas

Replace `MarkGeometryResponse { minimum_size, maximum_size }` with:

```rust
pub struct MarkGeometryResponse {
    pub minimum_fill: f64,
    pub maximum_fill: f64,
    pub rotation_offset_degrees: f64,
}
```

Both fill values must be finite and within `0.0..=2.0`, minimum must not exceed
maximum, and the rotation offset must be finite. Linear source response remains:

```text
fill = minimum_fill + signal * (maximum_fill - minimum_fill)
diameter = fill * nominal_cell_diameter
radius = diameter / 2
```

`signal` retains the accepted source mapping, clamping, alpha, and
SourceColorAlpha zero-alpha suppression rules. The channel rotation offset is
persisted and participates in realization identity now; circles remain visually
invariant. It is composed after output-layer Fixed/Tangent/Normal orientation
when Stage 20E2 makes orientation visible.

Add an immutable finite positive `NominalCellBasis` to every `FamilySite`, with
two document-space vectors and a derived diameter:

```text
nominal_cell_diameter = max(length(axis_a + axis_b), length(axis_a - axis_b))
```

The evaluator derives the axes in constant time while emitting each site; no
nearest-neighbor search is allowed:

- Guide intersections consider every ordered nonparallel contributor pair.
  For a pair, each local unit guide tangent is scaled by the other contributor's
  resolved normal spacing. Choose the pair with the smallest positive finite
  longer diagonal, breaking equal-bit ties by contributor order. This is the
  nominal local parallelogram basis; it deliberately does not divide by the
  intersection angle, so near-parallel contributors cannot create unbounded
  artistic marks.
- Along-guide sites use `axis_a = local_unit_tangent * resolved_along_interval`
  and `axis_b = local_unit_normal * resolved_transverse_spacing`. Repeated
  dimensions use their resolved repetition spacing; a Single dimension uses the
  directional channel-density spacing along the local normal.
- Random sites use an axis-aligned square-equivalent density cell with
  `side = sqrt((canvas_width / across_x) * (canvas_height / across_y))`.
  Raw, even, clustered, uniform, and artwork-weighted random families share this
  response model. A uniform mark size is expressed by equal minimum and maximum
  fill.

Family-site validation rejects absent, non-finite, zero-length, or
non-deterministically ordered bases before publication. The basis and diameter
participate in family/site fingerprints. Existing site identity, order, scope,
and provenance remain unchanged.

## Coverage, invalidation, and caching

Remove persisted `CoveragePolicy::maximum_support_radius` and its property,
command, preset, DTO, and visible-mark-capability semantics. Replace the stored
coverage field with finite nonnegative `additional_margin`, default `0.0`, as
the Addendum describes. `guard_steps` remains structural.

Coverage planning computes a conservative `maximum_nominal_cell_diameter` from
the same family/density/repetition inputs before allocating sites. Required mark
support is:

```text
maximum_fill * maximum_nominal_cell_diameter / 2
```

The planner inflates the generation envelope by required mark support,
`additional_margin`, antialiasing support, and topology-specific guards before
family generation. The realized per-site radius must not exceed that preflight
bound. This replaces the temporary `4.5` check; there is no independent authored
support ceiling.

Mark-response commands continue to report `Realization` invalidation. Minimum
fill and rotation offset retain a compatible cached family. A maximum-fill edit
may reuse a family only when the cached family envelope records support at least
as large as the newly required envelope; otherwise the family key misses and a
new complete envelope is generated. A lower maximum may reuse a broader family,
with realization filtering based on final mark/canvas intersection. Failed,
cancelled, stale, or superseded work publishes no cache transaction.

Visible-mark exclusion derives its conservative separation from the active
maximum realized support. It must not retain or recreate a pattern-owned 4.5
ceiling.

## Commands, persistence, and frontends

Rename descriptor/current-value/edit/command surfaces to Minimum fill and
Maximum fill, add Rotation offset, and preserve typed history, stale/no-op,
copy-on-edit, aggregate affected-channel, and atomic failure behavior. Bounds
metadata is exactly `0.0..=2.0`; rotation uses degrees with no artificial range.

This is an intentional pre-release format break:

- Keep the outer `.toniator` container at version 1 because its framing is
  unchanged; make document schema version 3 the only accepted document schema.
- Make preset format version 2 the only accepted preset schema.
- Remove document-v1 migration and document-v2/preset-v1 decoding. Reject those
  versions deterministically; do not infer normalized fill from old absolute
  values at load time and do not add a size-basis compatibility mode.
- Encode `minimum_fill`, `maximum_fill`, `rotation_offset_degrees`, and
  `additional_margin` explicitly and deterministically in current DTOs.
- Rewrite `assets/HolidayMugs_2024_2025.toniator` to current v3. Replace and
  rename `assets/raster-sample-v1.toniator` and
  `assets/vector-sample-v1.toniator` as `assets/raster-sample.toniator` and
  `assets/vector-sample.toniator`. Convert each old channel against the
  document's representative nominal cell diameter to preserve average visual
  scale, then update `assets/README.md`, fixture hashes, and current tests.
- Never modify `assets/raster-sample.png` or `assets/vector-sample.svg`.

The existing GTK and CLI controls use the labels `Minimum fill` and `Maximum
fill`, expose the same hard `0.0..=2.0` bounds, and explain that `1.0` reaches
the nominal cell corners. CLI create/direct render accepts normalized fill and
uses the new `0.0/1.0` defaults. CLI validate/render/inspect consume current-v3
containers. Neither frontend receives authored-shape authoring in this stage.

## Allowlist and protected scope

The sole writer may change only:

- `crates/toniator-domain/**`, `crates/toniator-geometry/**`,
  `crates/toniator-patterns/**`, `crates/toniator-engine/**`,
  `crates/toniator-io/**`, and `crates/toniator-cli/**`;
- narrowly necessary `crates/toniator-app/**` control labels/bounds and compile
  completeness, with no shape editor or preview-progress work;
- focused current tests in those crates and narrowly necessary Cargo manifests;
- the three tracked `.toniator` documents, their two approved renames, and
  `assets/README.md`;
- this contract, `ProgressTracker.md`, the Stage 20+ roadmap/goal documents,
  ignored checkout-aware evidence, and derived artifacts under
  `target/validation/stage-20e1/`.

Protected and excluded: `Project Specification/**`, immutable PNG/SVG/video and
Reddit source assets, `ToniatorLegacy/**`, unrelated documentation, GTK shape
editing/progress UI, renderer algorithms, authored-shape realization, and every
Stage 20E2+ implementation path.

Every touched non-trivial Rust function, method, and test receives literal
present-tense `///` documentation under the repository rule.

## Verification and stop gate

Focused tests must prove:

- exact basis/diameter formulas for rectangular, anisotropic, rotated,
  selected-multiguide, straight/curved along-guide, raw/even/clustered random,
  and guard sites without changing identity or provenance;
- 0/1 defaults, 0..2 bounds, constant fills, linear response, rotation bits,
  command/history/stale/no-op/atomic behavior, and absence of the 4.5 ceiling;
- complete edge coverage at fill 1 and bounded overlap at fill 2 across rotation,
  translation, anisotropic density, random coverage, and curved guides;
- broader-family reuse, required-envelope misses, limits, cancellation,
  supersession, transactional publication, and deterministic identities;
- v3/current-preset round-trip and deterministic bytes, explicit rejection of
  document v1/v2 and preset v1, current fixture hashes, and no migration route;
- matching GUI/CLI labels, bounds, defaults, validation, and direct/container
  render behavior.

Exercise both immutable source artworks at intrinsic dimensions. Store raw PNG,
SVG, cache/identity diagnostics, and a report under
`target/validation/stage-20e1/`; preserve native RGBA and the live-text/font
caveat. Use the private Wayland harness for the affected GTK control wording,
bounds, editing, focus, accessibility, preview update, logs, and screenshots;
stop the session at handoff. This is automated Sway/wlroots evidence, not manual
GNOME/Mutter acceptance.

Run the focused current test filters named in writer evidence, then:

```bash
cargo fmt --all -- --check
cargo check -p toniator-domain -p toniator-geometry -p toniator-patterns \
  -p toniator-engine -p toniator-io -p toniator-cli -p toniator-app --all-targets
cargo clippy -p toniator-domain -p toniator-geometry -p toniator-patterns \
  -p toniator-engine -p toniator-io -p toniator-cli -p toniator-app \
  --all-targets -- -D warnings
bash scripts/validate_architecture.sh
git diff --check
sha256sum assets/raster-sample.png assets/vector-sample.svg
git status --short --branch
```

Audit the allowlist, protected-spec hashes, fixture renames, absence of old
runtime schema decoders, and no Stage 20E2 pull-forward. The implementation
evidence and independent read-only repair re-review are recorded under
`.codex-work/`; the accepted named checkpoint contains the implementation and
synchronized durable documentation. No Stage 20E2 work is part of this
checkpoint. Push remains separately unauthorized.
