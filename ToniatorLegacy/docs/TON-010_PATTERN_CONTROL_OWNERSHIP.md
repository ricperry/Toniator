# TON-010 pattern-control ownership

Gate 1 record, 2026-08-02. This table records the current surface; it does not
introduce the Gate 2 parameter schema or change a recipe's canonical output.

`Document.pattern_state` is the semantic authority for the selected pattern,
its compatibility settings, and embedded instances. A pattern definition owns
reusable construction, topology, randomness policy, and structural
orientation. A channel instance owns enabled/color/opacity, sampling,
coverage-density response, source weighting, seed values, mark primitive, mark
scale, mark or treatment rotation, and selection visibility.
`Document.artwork_pipeline` owns source mapping; `DocumentAppearance` owns
presentation only. Current compatibility fields are projections into the
selected instance, never an additional semantic authority.

## Pattern selection and structural authoring

| Current UI location and control | Current storage authority | Intended owner | Class | Compatibility projection(s) | Gate 1 disposition |
| --- | --- | --- | --- | --- | --- |
| Pattern Settings: Shapes, Curves, Weighted Voronoi selector buttons and Pattern preset selector | `PatternDocumentState.selected` | Pattern definition selection | Structural | `RenderVariant`, `PATTERN_REGISTRY`, bundled/embedded definition dispatch | Retain; selection is already semantic authority. |
| Pattern Settings: hidden `dots`, `squares`, `lines`, `legacy` buttons | Legacy `Settings.treatment`/derived `RenderVariant` compatibility UI state | Compatibility projection, not a definition or channel owner | Hidden retained compatibility | Native Basic/legacy Shapes and Curves adapters | Retain hidden until Gate 6 parity permits removal; do not expose as a second selector. |
| Pattern Settings: Import Preset and Save Preset | Current `.tntr` document/preset serialization | Document/preset workflow, not pattern parameters | Workflow action | Preset adapters serialize `pattern_state` and treatment sections | Retain; no ownership move. |
| Pattern Settings: Edit Pattern | Local `PatternEditorDraft`, then one `DocumentEditor` edit | Pattern definition | Structural draft | `pattern_editor_recipe` currently mutates a Shapes-compatible definition | Retain as a bounded structural editor; Gate 2 replaces its hard-coded parameter surface. |
| Custom Pattern panel: summary and Edit custom pattern | Selected `embedded_patterns` entry in `PatternDocumentState` | Pattern definition | Structural | Embedded definition/instance runtime dispatch | Retain; summary is presentation, edit remains structural. |
| Pattern Editor: Name | `PatternEditorDraft.name`, then embedded definition display name | Pattern definition | Structural metadata | `pattern_editor_recipe` changes definition ID/display metadata | Retain. |
| Pattern Editor: Placement, Density, Grid Scale | `PatternEditorDraft`, then definition/instance structural values | Pattern definition | Structural | Shapes lattice operation and custom embedded instance | Retain. |
| Pattern Editor: X/Y Grid, X/Y Grid Curve, Curve Function | `PatternEditorDraft`, then definition parameters | Pattern definition | Structural orientation/policy | `shapes.lattice-placement-editor` compatibility operation | Retain as structural draft fields; replace hard-coded operation binding only in later gates. |
| Pattern Editor: X/Y Spacing, Curve Spacing | `PatternEditorDraft`, then definition parameters | Pattern definition | Structural spacing | Shapes editor lattice projection | Retain. |
| Pattern Editor: Random Dispersion and Jitter | `PatternEditorDraft`, then definition parameters | Pattern definition | Structural stochastic policy | Shapes editor lattice projection | Retain. Jitter algorithm is structural; seed is not. |
| Pattern Editor: Point Definition, Render Geometry, Connection Mode | `PatternEditorDraft`, then definition/output graph | Pattern definition | Structural topology/output | Shapes mark/network compatibility operation and emitter | Retain. |
| Pattern Editor: Curve canvas and Reset | `PatternEditorDraft.curve_path` until Apply | Pattern definition | Structural path topology | Curve-path parameter and Shapes editor lattice projection | Retain; Cancel discards the local draft. |
| Pattern Editor preview and sensitivity/disabled states | GTK-local presentation only | Presentation only | Presentation | No persisted projection | Retain; never serialize or treat as a pattern value. |

## Channel treatment, marks, source response, and randomness

