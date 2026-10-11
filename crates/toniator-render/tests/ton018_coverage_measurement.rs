//! Focused geometric-union mask parity and resource-bound witnesses.

use toniator_domain::{
    CanvasSpec, ChannelId, ColorValue, GuideDimensionId, PathStrokeStyle, PatternMechanismId,
    PatternOutputLayerId,
};
use toniator_geometry::{
    CanonicalFillRule, CanonicalMark, CanonicalPathMark, CanonicalRegionProposal,
    CanonicalRegionSourceGroup, CanonicalRegionSourceId, CanonicalStroke, CurvePath, FamilySiteId,
    FamilySiteProvenance, PathClosure, PathLocation, Point2, SiteScope, StrokeProfileSample,
    StructuralPathInstanceId, VariableWidthOutlineLimits, VariableWidthPathSample,
    build_canonical_regions, build_variable_width_outline_cancellable,
};
use toniator_render::{
    GeometryOutput, OccupancyWindow, OutputRasterTarget, RasterAntialiasing, RasterBackground,
    RasterizationLimits, RenderLayer, RenderScene, rasterize_geometry_occupancy,
    rasterize_geometry_occupancy_window, rasterize_mark_multiplicity, rasterize_output,
};

/// Returns one stable site identity for a finite, renderer-owned fixture.
fn site(ordinal: usize) -> FamilySiteId {
    FamilySiteId {
        mechanism_id: PatternMechanismId(3),
        ordinal,
    }
}

/// Returns irrelevant but valid provenance without implying a pattern-specific mask table.
fn provenance(ordinal: usize) -> FamilySiteProvenance {
    FamilySiteProvenance::Random {
        candidate_ordinal: ordinal,
        accepted_ordinal: ordinal,
        exclusion_neighbor_ordinal: None,
    }
}

/// Builds a circle that crosses another mark and the raster edge in parity tests.
fn circle(ordinal: usize, x: f64, y: f64, radius: f64) -> CanonicalMark {
    CanonicalMark::Circle {
        source_site_id: site(ordinal),
        center: Point2::new(x, y),
        radius,
        scope: SiteScope::Canvas,
        provenance: provenance(ordinal),
        fill_rule: CanonicalFillRule::EvenOdd,
    }
}

/// Builds an authored closed diamond so even-odd path flattening is exercised.
///
/// # Panics
/// Panics only if the finite closed fixture fails geometry validation.
fn closed_mark() -> CanonicalMark {
    let path = CurvePath::polyline(
        vec![
            Point2::new(3.0, 1.0),
            Point2::new(11.0, 6.0),
            Point2::new(5.0, 13.0),
            Point2::new(-2.0, 7.0),
        ],
        PathClosure::Closed,
    )
    .expect("closed diamond");
    CanonicalMark::ClosedPath(
        CanonicalPathMark::new(
            site(9),
            path,
            SiteScope::Canvas,
            provenance(9),
            CanonicalFillRule::EvenOdd,
        )
        .expect("valid path mark"),
    )
}

/// Builds one direct filled outline stroke crossing the target boundary.
///
/// # Panics
/// Panics only if finite centerline, profile, or outline fixture construction fails.
fn stroke() -> CanonicalStroke {
    let path = CurvePath::line(Point2::new(-2.0, 6.0), Point2::new(18.0, 6.0)).expect("line");
    let profile = vec![
        StrokeProfileSample {
            location: PathLocation::new(0, 0.0).expect("start"),
            center: Point2::new(-2.0, 6.0),
            normalized_thickness: 1.0,
            width: 3.0,
        },
        StrokeProfileSample {
            location: PathLocation::new(0, 1.0).expect("end"),
            center: Point2::new(18.0, 6.0),
            normalized_thickness: 1.0,
            width: 3.0,
        },
    ];
    let outline = build_variable_width_outline_cancellable(
        &path,
        &[
            VariableWidthPathSample {
                location: profile[0].location,
                width: 3.0,
            },
            VariableWidthPathSample {
                location: profile[1].location,
                width: 3.0,
            },
        ],
        PathStrokeStyle::default(),
        0.0,
        1.0 / 8.0,
        VariableWidthOutlineLimits::new(64).expect("outline limit"),
        &|| false,
    )
    .expect("finite outline");
    CanonicalStroke::new(
        StructuralPathInstanceId::guide_dimension(GuideDimensionId(1), 0, 0),
        None,
        path,
        3.0,
        PathStrokeStyle::default(),
        profile,
        outline,
    )
    .expect("canonical stroke")
}

