# Toniator Module Structure

**Status:** Normative repository and dependency plan  
**Applies to:** Greenfield Toniator rewrite  
**Related documents:** [ArchitectureSchema.md](ArchitectureSchema.md), [PatternSchema.md](PatternSchema.md), [ChannelSchema.md](ChannelSchema.md)

---
Noted exceptions can be found in `Addendum.md`.
---

## 1. Repository strategy

Create the rewrite as a new repository or a completely independent source root.

Recommended location:

```text
~/projects/Toniator2/
```

The existing Toniator repository remains a legacy reference for:

- Algorithms.
- Fixtures.
- Presets.
- UI behavior.
- Screenshots.
- Export expectations.
- Regression evidence.

Legacy source must not be copied wholesale. Every reused component must be isolated, tested, and adapted to the new interfaces.

---

## 2. Workspace layout

```text
Toniator2/
├── Cargo.toml
├── Cargo.lock
├── rust-toolchain.toml
├── README.md
├── AGENTS.md
├── QWEN.md
├── LICENSE
│
├── crates/
│   ├── toniator-domain/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── document.rs
│   │       ├── canvas.rs
│   │       ├── source.rs
│   │       ├── output.rs
│   │       ├── channel.rs
│   │       ├── pattern_definition.rs
│   │       ├── commands.rs
│   │       ├── undo.rs
│   │       ├── validation.rs
│   │       └── ids.rs
│   │
│   ├── toniator-geometry/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── primitives/
│   │       │   ├── point.rs
│   │       │   ├── vector.rs
│   │       │   ├── rect.rs
│   │       │   ├── transform.rs
│   │       │   ├── bounds.rs
│   │       │   └── angle.rs
│   │       ├── curves/
│   │       │   ├── bezier.rs
│   │       │   ├── polyline.rs
│   │       │   ├── arc_length.rs
│   │       │   ├── tangent.rs
│   │       │   ├── normal.rs
│   │       │   └── intersections.rs
│   │       ├── guides/
│   │       │   ├── prototype.rs
│   │       │   ├── repetition.rs
│   │       │   ├── transform_stack.rs
│   │       │   ├── tile.rs
│   │       │   ├── normal_offset.rs
│   │       │   └── coverage.rs
│   │       ├── sites/
│   │       │   ├── site.rs
│   │       │   ├── provenance.rs
│   │       │   ├── jitter.rs
│   │       │   └── spatial_index.rs
│   │       ├── topology/
│   │       │   ├── graph.rs
│   │       │   ├── arrangement.rs
│   │       │   ├── faces.rs
│   │       │   ├── winding.rs
│   │       │   └── cleanup.rs
│   │       ├── regions/
│   │       │   ├── region.rs
│   │       │   ├── clipping.rs
│   │       │   ├── booleans.rs
│   │       │   ├── offset.rs
│   │       │   ├── crossings.rs
│   │       │   └── collapse.rs
│   │       ├── voronoi/
│   │       │   ├── mod.rs
│   │       │   ├── construct.rs
│   │       │   ├── guard_sites.rs
│   │       │   └── clip_cells.rs
│   │       └── canonical/
│   │           ├── marks.rs
│   │           ├── paths.rs
│   │           ├── regions.rs
│   │           └── geometry_output.rs
│   │
│   ├── toniator-sampling/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── fields.rs
│   │       ├── source_decode.rs
│   │       ├── interpolation.rs
│   │       ├── point_sample.rs
│   │       ├── path_sample.rs
│   │       ├── region_statistics.rs
│   │       ├── response_curve.rs
│   │       ├── polarity.rs
│   │       └── weighted_distribution.rs
│   │
│   ├── toniator-patterns/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── schema/
│   │       │   ├── mod.rs
│   │       │   ├── family.rs
│   │       │   ├── grid.rs
│   │       │   ├── random.rs
│   │       │   ├── output.rs
│   │       │   ├── modulation.rs
│   │       │   └── coverage.rs
│   │       ├── registry/
│   │       │   ├── mod.rs
│   │       │   ├── descriptor.rs
│   │       │   └── presets.rs
│   │       ├── family/
│   │       │   ├── mod.rs
│   │       │   ├── evaluate.rs
│   │       │   ├── grid_family.rs
│   │       │   ├── random_family.rs
│   │       │   ├── density.rs
│   │       │   └── family_output.rs
│   │       ├── realization/
│   │       │   ├── mod.rs
│   │       │   ├── marks.rs
│   │       │   ├── connected.rs
│   │       │   ├── network.rs
│   │       │   ├── maze.rs
│   │       │   ├── guide_faces.rs
│   │       │   └── voronoi.rs
│   │       ├── modulation/
│   │       │   ├── mod.rs
│   │       │   ├── marks.rs
│   │       │   ├── paths.rs
│   │       │   └── regions.rs
│   │       ├── validation.rs
│   │       └── evaluator.rs
│   │
│   ├── toniator-render/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── scene.rs
│   │       ├── layer.rs
│   │       ├── preview/
│   │       │   ├── mod.rs
│   │       │   └── renderer.rs
│   │       ├── raster/
│   │       │   ├── mod.rs
│   │       │   ├── compositor.rs
│   │       │   └── png.rs
│   │       ├── svg/
│   │       │   ├── mod.rs
│   │       │   ├── document.rs
│   │       │   ├── marks.rs
│   │       │   ├── paths.rs
│   │       │   ├── regions.rs
│   │       │   └── clipping.rs
│   │       └── debug/
│   │           ├── mod.rs
│   │           ├── guides.rs
│   │           ├── sites.rs
│   │           └── boundaries.rs
│   │
│   ├── toniator-io/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── document_format.rs
│   │       ├── preset_format.rs
│   │       ├── migrations/
│   │       │   ├── mod.rs
│   │       │   └── current.rs
│   │       ├── recovery.rs
│   │       ├── recent.rs
│   │       ├── source_files.rs
│   │       └── export.rs
│   │
│   └── toniator-app/
│       ├── Cargo.toml
│       ├── build.rs
│       ├── resources/
│       │   ├── toniator.gresource.xml
│       │   ├── blueprint/
│       │   │   ├── main_window.blp
│       │   │   ├── inspector.blp
│       │   │   ├── channel_editor.blp
│       │   │   ├── pattern_editor.blp
│       │   │   └── dialogs/
│       │   ├── icons/
│       │   └── styles/
│       └── src/
│           ├── main.rs
│           ├── application.rs
│           ├── main_window.rs
│           ├── actions.rs
│           ├── controllers/
│           │   ├── document_controller.rs
│           │   ├── evaluation_controller.rs
│           │   ├── export_controller.rs
│           │   └── selection_controller.rs
│           ├── view_models/
│           │   ├── document_view_model.rs
│           │   ├── channel_view_model.rs
│           │   └── pattern_view_model.rs
│           ├── widgets/
│           │   ├── canvas.rs
│           │   ├── inspector.rs
│           │   ├── channel_editor.rs
│           │   ├── pattern_editor.rs
│           │   └── guide_editor.rs
│           ├── binding/
│           │   ├── command_binding.rs
│           │   ├── schema_controls.rs
│           │   └── validation_display.rs
│           └── tasks/
│               ├── evaluation_task.rs
│               └── cancellation.rs
│
├── docs/
│   ├── ArchitectureSchema.md
│   ├── PatternSchema.md
│   ├── ChannelSchema.md
│   ├── ModuleStructure.md
│   ├── GEOMETRY_PIPELINE.md
│   ├── UI_ARCHITECTURE.md
│   ├── TESTING.md
│   └── adr/
│       ├── 0001-greenfield-rewrite.md
│       ├── 0002-single-document-authority.md
│       ├── 0003-pattern-family-realization-split.md
│       ├── 0004-canonical-render-scene.md
│       ├── 0005-floating-point-authored-values.md
│       └── 0006-no-runtime-plugin-system.md
│
├── presets/
│   ├── rectangular-dots.toml
│   ├── triangular-dots.toml
│   ├── random-stipple.toml
│   ├── curved-lines.toml
│   ├── maze.toml
│   └── voronoi.toml
│
├── fixtures/
│   ├── documents/
│   ├── sources/
│   ├── canonical/
│   ├── svg/
│   └── png/
│
├── tests/
│   ├── architecture/
│   ├── integration/
│   ├── golden/
│   └── ui/
│
├── scripts/
│   ├── validate_architecture.sh
│   ├── update_golden_fixtures.sh
│   └── check_blueprints.sh
│
└── .github/
    └── workflows/
        ├── ci.yml
        ├── architecture.yml
        └── fixtures.yml
```