| Current UI location and control | Current storage authority | Intended owner | Class | Compatibility projection(s) | Gate 1 disposition |
| --- | --- | --- | --- | --- | --- |
| Channel Settings: scope/active channel target and visible-channel checkboxes | Selected output model plus `WebShapeChannel.enabled` / `WebCurveChannel.enabled` | Channel instance | Channel-specific treatment | Shapes/Curves channel instance values and semantic-channel filters | Retain. Scope is a UI target, not persisted pattern data. |
| Shapes: Share Mark Across Inks/Channels | `WebShapeSettings.use_shared_mark` and shared primitive convenience fields | Channel instance | Channel-specific treatment policy | Shapes compatibility definition and embedded output-channel values | Retain as one channel-treatment authority. The shared value is a convenience projection, not definition-owned mark selection. |
| Shapes: Mark Shape, Polygon Sides, Edit Custom Shape | Shared or per-channel `WebShapeSettings` shape/path fields | Channel instance | Channel-specific mark primitive | `adapt_shapes_settings_to_recipe` and embedded output-channel values | Retain current shared convenience storage for parity. Gate 2 must expose one per-ink mark authority without duplicating primitive/polygon/custom-mark values. Definitions declare supported emitted geometry only. |
| Shapes: Adjust Channel, Channel HEX, Visible Inks | `WebShapeChannel.color` and enabled state | Channel instance | Channel-specific treatment | Shapes output-channel values | Retain. |
| Shapes: Mark Size, Horizontal/Vertical Mark Scale, Light-Tone Cutoff, Ink Opacity | `WebShapeChannel.scale`, width/height scale, threshold, opacity | Channel instance | Channel-specific treatment | Shapes output-channel values | Retain. |
| Shapes: Rotate Ink Screen | Current `WebShapeChannel.grid_rotation` | Pattern definition (structural orientation) | Structural orientation currently stored per channel | Shapes output-channel `grid-rotation`; legacy renderer | Retain current storage for parity. Gate 2 must move/expose this as definition-owned without adding a duplicate channel authority. |
| Shapes: Rotate Marks | `WebShapeChannel.rotation` | Channel instance | Channel-specific treatment | Shapes output-channel `rotation` | Retain. |
| Shapes: Sampling Detail | `WebShapeChannel.resolution_scale` | Channel instance | Source-response/treatment | Shapes output-channel `resolution-scale` | Retain. |
| Shapes: Site sampler (Grid/Uniform/Weighted) | One current `WebShapeChannel.point_sampler` compatibility value | One current compatibility authority; Gate 2 splits definition-owned distribution algorithm/policy from channel-owned source weighting/response | Conflated compatibility value | Embedded `point-sampler`; neutral site-distribution request | Retain exactly one current value. Gate 2 must decompose its semantics without creating concurrent definition and channel authorities. |
| Shapes: Random seed (per channel) and Unified random seed | `WebShapeChannel.random_seed` | Channel instance | Channel-specific stochastic value | Embedded `channel-seed`; neutral distribution seed | Retain. Seeds are never Pattern Editor draft data. |
| Shapes: Weight Influence | `WebShapeChannel.weight_influence` | Channel instance | Source response | Embedded `channel-weight-influence`; weighted distribution request | Retain. |
| Shapes: Uniform — Source responsive Shape Size Response | `WebShapeChannel.random_size_response` | Channel instance | Source response | Embedded `random-size-response`; Shapes native mapping | Rename retained widget and Rust field to `channel_random_size_response`; remove it from `PatternEditorDraft`. |
| Shapes: grid pivot and offsets (retained where exposed by legacy controls) | Current `WebShapeChannel.grid_pivot_*`, `offset_*` compatibility projection | Pattern definition | Structural placement | Shapes output-channel values | Retain current projection for parity. Gate 2 moves/exposes this structural value once; it is not independently channel-authoritative. |
| Curves: Layout and Shared Curve | `WebCurveSettings.layout`, `use_shared_curve`, shared path fields | Pattern definition | Structural topology | `adapt_curves_settings_to_recipe` and Curves definition | Retain. |
| Curves: curve canvas, Reset, Close Ends, Smooth Join | Shared or per-channel `CurvePath` and close/join compatibility fields | Pattern definition | Authored path topology | Curves motif/deformation instance values | Retain the current projection for parity. An authored mathematical/emitted path stays definition-owned even when compatibility storage selects a per-channel path. |
| Curves: Curve Spacing, Motif Size, Columns, Rows, Row Spacing, Stagger, Alternate Transform | `WebCurveSettings` / `WebCurveChannel` layout and motif compatibility fields | Pattern definition | Structural topology/spacing | Curves compatibility recipe values | Retain current projection for parity; Gate 2 normalizes the structural value once. |
| Curves: Rotate Ink Screen, Position X/Y | `WebCurveChannel.grid_rotation`, offsets | Pattern definition structural orientation/placement | Structural orientation currently stored per channel | Curves output-channel values | Retain current storage until schema work. |
| Curves: Curve Width, Curve Coverage, Ink Opacity, Light-Tone Cutoff, Sampling Detail, Output Quality | `WebCurveChannel` max-mark/scale, opacity, threshold, resolution, quality | Channel instance | Channel-specific treatment/source response | Curves output-channel values | Retain. Current Curves has no separate mark-only rotation; structural screen rotation remains definition-owned. |
| Curves: Channel HEX and Visible Inks | `WebCurveChannel.color`, enabled | Channel instance | Channel-specific treatment | Curves emitter values | Retain. |
| Curves: Motif Coverage and Bleed | `WebCurveChannel` motif treatment fields | Channel instance | Channel-specific treatment | Curves deformation values | Retain. |
| Weighted Voronoi: active Channel and visible-channel checkboxes | `WeightedVoronoiSettings.channels[*]` enabled semantic output | Channel instance | Channel-specific output selection | Weighted runtime channel projection | Retain. |
| Weighted Voronoi: Cell Count, Arrangement, Placement policy | Current `WeightedVoronoiChannelSettings` | Pattern definition | Structural count/spacing and distribution algorithm/policy | `site_distribution` request through Weighted adapter | Retain current per-channel compatibility storage; Gate 2 moves/exposes one definition authority without changing stable placement/tessellation. |
| Weighted Voronoi: Boundary Gap and Minimum Cell Scale | Current `WeightedVoronoiChannelSettings` | Pattern definition | Structural geometry policy | Weighted adapter -> canonical positive regions | Retain current compatibility storage. No edits to `site_distribution.rs` or `voronoi_geometry.rs`. |
| Weighted Voronoi: Density Strength and Response Strength | Current `WeightedVoronoiChannelSettings` | Channel instance | Channel-specific density/coverage response and source weighting/influence | Weighted adapter -> resolved semantic channel output | Retain current compatibility storage as one channel treatment authority. |
| Weighted Voronoi: Seed | `WeightedVoronoiChannelSettings.seed` | Channel instance | Channel-specific stochastic value | `site_distribution` request | Retain. |