/// Builds a nonzero-filled canonical region whose boundary is clipped only by the consumer.
///
/// # Panics
/// Panics only if the closed finite region fixture is rejected.
fn region() -> GeometryOutput {
    let ring = CurvePath::polyline(
        vec![
            Point2::new(-1.0, -1.0),
            Point2::new(8.0, 0.0),
            Point2::new(13.0, 9.0),
            Point2::new(3.0, 14.0),
        ],
        PathClosure::Closed,
    )
    .expect("region ring");
    GeometryOutput::CanonicalRegions(
        build_canonical_regions(CanonicalRegionProposal {
            output_layer_id: PatternOutputLayerId(7),
            source_groups: vec![CanonicalRegionSourceGroup {
                source_id: CanonicalRegionSourceId::SiteOwners(vec![site(7)]),
                components: vec![ring],
            }],
        })
        .expect("canonical region"),
    )
}

/// Compares every binary mask bit with the alpha from the same geometry's opaque-white AA-Off output.
///
/// This is exact pixel parity at two noncommensurate targets, not an AA-On optical promise.
///
/// # Panics
/// Panics if scene construction or a validated geometry measurement fails.
fn assert_opaque_alpha_parity(geometry: GeometryOutput) {
    let canvas = CanvasSpec {
        width: 16.0,
        height: 14.0,
    };
    let white = ColorValue {
        red: 1.0,
        green: 1.0,
        blue: 1.0,
        alpha: 1.0,
    };
    let scene = RenderScene::new(
        canvas.clone(),
        "mask-family".into(),
        "mask-geometry".into(),
        vec![RenderLayer::new(ChannelId(1), true, white, 1.0, geometry.clone()).expect("layer")],
    )
    .expect("scene");
    for (width, height) in [(16, 14), (31, 23)] {
        let target = OutputRasterTarget::new(width, height).expect("bounded target");
        let mask = rasterize_geometry_occupancy(
            &geometry,
            &canvas,
            target,
            RasterizationLimits::default(),
            &|| false,
        )
        .expect("mask");
        let rgba = rasterize_output(
            &scene,
            RasterBackground::Transparent,
            Some(target),
            RasterAntialiasing::Off,
        )
        .expect("opaque white raster");
        assert_eq!((mask.width(), mask.height()), (width, height));
        assert!(mask.pixels().iter().all(|bit| *bit <= 1));
        let alpha = rgba
            .pixels()
            .chunks_exact(4)
            .map(|pixel| u8::from(pixel[3] > 0));
        assert!(mask.pixels().iter().copied().eq(alpha));
        assert_eq!(
            mask.covered_pixels(),
            mask.pixels().iter().filter(|&&bit| bit == 1).count()
        );
        assert_eq!(
            mask.fraction(),
            mask.covered_pixels() as f64 / f64::from(width * height)
        );
    }
}

/// Counts complete closed marks once per pixel while preserving distinct-mark overlap.
///
/// # Panics
/// Fails if the canonical even-odd path walker counts segments or loses union parity.
#[test]
fn uniform_mark_multiplicity_counts_canonical_footprints_once() {
    let canvas = CanvasSpec {
        width: 16.0,
        height: 14.0,
    };
    let target = OutputRasterTarget::new(31, 23).expect("bounded target");
    let single = GeometryOutput::CanonicalMarks(vec![closed_mark()]);
    let single_mask = rasterize_mark_multiplicity(
        &single,
        &canvas,
        target,
        RasterizationLimits::default(),
        &|| false,
    )
    .expect("closed mark counts");
    let union = rasterize_geometry_occupancy(
        &single,
        &canvas,
        target,
        RasterizationLimits::default(),
        &|| false,
    )
    .expect("closed mark union");
    assert!(single_mask.counts().iter().all(|count| *count <= 1));
    assert!(
        single_mask
            .counts()
            .iter()
            .copied()
            .map(u8::try_from)
            .map(Result::unwrap)
            .eq(union.pixels().iter().copied())
    );
    let overlapping = GeometryOutput::CanonicalMarks(vec![
        circle(1, 5.0, 6.0, 5.0),
        circle(2, 8.0, 6.0, 5.0),
        closed_mark(),
    ]);
    let multi = rasterize_mark_multiplicity(
        &overlapping,
        &canvas,
        target,
        RasterizationLimits::default(),
        &|| false,
    )
    .expect("overlapping marks count");
    let occupied = rasterize_geometry_occupancy(
        &overlapping,
        &canvas,
        target,
        RasterizationLimits::default(),
        &|| false,
    )
    .expect("overlapping union");
    assert!(multi.counts().iter().any(|count| *count >= 2));
    assert!(
        multi
            .counts()
            .iter()
            .map(|count| u8::from(*count > 0))
            .eq(occupied.pixels().iter().copied())
    );
}