---

## 3. Crate responsibilities

### 3.1 `toniator-domain`

Owns authoritative persisted concepts:

- Document.
- Canvas.
- Source references and interpretation settings.
- Output settings.
- Channel state.
- Pattern-definition references.
- Commands.
- Undo and redo.
- Validation interfaces.
- Stable IDs.

Must not contain:

- GTK.
- Cairo.
- SVG serialization.
- Pattern geometry.
- Random distribution algorithms.
- Voronoi.
- File dialogs.

### 3.2 `toniator-geometry`

Owns reusable mathematics and canonical geometry:

- Points, vectors, transforms, and bounds.
- Curves and arc-length operations.
- Guide repetition.
- Coverage planning primitives.
- Intersections.
- Sites and provenance.
- Graphs and topology.
- Regions and winding.
- Clipping and boolean operations.
- Region offset and crossing dissolution.
- Voronoi construction.
- Canonical marks, paths, and regions.

Must not contain:

- GTK.
- Document persistence.
- Channel color UI.
- Preset selection.
- Export dialogs.

### 3.3 `toniator-sampling`

Owns source-derived numerical fields:

- Image decode adapters.
- Point and interpolated sampling.
- Path sampling.
- Region statistics.
- Response curves.
- Polarity.
- Artwork-density weighting.

