# Adaptive Shape Library Marks

**Proposal for artist review.** Use one square cell per mark. Its average channel ink selects a footprint, and the ink’s position inside the cell determines which way the mark faces.

## Footprints and decisions

The [artist reference](../target/validation/adaptive-shape-marks-planning/artist-reference.png) supplies five silhouettes. Their apparent width is about half the blue guide pitch. Reproducing that spacing literally would limit a solid square to roughly 25% coverage. I recommend normalizing the silhouettes to the full cell, subject to artist approval.

Use unit coordinates with x rightward and y downward. Every cell has side one. The sector, triangle and segment share the bottom-left corner as their construction anchor.

| Mark | Base footprint in the unit cell | Area fraction | Distinct poses |
| --- | --- | --- | --- |
| Empty | No geometry | 0 | 1 |
| Circular segment | Sector minus the triangle, between its arc and chord y=x | π/4 − 1/2 ≈ 0.2854 | 4 |
| Right isosceles triangle | Vertices (0,0), (0,1), (1,1), below y=x | 1/2 | 4 |
| Quarter-circle sector | Center (0,1), radius 1, bounded by the left and bottom sides | π/4 ≈ 0.7854 | 4 |
| Full circle | Center (1/2,1/2), radius 1/2 | π/4 ≈ 0.7854 | 1 |
| Square | Entire cell | 1 | 1 |

Generate quarter turns and reflected variants, then remove geometrically identical poses. Reflection introduces no additional silhouettes for these shapes.

1. **Choose an area.** Average the required channel ink over the full square. Choose the nearest distinct area fraction in the table. At an exact midpoint, take the lower level.
2. **Fit the ink.** At that level, choose the outline and pose that cover the most requested ink. At π/4, disk-shaped ink favors the centered circle and ink shaped like a corner sector favors that sector. Equal scores favor the circle when eligible, then the reference pose and successive clockwise quarter turns about the cell center. Use a fixed score precision and comparison order so parallel scheduling yields the same result.

For a half-ink cell whose ink lies below y=x, the area decision selects 1/2 and the triangle fills that region exactly. When ink lies above y=x, the area decision remains 1/2 and the triangle turns 180°. Both obtain overlap 1/2. The segment is the curved remainder outside this triangle, never the triangle on the other side of the diagonal.

## Tone, sampling and placement

Recommend independent nearest-area selection for version one. Mapped, eligible zero produces empty geometry and one produces a square. Below the first midpoint, approximately 14.27%, nearest-area selection remains empty. A uniform 60% cell selects the 50% triangle, so flat tones can show steps. At uniform π/4 the circle wins an overlap tie. Artist review should judge these steps.

An optional later residual mode could distribute a 60% flat target between 50% and π/4. Approximately 35% of cells would use π/4 and 65% would use triangles. Carry requested minus selected area in stable artwork-space row order. Preserve exact transparent/no-ink and full endpoints. This improves average tone but can weaken local edges and creates dependencies and possible directional texture. It requires a global artwork traversal, including the same preceding cells for every tile. Independent cells can run in parallel immediately. Residual rows cannot be evaluated independently without an additional partition contract.

At each sample, use the channel’s existing Fill mapping and cutoff, then apply the Advanced desired-coverage endpoints to the surviving response. Keep the current alpha rules. Color-derived response incorporates alpha once. Explicit Alpha mapping has its own polarity, so an inverted Alpha channel can intentionally request ink from transparent pixels. For **Source color + alpha**, use alpha to choose the geometry. Average associated linear RGB and alpha over the chosen footprint, then unassociate once to obtain the mark color. A footprint with zero total alpha deposits no paint. Test ink in a corner with a transparent cell center.

Use bounded, symmetric subcell quadrature for both required ink and overlap. Include fractional weights for samples intersecting curved boundaries, and establish an accuracy bound. Scoring must never depend on screen pixels, zoom or tile presence. Per-cell work can run concurrently, with stable cell-order geometry publication.