/// Covers the shared circle, authored path, stroke-outline, and region walkers at both targets.
#[test]
fn occupancy_matches_opaque_off_alpha_for_all_current_geometry_types() {
    assert_opaque_alpha_parity(GeometryOutput::CanonicalMarks(vec![
        circle(0, 5.0, 5.0, 4.0),
        circle(1, 9.0, 5.0, 4.0),
    ]));
    assert_opaque_alpha_parity(GeometryOutput::CanonicalMarks(vec![closed_mark()]));
    assert_opaque_alpha_parity(GeometryOutput::CanonicalStrokes(vec![stroke()]));
    assert_opaque_alpha_parity(region());
}

/// Confirms an aligned document crop matches the full raster without selecting geometry first.
///
/// # Panics
/// Panics if a validated shared-walker crop changes any binary sample center.
fn assert_aligned_window_crop(geometry: GeometryOutput) {
    let canvas = CanvasSpec {
        width: 16.0,
        height: 14.0,
    };
    let full = rasterize_geometry_occupancy(
        &geometry,
        &canvas,
        OutputRasterTarget::new(64, 56).expect("full target"),
        RasterizationLimits::default(),
        &|| false,
    )
    .expect("full mask");
    for (x, y, width, height) in [(4_u32, 3_u32, 6_u32, 5_u32), (12, 10, 3, 3)] {
        let window = OccupancyWindow::new(
            f64::from(x),
            f64::from(y),
            f64::from(width),
            f64::from(height),
        )
        .expect("document crop");
        let crop = rasterize_geometry_occupancy_window(
            &geometry,
            &canvas,
            window,
            OutputRasterTarget::new(width * 4, height * 4).expect("crop target"),
            RasterizationLimits::default(),
            &|| false,
        )
        .expect("window mask");
        for row in 0..height * 4 {
            let full_start = ((y * 4 + row) * 64 + x * 4) as usize;
            let crop_start = (row * width * 4) as usize;
            assert_eq!(
                &crop.pixels()[crop_start..crop_start + (width * 4) as usize],
                &full.pixels()[full_start..full_start + (width * 4) as usize]
            );
        }
    }
}

/// Proves crossing and sparse primitives keep exact sample-center semantics across geometry types.
///
/// # Panics
/// Panics if bounded fixture construction or exact crop comparison fails.
#[test]
fn occupancy_window_is_exact_full_mask_crop_for_crossing_and_sparse_geometry() {
    assert_aligned_window_crop(GeometryOutput::CanonicalMarks(vec![
        circle(0, 3.0, 5.0, 4.0),
        circle(1, 15.0, 13.0, 1.0),
        circle(2, -10.0, -10.0, 1.0),
    ]));
    assert_aligned_window_crop(GeometryOutput::CanonicalMarks(vec![closed_mark()]));
    assert_aligned_window_crop(GeometryOutput::CanonicalStrokes(vec![stroke()]));
    assert_aligned_window_crop(region());
}