Must not contain:

- GTK.
- Pattern registry.
- Voronoi.
- SVG output.
- Document commands.

### 3.4 `toniator-patterns`

Owns pattern composition and evaluation:

- Pattern schema.
- Registry.
- Presets.
- Grid family.
- Random family.
- Continuous density interpretation.
- Site generation.
- Marks.
- Connected paths and networks.
- Mazes.
- Guide-derived regions.
- Voronoi realization as a thin site-to-cell adapter.
- Modulation coordination.
- Coverage coordination.
- Pattern validation.

Must not contain:

- GTK.
- File dialogs.
- SVG XML.
- PNG encoding.
- Writable document state.

### 3.5 `toniator-render`

Owns render-scene consumption:

- Render scene and layers.
- Preview rendering.
- Raster compositing and PNG.
- SVG writing.
- Debug overlays.

Must not:

- Generate sites.
- Read GTK widgets.
- Change pattern definitions.
- Reinterpret density.
- Own authoritative channel state.

### 3.6 `toniator-io`

Owns files and schema migration:

- Document format.
- Preset format.
- Recovery.
- Recent documents.
- Source file references.
- Export coordination.
- Version migration.

Must not:

- Implement pattern geometry.
- Own renderer math.
- Bind UI widgets.

### 3.7 `toniator-app`

Owns GTK/libadwaita:

- Application lifecycle.
- Window and actions.
- Controllers.
- View models.
- Canvas widget.
- Inspector.
- Channel editor.
- Pattern editor.
- Specialized guide editor.
- Blueprint/GResource.
- Background task coordination.
- Validation display.

Must not implement:

- Curve intersections.
- Density-weighted site generation.
- Voronoi.
- Region offset.
- SVG semantics.
- Document serialization rules.

---

## 4. Dependency matrix

| Crate | May depend on |
|---|---|
| `toniator-domain` | Standard library, serialization primitives |
| `toniator-geometry` | `toniator-domain` only where shared domain types are unavoidable |
| `toniator-sampling` | `toniator-domain`, low-level image/math libraries |
| `toniator-patterns` | `toniator-domain`, `toniator-geometry`, `toniator-sampling` |
| `toniator-render` | `toniator-domain`, `toniator-geometry` |
| `toniator-io` | `toniator-domain`, `toniator-render`, serialization/file libraries |
| `toniator-app` | All project crates, GTK/libadwaita |

Preferred refinement:

- Move primitive shared numeric types into `toniator-geometry` or a very small `toniator-core` only if cyclic pressure appears.
- Do not create a broad “common” crate as a dumping ground.

---

## 5. Public API boundaries

### 5.1 Domain to app

The app may:

- Read document snapshots.
- Dispatch commands.
- Subscribe to revision changes.
- Request evaluations.
- Present validation errors.

The app may not mutate internal fields directly.

### 5.2 Patterns to render

Patterns return canonical geometry only.

```rust
pub fn evaluate_channel(
    context: &EvaluationContext,
) -> Result<GeometryOutput, PatternError>;
```

### 5.3 Render to app

Render returns:

- Preview surfaces.
- Export bytes or file results.
- Render diagnostics.

Render does not return writable pattern state.

### 5.4 IO to app

IO returns:

- Loaded authoritative documents.
- Migration diagnostics.
- Save/export results.

---

## 6. Internal module boundaries

### 6.1 Pattern schema versus evaluation

Keep these separate:

```text
schema/
    Serialized structural types

family/
    Guide and site generation

realization/
    Marks, connected output, and regions

modulation/
    Artwork-to-geometry response

evaluator.rs
    Pipeline orchestration
```