## Source mapping, basic compatibility, and presentation

| Current UI location and control | Current storage authority | Intended owner | Class | Compatibility projection(s) | Gate 1 disposition |
| --- | --- | --- | --- | --- | --- |
| Source: Artwork Source, Source Alpha | `Document.artwork_pipeline` | Artwork pipeline | Source mapping | Resolved channel fields consumed by all patterns | Retain; not recipe/draft data. |
| Output: Output Model, Channel Assignment, Active Channel | `Document.artwork_pipeline` and `Document.output_mode` | Artwork pipeline/document output | Source mapping/output routing | Legacy renderer pipeline projection | Retain; not per-pattern data. |
| Native Basic panel: Sampling Detail, Coverage, Contrast, Screen Angle | Legacy `Document.settings` / `PatternSelection::NativeBasicV1` compatibility | Retained compatibility pattern only | Hidden/compatibility structural and treatment controls | Native Basic renderer and `RenderVariant` | Retain without new controls; remove only after Gate 6 equivalence work. |
| Appearance: Preview Surface and color | `DocumentAppearance.preview_surface` | Presentation only | Presentation | Preview renderer only | Retain; never affects canonical output or export. |
| Appearance: Export Background and color | `DocumentAppearance.export_background` | Export presentation only | Presentation | PNG/SVG export presentation stage | Retain; never affects pattern definition or channel instance. |

## Enforcement at this boundary

`PatternEditorDraft` no longer has a `random_size_response` field. The
`channel_random_size_response` widget is located in Channel Settings and writes
only `WebShapeChannel.random_size_response`. The focused recipe test changes
structural draft controls and verifies that channel seed, source-response,
mark rotation, and source-weight influence remain channel treatment values from
`Document.pattern_state`. It also preserves `grid_rotation` as a retained
structural compatibility projection from that same authority, without calling
it channel-owned. This is a Gate 1 mechanical boundary, not a typed schema
implementation.