/// Confirms duplicate geometry cannot increase a Boolean union and appearance is outside the API.
///
/// # Panics
/// Panics if a small validated mask or appearance witness fails construction.
#[test]
fn duplicate_geometry_is_idempotent_and_paint_cannot_enter_mask() {
    let canvas = CanvasSpec {
        width: 16.0,
        height: 14.0,
    };
    let target = OutputRasterTarget::new(31, 23).expect("target");
    let one = GeometryOutput::CanonicalMarks(vec![circle(0, 6.0, 7.0, 4.0)]);
    let duplicate =
        GeometryOutput::CanonicalMarks(vec![circle(0, 6.0, 7.0, 4.0), circle(1, 6.0, 7.0, 4.0)]);
    let first = rasterize_geometry_occupancy(
        &one,
        &canvas,
        target,
        RasterizationLimits::default(),
        &|| false,
    )
    .expect("single mask");
    let second = rasterize_geometry_occupancy(
        &duplicate,
        &canvas,
        target,
        RasterizationLimits::default(),
        &|| false,
    )
    .expect("duplicate mask");
    assert_eq!(first, second);
    let red = RenderScene::new(
        canvas.clone(),
        "a".into(),
        "a".into(),
        vec![
            RenderLayer::new(
                ChannelId(1),
                true,
                ColorValue {
                    red: 1.0,
                    green: 0.0,
                    blue: 0.0,
                    alpha: 1.0,
                },
                0.25,
                one.clone(),
            )
            .expect("red layer"),
        ],
    )
    .expect("red scene");
    let blue = RenderScene::new(
        canvas,
        "b".into(),
        "b".into(),
        vec![
            RenderLayer::new(
                ChannelId(1),
                true,
                ColorValue {
                    red: 0.0,
                    green: 0.0,
                    blue: 1.0,
                    alpha: 1.0,
                },
                1.0,
                one,
            )
            .expect("blue layer"),
        ],
    )
    .expect("blue scene");
    assert_ne!(
        rasterize_output(
            &red,
            RasterBackground::Transparent,
            Some(target),
            RasterAntialiasing::Off
        )
        .expect("red raster"),
        rasterize_output(
            &blue,
            RasterBackground::Transparent,
            Some(target),
            RasterAntialiasing::Off
        )
        .expect("blue raster")
    );
}

/// Verifies cancellation, target safety, and request-wide flattened-edge bounds remain enforced.
///
/// # Panics
/// Panics only if a safe finite fixture unexpectedly bypasses a configured limit.
#[test]
fn occupancy_respects_cancellation_target_and_edge_limits() {
    let canvas = CanvasSpec {
        width: 16.0,
        height: 14.0,
    };
    let target = OutputRasterTarget::new(31, 23).expect("target");
    let geometry = GeometryOutput::CanonicalMarks(vec![closed_mark()]);
    let cancelled = rasterize_geometry_occupancy(
        &geometry,
        &canvas,
        target,
        RasterizationLimits::default(),
        &|| true,
    )
    .expect_err("cancelled request");
    assert_eq!(cancelled.path(), "evaluation.cancelled");
    let limited = rasterize_geometry_occupancy(
        &geometry,
        &canvas,
        target,
        RasterizationLimits::new(1).expect("one edge"),
        &|| false,
    )
    .expect_err("edge limit");
    assert_eq!(limited.path(), "raster.limits.flattened_edges");
    assert!(OutputRasterTarget::new(100_000, 100_000).is_err());
    let malformed = GeometryOutput::CanonicalMarks(vec![circle(3, 5.0, 5.0, f64::NAN)]);
    let invalid = rasterize_geometry_occupancy(
        &malformed,
        &canvas,
        target,
        RasterizationLimits::default(),
        &|| false,
    )
    .expect_err("public geometry input must use normal scene validation");
    assert_eq!(invalid.path(), "scene.layer.geometry");
    let tiny_canvas = CanvasSpec {
        width: f64::MIN_POSITIVE,
        height: 14.0,
    };
    let nonfinite_scale = rasterize_geometry_occupancy(
        &geometry,
        &tiny_canvas,
        target,
        RasterizationLimits::default(),
        &|| false,
    )
    .expect_err("a finite canvas can still overflow its target scale");
    assert_eq!(nonfinite_scale.path(), "raster.output_transform");
    assert!(OccupancyWindow::new(1.0, 2.0, 0.0, 3.0).is_err());
    assert!(OccupancyWindow::new(f64::NAN, 2.0, 3.0, 3.0).is_err());
    let outside = rasterize_geometry_occupancy_window(
        &geometry,
        &canvas,
        OccupancyWindow::new(15.0, 1.0, 2.0, 3.0).expect("finite window"),
        target,
        RasterizationLimits::default(),
        &|| false,
    )
    .expect_err("window cannot extend beyond canvas");
    assert_eq!(outside.path(), "raster.window");
}