A schema module must not perform rendering.

### 6.2 Voronoi placement

`toniator-patterns/src/realization/voronoi.rs` must remain thin:

```text
Read FamilyOutput.sites
→ call toniator-geometry Voronoi constructor
→ clip/filter cells
→ return canonical regions
```

It must not contain:

- Random generation.
- Density weighting.
- Grid generation.
- Seed handling.
- Guide intersections.

### 6.3 Region offset

Region offset belongs in:

```text
toniator-geometry/src/regions/offset.rs
toniator-geometry/src/regions/crossings.rs
toniator-geometry/src/regions/collapse.rs
```

Pattern modules only configure and invoke it.

### 6.4 Coverage

Coverage responsibility is divided:

- `toniator-patterns`: knows family semantics and requests coverage.
- `toniator-geometry`: supplies transform, bounds, projection, envelope, and guide coverage algorithms.
- `toniator-app`: never calculates coverage.

---

## 7. Architecture enforcement

### 7.1 Cargo dependency enforcement

Keep dependencies explicit in each crate.

Do not allow:

```text
toniator-domain → toniator-app
toniator-patterns → toniator-app
toniator-geometry → GTK
toniator-render → toniator-patterns internals
```

### 7.2 Source checks

`scripts/validate_architecture.sh` should fail CI when prohibited imports appear.

Examples:

```text
gtk or libadwaita imported outside toniator-app
svg writer imported inside toniator-patterns
document mutation APIs called inside toniator-render
```

### 7.3 Review checklist

Every pull request must state:

- Crates changed.
- Dependency changes.
- Authoritative state affected.
- Invalidation class.
- Tests added.
- Whether canonical geometry changed.
- Whether document schema changed.
- Whether migration is required.

---

## 8. Testing layout

### `tests/architecture`

- Dependency-direction checks.
- Prohibited-import checks.
- Schema round trips.
- No GTK in headless crates.

### `tests/integration`

- Full channel evaluation.
- Multi-channel documents.
- Shared pattern definitions.
- Undo and redo.
- Save and load.
- Background evaluation revision handling.

### `tests/golden`

- Canonical geometry fixtures.
- SVG fixtures.
- PNG fixtures.
- Edge-coverage fixtures at multiple rotations and aspect ratios.

### `tests/ui`

- Blueprint realization.
- Focus.
- Accessibility.
- Command binding.
- Channel editor.
- Pattern editor.
- Error display.

---

## 9. First vertical slice ownership

The first vertical slice should touch only:

```text
toniator-domain
toniator-geometry
toniator-patterns
toniator-render
toniator-app
fixtures
tests
```

Target behavior:

```text
900 × 600 canvas
→ density 90.0 × 60.0
→ two straight guide dimensions
→ arbitrary rotation and X/Y offset
→ analytical coverage with no edge gaps
→ intersections
→ circular marks
→ per-channel size response
→ color and opacity
→ preview, PNG, and SVG
```

Explicit non-goals:

- Curved guides.
- Random sites.
- Maze.
- Voronoi.
- Region offset.
- Preset editor.
- Plugin system.
- Legacy file import.

---

## 10. Legacy-code import procedure

Every imported legacy algorithm requires:

1. A written responsibility statement.
2. Identification of hidden dependencies.
3. Characterization tests against legacy behavior.
4. Conversion to the new input/output types.
5. Removal of GTK, global state, file access, and renderer assumptions.
6. Unit tests in the receiving crate.
7. A focused architecture review.
8. No unrelated source copied with it.

The old module structure is never imported as authority.

---

## 11. Recommended initial files

Before implementation, create:

```text
docs/ArchitectureSchema.md
docs/PatternSchema.md
docs/ChannelSchema.md
docs/ModuleStructure.md
docs/adr/0001-greenfield-rewrite.md
docs/adr/0002-single-document-authority.md
docs/adr/0003-pattern-family-realization-split.md
docs/adr/0004-canonical-render-scene.md
docs/adr/0005-floating-point-authored-values.md
```

Then create only the crate shells and compile the empty workspace before beginning the first vertical slice.

---

## 12. Completion criteria for the foundation

The repository foundation is complete when:

- Every crate compiles.
- Dependency rules are enforced in CI.
- A document with channels and pattern-definition references round-trips.
- Commands and undo/redo operate without GTK.
- Hard-coded canonical geometry renders identically through preview, PNG, and SVG.
- The first grid vertical slice passes edge-coverage fixtures.
- No lower-level crate imports GTK.
- No renderer generates pattern structure.
- No pattern evaluator mutates document state.
