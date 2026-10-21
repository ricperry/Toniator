use toniator_domain::{
    CanvasSpec, ChannelId, ColorValue, PathStrokeStyle, PatternMechanismId, PatternOutputLayerId,
    StrokeCap, StrokeJoin,
};
use toniator_geometry::{
    CanonicalConnections, CanonicalFillRule, CanonicalMark, CanonicalPathMark, CanonicalStroke,
    ConnectionPathId, CurvePath, FamilySiteId, FamilySiteProvenance, PathClosure, PathLocation,
    Point2, SiteScope, StrokeProfileSample, VariableWidthOutlineLimits, VariableWidthPathSample,
    build_variable_width_outline_cancellable,
};
use toniator_render::{
    GeometryOutput, PngStreamOptions, RasterBackground, RasterizationLimits, RenderLayer,
    RenderOutputLayer, RenderScene, StreamingRasterTarget, VectorOccupancyPrecision,
    measure_geometry_vector_occupancy, rasterize, write_png_streaming, write_svg,
};

/// Returns existing family-point identity without creating synthetic graph vertices.
fn site(ordinal: usize) -> FamilySiteId {
    FamilySiteId {
        mechanism_id: PatternMechanismId(2),
        ordinal,
    }
}

/// Creates an ordinary source-provenance circle for the mixed consumer contract.
fn circle(x: f64) -> CanonicalMark {
    CanonicalMark::Circle {
        source_site_id: site(2),
        center: Point2::new(x, 8.0),
        radius: 3.0,
        scope: SiteScope::Canvas,
        provenance: FamilySiteProvenance::Random {
            candidate_ordinal: 2,
            accepted_ordinal: 2,
            exclusion_neighbor_ordinal: None,
        },
        fill_rule: CanonicalFillRule::EvenOdd,
    }
}

/// Builds one genuine positive connection path and its existing canonical round outline.
/// # Panics
/// Panics only if the bounded validation fixture cannot be constructed or accessed.
fn stroke() -> CanonicalStroke {
    let path = CurvePath::line(Point2::new(4.0, 8.0), Point2::new(16.0, 8.0)).unwrap();
    let locations = [
        PathLocation::new(0, 0.0).unwrap(),
        PathLocation::new(0, 1.0).unwrap(),
    ];
    let widths = locations.map(|location| VariableWidthPathSample {
        location,
        width: 4.0,
    });
    let style = PathStrokeStyle {
        join: StrokeJoin::Round,
        cap: StrokeCap::Round,
    };
    let outline = build_variable_width_outline_cancellable(
        &path,
        &widths,
        style,
        0.0,
        0.125,
        VariableWidthOutlineLimits::new(1000).unwrap(),
        &|| false,
    )
    .unwrap();
    let profile = locations
        .into_iter()
        .map(|location| StrokeProfileSample {
            location,
            center: path.point_at(location).unwrap(),
            normalized_thickness: 1.0,
            width: 4.0,
        })
        .collect();
    CanonicalStroke::new_connection(
        ConnectionPathId {
            output_layer_id: PatternOutputLayerId(1),
            component_minimum: site(0),
            component_ordinal: 0,
            first_endpoint: site(0),
            last_endpoint: site(1),
            ordinal: 0,
        },
        path,
        4.0,
        style,
        profile,
        outline,
    )
    .unwrap()
}

/// Creates one solid-painted output and lets the shared scene validate its complete geometry.
/// # Errors
/// Returns shared geometry or presentation validation diagnostics without publishing a scene.
fn scene(
    geometry: GeometryOutput,
    alpha: f64,
) -> Result<RenderScene, toniator_render::RenderError> {
    let layer = RenderLayer::new_for_output(
        ChannelId(1),
        true,
        ColorValue {
            red: 0.25,
            green: 0.5,
            blue: 0.75,
            alpha,
        },
        1.0,
        PatternOutputLayerId(1),
        geometry,
    )?;
    RenderScene::new(
        CanvasSpec {
            width: 32.0,
            height: 16.0,
        },
        "family".into(),
        "realization".into(),
        vec![layer],
    )
}