Use the native centered artwork-local square grid, following `grid_prototype_local_to_document_transform`. One Feature size supplies equal pitches on both axes. Guides uses two perpendicular straight directions. Aspect, Uniform/shape scale and single-shape rotation are inapplicable and hidden for this strategy. Preserve stable signed lattice IDs through translation and rotation. Enumerate full guard cells, score complete cells using the existing edge-clamped source sampler, finish their geometry and only then clip to the canvas. Measure visible fragments separately when reporting edge coverage.

## Smallest product integration

Represent this as an explicit compatible square-grid mark strategy in domain authority. The Shape Library entry contains fixed silhouettes and pose policy, with a distinct right-triangle template because the existing Triangle is equilateral. Wizard should offer one gallery recipe and a Drawing → Shapes selection, with eligibility derived from the same descriptor authority. Layer and All application must retain normal assignment behavior. Save/reopen must preserve strategy intent. Changing it invalidates geometry and calibration. Preserve the common unit cell instead of ordinary per-outline bounds/circumradius fitting.

Calibration should measure the actual discrete response, including its plateaus. It must not interpolate an inverse through unattainable tones. Residual mode needs its own measured response and identity.

Emit canonical circles, closed line paths and compact cubic paths, with explicit quarter-arc approximation tolerance against analytic area buckets. Shared canonical geometry serves Accurate preview, PNG and SVG, retaining existing Fast fallback for mixed geometry. The existing SVG writer groups eligible opaque runs into editable compound subpaths and applies final clipping. Validate actual nodes and native clipping before changing that route. Editable silhouettes do not imply one selectable SVG object per cell.

Engineering approval must settle the discrete cell-scaled response contract against protected [Addendum §17.5 and §16](../Project%20Specification/Addendum.md), which govern ordinary Variable raw C(size) inversion and Source color + alpha. [Discrete hatching](../crates/toniator-engine/src/reference_calibration.rs) is a precedent, not a reusable fit. Structural Apply must atomically publish the typed binding and genuine new measurement. Preview/export never fit. If formats change, deterministically convert supported source-free Document Presets while preserving original bytes.

## Qualification after approval

Check uncropped sector/segment short-arc commands, flat levels around every area threshold, both diagonal half-ink examples, horizontal and vertical edges, curved edges and a smooth ramp. Measure per-channel selected area, visible clipped coverage, pose choice, transparency and deterministic output across thread counts and tiled previews. Include portrait and landscape canvases with odd dimensions and marks crossing every edge and corner.

Exercise immutable `assets/raster-sample.png` (1024×1024 RGBA) and `assets/vector-sample.svg` (900×620, including font-dependent live text), preserving native RGBA PNG and SVG outputs plus an independent SVG rasterization. Inspect color and alpha separately. A solid-square witness must cover all four corners without transparent seams. Require at most four vertices per uncut polygon and four cubic spans per circle, plus documented final-clipping overhead, and inspect actual exported editability. Compare cold and warm preview times against fixed square marks at matched pitch and coverage. Later UI verification uses the private GTK harness for eligibility, layer/All, keyboard actions and save/reopen, with screenshots and logs. Artist acceptance and GNOME/Mutter acceptance remain separate.

Source references are [grid authority](../crates/toniator-patterns/src/lib.rs), [sampling](../crates/toniator-sampling/src/lib.rs), [domain descriptors](../crates/toniator-domain/src/lib.rs), [calibration](../crates/toniator-domain/src/calibration.rs), [Shape Library](../crates/toniator-app/src/shape_library.rs), [outlines](../crates/toniator-geometry/src/outlines.rs), [SVG joining](../crates/toniator-render/src/svg_join.rs), [SVG clipping](../crates/toniator-render/src/svg_clip.rs), and [artwork guidance](../assets/README.md).

Recommend full-cell footprints and independent nearest-area selection first. Artist review must settle footprint spacing, circle-first ties on uniform tones, and whether discrete tone steps justify a later residual mode. Stop here pending that review.