/// Proves both collections reach native raster, banded PNG, editable SVG and opaque occupancy.
/// # Panics
/// Panics if an ordinary mixed output loses either collection or consumer pixels disagree.
#[test]
fn mixed_connections_reach_all_shared_consumers_with_overlap_and_transparency() {
    for x in [24.0, 12.0] {
        for alpha in [1.0, 0.5] {
            let geometry = GeometryOutput::CanonicalConnections(CanonicalConnections {
                strokes: vec![stroke()],
                isolated_points: vec![circle(x)],
            });
            assert_eq!(geometry.primitive_count(), 2);
            let occupancy = measure_geometry_vector_occupancy(
                &geometry,
                &CanvasSpec {
                    width: 32.0,
                    height: 16.0,
                },
                VectorOccupancyPrecision::Baseline,
                RasterizationLimits::default(),
                &|| false,
            )
            .unwrap();
            assert!(occupancy.fraction > 0.1);
            let rendered = scene(geometry, alpha).unwrap();
            assert_eq!(rendered.circular_mark_count(), 2);
            let raster = rasterize(&rendered, RasterBackground::Transparent).unwrap();
            let mut png = Vec::new();
            write_png_streaming(
                &rendered,
                StreamingRasterTarget::new(32, 16).unwrap(),
                PngStreamOptions {
                    maximum_band_rows: 3,
                    ..PngStreamOptions::default()
                },
                RasterizationLimits::default(),
                &mut png,
                &|| false,
                &|_, _| {},
            )
            .unwrap();
            let decoded = image::load_from_memory(&png).unwrap().into_rgba8();
            assert_eq!(decoded.as_raw(), raster.pixels());
            assert!(raster.pixels().chunks_exact(4).any(|pixel| pixel[3] > 0));
            assert!(raster.pixels()[(8 * 32 + 8) * 4 + 3] > 0);
            if x == 24.0 {
                assert!(raster.pixels()[(8 * 32 + 24) * 4 + 3] > 0);
                let without_circle =
                    scene(GeometryOutput::CanonicalStrokes(vec![stroke()]), alpha).unwrap();
                assert_eq!(
                    rasterize(&without_circle, RasterBackground::Transparent)
                        .unwrap()
                        .pixels()[(8 * 32 + 24) * 4 + 3],
                    0
                );
            }

            let svg = write_svg(&rendered).unwrap();
            assert!(svg.contains("<path"));
            let strokes_only =
                scene(GeometryOutput::CanonicalStrokes(vec![stroke()]), alpha).unwrap();
            assert_ne!(svg, write_svg(&strokes_only).unwrap());
            let stroke_coverage = measure_geometry_vector_occupancy(
                &GeometryOutput::CanonicalStrokes(vec![stroke()]),
                &CanvasSpec {
                    width: 32.0,
                    height: 16.0,
                },
                VectorOccupancyPrecision::Baseline,
                RasterizationLimits::default(),
                &|| false,
            )
            .unwrap()
            .fraction;
            assert!(occupancy.fraction > stroke_coverage);
        }
    }
}

/// Retains the mixed variant in scene identity and rejects noncircle or per-primitive paint misuse.
/// # Panics
/// Panics if invalid mixed payloads pass validation or identity erases the output variant.
#[test]
fn mixed_identity_and_validation_preserve_the_single_output_contract() {
    let plain = scene(GeometryOutput::CanonicalStrokes(vec![stroke()]), 1.0).unwrap();
    let mixed = scene(
        GeometryOutput::CanonicalConnections(CanonicalConnections {
            strokes: vec![stroke()],
            isolated_points: Vec::new(),
        }),
        1.0,
    )
    .unwrap();
    assert_ne!(plain.identity(), mixed.identity());
    let first = scene(
        GeometryOutput::CanonicalConnections(CanonicalConnections {
            strokes: vec![stroke()],
            isolated_points: vec![circle(24.0)],
        }),
        1.0,
    )
    .unwrap();
    let changed_circle = scene(
        GeometryOutput::CanonicalConnections(CanonicalConnections {
            strokes: vec![stroke()],
            isolated_points: vec![circle(25.0)],
        }),
        1.0,
    )
    .unwrap();
    assert_ne!(first.identity(), changed_circle.identity());

    let points = [
        Point2::new(1.0, 1.0),
        Point2::new(2.0, 1.0),
        Point2::new(1.0, 2.0),
        Point2::new(1.0, 1.0),
    ];
    let path = CurvePath::polyline(points.to_vec(), PathClosure::Closed).unwrap();
    let closed = CanonicalPathMark::new(
        site(3),
        path,
        SiteScope::Canvas,
        FamilySiteProvenance::Random {
            candidate_ordinal: 3,
            accepted_ordinal: 3,
            exclusion_neighbor_ordinal: None,
        },
        CanonicalFillRule::EvenOdd,
    )
    .unwrap();
    assert!(
        scene(
            GeometryOutput::CanonicalConnections(CanonicalConnections {
                strokes: Vec::new(),
                isolated_points: vec![CanonicalMark::ClosedPath(closed)]
            }),
            1.0
        )
        .is_err()
    );
    for geometry in [
        GeometryOutput::CanonicalConnections(CanonicalConnections::default()),
        GeometryOutput::CanonicalConnections(CanonicalConnections {
            strokes: vec![stroke()],
            isolated_points: vec![circle(24.0)],
        }),
    ] {
        let output = RenderOutputLayer::new(PatternOutputLayerId(1), geometry, Some(Vec::new()));
        assert!(
            RenderLayer::new_outputs(
                ChannelId(1),
                true,
                ColorValue {
                    red: 1.0,
                    green: 0.0,
                    blue: 0.0,
                    alpha: 1.0
                },
                1.0,
                vec![output]
            )
            .is_err()
        );
    }
}
