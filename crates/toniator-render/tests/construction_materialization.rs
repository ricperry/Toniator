//! Exact construction-efficiency witnesses retain the complete logical source and current consumers.
use toniator_domain::{CanvasSpec, ChannelId, ColorValue, GuideDimensionId, PathStrokeStyle};
use toniator_geometry::*;
use toniator_render::*;

/// Constructs a long exterior excursion with preserved interior endpoints, round corners and reentry.
/// # Panics
/// Panics if the deterministic finite fixture violates the ordinary canonical path contract.
fn fixture(mode: ConstructionMode, bias: f64) -> (GeometryOutput, ConstructionMetrics) {
    let canvas = Bounds::new(Point2::new(0., 0.), Point2::new(32., 24.)).unwrap();
    let mut points = vec![Point2::new(4., 5.), Point2::new(-15., 5.)];
    points.extend((0..80).map(|i| Point2::new(-20. - i as f64 * 0.5, 6. + (i % 3) as f64)));
    points.extend([Point2::new(-15., 16.), Point2::new(27., 16.)]);
    let path = CurvePath::polyline(points, PathClosure::Open).unwrap();
    let mut profile = Vec::new();
    for (i, segment) in path.segments().iter().enumerate() {
        for parameter in [0., 1.] {
            profile.push(StrokeProfileSample {
                location: PathLocation::new(i, parameter).unwrap(),
                center: segment.point_at(parameter).unwrap(),
                normalized_thickness: (2. + (i % 4) as f64 * 0.1) / 4.,
                width: 2. + (i % 4) as f64 * 0.1,
            });
        }
    }
    let samples = profile
        .iter()
        .map(|v| VariableWidthPathSample {
            location: v.location,
            width: v.width,
        })
        .collect::<Vec<_>>();
    let context = ConstructionContext::new(mode, Some(canvas), true);
    let outline = context
        .run(|| {
            build_variable_width_outline_cancellable(
                &path,
                &samples,
                PathStrokeStyle::default(),
                bias,
                0.125,
                VariableWidthOutlineLimits::new(usize::MAX).unwrap(),
                &|| false,
            )
        })
        .unwrap();
    let stroke = CanonicalStroke::new(
        StructuralPathInstanceId::guide_dimension(GuideDimensionId(1), 0, 0),
        None,
        path,
        4.,
        PathStrokeStyle::default(),
        profile,
        outline,
    )
    .unwrap();
    (
        GeometryOutput::CanonicalStrokes(vec![stroke]),
        context.measurements.unwrap().snapshot(),
    )
}
/// Creates the ordinary shared canonical scene without background compositing or extra drawing layers.
/// # Panics
/// Panics if canonical fixture validation fails.
fn scene(geometry: GeometryOutput) -> RenderScene {
    RenderScene::new(
        CanvasSpec {
            width: 32.,
            height: 24.,
        },
        "construction-family".into(),
        "construction-realization".into(),
        vec![
            RenderLayer::new(
                ChannelId(1),
                true,
                ColorValue {
                    red: 0.2,
                    green: 0.4,
                    blue: 0.7,
                    alpha: 1.,
                },
                1.,
                geometry,
            )
            .unwrap(),
        ],
    )
    .unwrap()
}
/// Requires real constructor reduction and exact pixels across current target scales and consumers.
/// # Panics
/// Panics if pruning alters pixels, occupancy or full fallback, or merely post-filters built arrays.
#[test]
fn exterior_outline_prunes_before_construction_and_preserves_all_consumer_pixels() {
    for bias in [-1., 0., 1.] {
        let (full, fc) = fixture(ConstructionMode::Complete, bias);
        let (pruned, pc) = fixture(ConstructionMode::Canvas, bias);
        assert_eq!(fc.outline_commands, pc.outline_commands);
        assert!(
            pc.outline_constructors < fc.outline_constructors / 2,
            "{fc:?} {pc:?}"
        );
        let a = scene(full.clone());
        let b = scene(pruned.clone());
        for (width, height) in [(32, 24), (96, 72), (97, 41)] {
            let target = OutputRasterTarget::new(width, height).unwrap();
            for aa in [RasterAntialiasing::On, RasterAntialiasing::Off] {
                let ap =
                    rasterize_output(&a, RasterBackground::Transparent, Some(target), aa).unwrap();
                let bp =
                    rasterize_output(&b, RasterBackground::Transparent, Some(target), aa).unwrap();
                assert_eq!(ap.pixels(), bp.pixels(), "{bias} {width} {height} {aa:?}");
            }
        }
        let ap = rasterize_preview(&a, PreviewRasterTarget::new(19, 17).unwrap()).unwrap();
        let bp = rasterize_preview(&b, PreviewRasterTarget::new(19, 17).unwrap()).unwrap();
        assert_eq!(ap.pixels(), bp.pixels());
        let target = OutputRasterTarget::new(32, 24).unwrap();
        let am =
            rasterize_geometry_occupancy(&full, a.canvas(), target, Default::default(), &|| false)
                .unwrap();
        let bm =
            rasterize_geometry_occupancy(&pruned, b.canvas(), target, Default::default(), &|| {
                false
            })
            .unwrap();
        assert_eq!(am, bm);
        let av = measure_geometry_vector_occupancy(
            &full,
            a.canvas(),
            VectorOccupancyPrecision::Tight,
            Default::default(),
            &|| false,
        )
        .unwrap();
        let bv = measure_geometry_vector_occupancy(
            &pruned,
            b.canvas(),
            VectorOccupancyPrecision::Tight,
            Default::default(),
            &|| false,
        )
        .unwrap();
        assert_eq!(av.fraction, bv.fraction);
        let diagnostic = ConstructionContext::new(ConstructionMode::Complete, None, true);
        let svg = diagnostic.run(|| write_svg(&b)).unwrap();
        assert!(
            diagnostic
                .measurements
                .as_ref()
                .unwrap()
                .snapshot()
                .full_rematerializations
                > 0
        );
        assert!(svg.contains("nonzero"));
        assert_eq!(write_svg(&a).unwrap(), svg);
        let restored = prepare_geometry_for_canvas(
            &pruned,
            b.canvas(),
            "forced-current-contract-mismatch",
            &|| false,
        )
        .unwrap();
        let GeometryOutput::CanonicalStrokes(full_strokes) = &full else {
            unreachable!()
        };
        let GeometryOutput::CanonicalStrokes(restored_strokes) = restored.as_ref() else {
            unreachable!()
        };
        assert_eq!(
            full_strokes[0].outline.contours,
            restored_strokes[0].outline.contours
        );
        assert_eq!(full_strokes[0].profile, restored_strokes[0].profile);
    }
}
/// Preserves the full original logical segment limit even when exterior commands would be skipped.
/// # Panics
/// Panics if optimized materialization weakens explicit caller policy.
#[test]
fn pruned_outline_keeps_complete_logical_segment_limit() {
    let (geometry, _) = fixture(ConstructionMode::Canvas, 0.);
    let GeometryOutput::CanonicalStrokes(strokes) = geometry else {
        unreachable!()
    };
    let outline = &strokes[0].outline;
    let error = materialize_prepared_outline_cancellable(
        outline.prepared.as_ref().unwrap().clone(),
        outline.coverage,
        VariableWidthOutlineLimits::new(10).unwrap(),
        &|| false,
    )
    .unwrap_err();
    assert_eq!(error.path(), "curve.outline.segment_limit");
}

/// Retains three explicit paint slots with a certified exterior middle authored decision.
/// # Panics
/// Panics if finite frozen authored geometry fails its ordinary validation.
fn authored_fixture() -> (GeometryOutput, GeometryOutput, Vec<ColorValue>) {
    use std::sync::Arc;
    let prototype = Arc::new(PreparedMarkPrototype::new(
        CurvePath::polyline(
            vec![
                Point2::new(-2., -2.),
                Point2::new(2., -2.),
                Point2::new(2., 2.),
                Point2::new(-2., 2.),
            ],
            PathClosure::Closed,
        )
        .unwrap(),
        Point2::new(0., 0.),
    ));
    let coverage = MaterializationCoverage::for_canvas(
        Bounds::new(Point2::new(0., 0.), Point2::new(32., 24.)).unwrap(),
    );
    let mut complete = Vec::new();
    let mut pruned = Vec::new();
    for (i, destination) in [
        Point2::new(-60., -30.),
        Point2::new(5., 5.),
        Point2::new(-40., 9.),
        Point2::new(20., 15.),
        Point2::new(60., 50.),
    ]
    .into_iter()
    .enumerate()
    {
        let bounds = prepared_mark_hulls(
            prototype.path(),
            prototype.anchor(),
            1.,
            17.,
            destination,
            &|| false,
        )
        .unwrap()
        .0;
        let decision = DeferredPathMark {
            prototype: Arc::clone(&prototype),
            scale: 1.,
            orientation_degrees: 17.,
            destination,
            source_site_id: FamilySiteId {
                mechanism_id: toniator_domain::PatternMechanismId(1),
                ordinal: i,
            },
            scope: if i == 2 {
                SiteScope::Guard
            } else {
                SiteScope::Canvas
            },
            provenance: FamilySiteProvenance::Random {
                candidate_ordinal: i,
                accepted_ordinal: i,
                exclusion_neighbor_ordinal: None,
            },
            fill_rule: CanonicalFillRule::EvenOdd,
            bounds,
            coverage,
        };
        let full = CanonicalMark::ClosedPath(decision.materialize(&|| false).unwrap());
        complete.push(full.clone());
        pruned.push(if i == 2 {
            CanonicalMark::DeferredPath(Arc::new(decision))
        } else {
            full
        });
    }
    (
        GeometryOutput::CanonicalMarks(complete),
        GeometryOutput::CanonicalMarks(pruned),
        vec![
            ColorValue {
                red: 0.4,
                green: 0.4,
                blue: 0.4,
                alpha: 1.,
            },
            ColorValue {
                red: 1.,
                green: 0.,
                blue: 0.,
                alpha: 1.,
            },
            ColorValue {
                red: 0.,
                green: 1.,
                blue: 0.,
                alpha: 1.,
            },
            ColorValue {
                red: 0.,
                green: 0.,
                blue: 1.,
                alpha: 1.,
            },
            ColorValue {
                red: 0.8,
                green: 0.8,
                blue: 0.8,
                alpha: 1.,
            },
        ],
    )
}
/// Excludes representation identity metadata while retaining every visible SVG drawing command.
fn svg_drawing(svg: &str) -> String {
    svg.lines()
        .filter(|line| !line.starts_with("<metadata>"))
        .collect::<Vec<_>>()
        .join("\n")
}
/// Exercises explicit deferred paint indices and actual full fallback through current public consumers.
/// # Panics
/// Panics on changed RGBA, stream bytes, occupancy, multiplicity, vector area, paint order or fallback.
#[test]
fn deferred_authored_paint_slots_and_public_source_free_fallback_are_exact() {
    println!(
        "canonical_mark_bytes={} closed_path_bytes={} deferred_bytes={}",
        std::mem::size_of::<CanonicalMark>(),
        std::mem::size_of::<CanonicalPathMark>(),
        std::mem::size_of::<DeferredPathMark>()
    );
    let (full, pruned, paints) = authored_fixture();
    let make_scene = |geometry, canvas: CanvasSpec| {
        let GeometryOutput::CanonicalMarks(marks) = geometry else {
            unreachable!()
        };
        RenderScene::new_modeled(
            canvas,
            "marks-family".into(),
            "marks-realization".into(),
            toniator_domain::HalftoneChannelModel::SourceColorAlpha,
            vec![
                RenderLayer::new_source_color_geometry(
                    ChannelId(1),
                    true,
                    1.,
                    marks,
                    paints.clone(),
                )
                .unwrap(),
            ],
        )
        .unwrap()
    };
    let diagnostic = ConstructionContext::new(ConstructionMode::Complete, None, true);
    diagnostic
        .run(|| {
            write_svg(&make_scene(
                pruned.clone(),
                CanvasSpec {
                    width: 32.,
                    height: 24.,
                },
            ))
        })
        .unwrap();
    let measured = diagnostic.measurements.as_ref().unwrap().snapshot();
    assert_eq!(measured.full_rematerializations, 0);
    assert_eq!(measured.mark_constructors, 0);
    #[allow(dead_code)]
    enum PreviousCanonicalMark {
        Circle {
            source_site_id: FamilySiteId,
            center: Point2,
            radius: f64,
            scope: SiteScope,
            provenance: FamilySiteProvenance,
            fill_rule: CanonicalFillRule,
        },
        ClosedPath(CanonicalPathMark),
    }
    assert_eq!(
        std::mem::size_of::<CanonicalMark>(),
        std::mem::size_of::<PreviousCanonicalMark>()
    );
    let mut changed = pruned.clone();
    let GeometryOutput::CanonicalMarks(marks) = &mut changed else {
        unreachable!()
    };
    let CanonicalMark::DeferredPath(mark) = &mut marks[2] else {
        unreachable!()
    };
    std::sync::Arc::make_mut(mark).fill_rule = CanonicalFillRule::NonZero;
    assert_ne!(
        make_scene(
            pruned.clone(),
            CanvasSpec {
                width: 32.,
                height: 24.
            }
        )
        .identity(),
        make_scene(
            changed,
            CanvasSpec {
                width: 32.,
                height: 24.
            }
        )
        .identity()
    );
    for incompatible in [false, true] {
        let mut restricted = pruned.clone();
        if incompatible {
            let GeometryOutput::CanonicalMarks(marks) = &mut restricted else {
                unreachable!()
            };
            let CanonicalMark::DeferredPath(mark) = &mut marks[2] else {
                unreachable!()
            };
            let MaterializationCoverage::Canvas { contract, .. } =
                &mut std::sync::Arc::make_mut(mark).coverage
            else {
                unreachable!()
            };
            *contract = "forced-numeric-contract-mismatch";
        }
        for canvas in [
            CanvasSpec {
                width: 32.,
                height: 24.,
            },
            CanvasSpec {
                width: 40.,
                height: 30.,
            },
        ] {
            let a = make_scene(full.clone(), canvas.clone());
            let b = make_scene(restricted.clone(), canvas.clone());
            for target in [
                OutputRasterTarget::new(32, 24).unwrap(),
                OutputRasterTarget::new(123, 57).unwrap(),
            ] {
                assert_eq!(
                    rasterize_output(
                        &a,
                        RasterBackground::Transparent,
                        Some(target),
                        RasterAntialiasing::On
                    )
                    .unwrap()
                    .pixels(),
                    rasterize_output(
                        &b,
                        RasterBackground::Transparent,
                        Some(target),
                        RasterAntialiasing::On
                    )
                    .unwrap()
                    .pixels()
                );
            }
            assert_eq!(
                rasterize_preview(&a, PreviewRasterTarget::new(13, 11).unwrap())
                    .unwrap()
                    .pixels(),
                rasterize_preview(&b, PreviewRasterTarget::new(13, 11).unwrap())
                    .unwrap()
                    .pixels()
            );
            let mut ap = Vec::new();
            let mut bp = Vec::new();
            let target = StreamingRasterTarget::new(97, 41).unwrap();
            let options = PngStreamOptions {
                maximum_band_rows: 3,
                ..Default::default()
            };
            write_png_streaming(
                &a,
                target,
                options,
                Default::default(),
                &mut ap,
                &|| false,
                &|_, _| {},
            )
            .unwrap();
            write_png_streaming(
                &b,
                target,
                options,
                Default::default(),
                &mut bp,
                &|| false,
                &|_, _| {},
            )
            .unwrap();
            assert_eq!(ap, bp);
            let target = OutputRasterTarget::new(40, 30).unwrap();
            let window = OccupancyWindow::new(3., 2., 20., 15.).unwrap();
            assert_eq!(
                rasterize_geometry_occupancy_window(
                    &full,
                    &canvas,
                    window,
                    target,
                    Default::default(),
                    &|| false
                )
                .unwrap(),
                rasterize_geometry_occupancy_window(
                    &restricted,
                    &canvas,
                    window,
                    target,
                    Default::default(),
                    &|| false
                )
                .unwrap()
            );
            assert_eq!(
                rasterize_mark_multiplicity(&full, &canvas, target, Default::default(), &|| false)
                    .unwrap(),
                rasterize_mark_multiplicity(
                    &restricted,
                    &canvas,
                    target,
                    Default::default(),
                    &|| false
                )
                .unwrap()
            );
            assert_eq!(
                measure_geometry_vector_occupancy(
                    &full,
                    &canvas,
                    VectorOccupancyPrecision::Tight,
                    Default::default(),
                    &|| false
                )
                .unwrap()
                .fraction,
                measure_geometry_vector_occupancy(
                    &restricted,
                    &canvas,
                    VectorOccupancyPrecision::Tight,
                    Default::default(),
                    &|| false
                )
                .unwrap()
                .fraction
            );
            assert_eq!(
                svg_drawing(&write_svg(&a).unwrap()),
                svg_drawing(&write_svg(&b).unwrap())
            );
            let prepared = prepare_geometry_for_canvas(
                &restricted,
                &canvas,
                CANVAS_MATERIALIZATION_CONTRACT,
                &|| false,
            )
            .unwrap();
            if incompatible || canvas.width == 40. {
                assert_eq!(prepared.as_ref(), &full);
                assert_eq!(
                    svg_drawing(&write_svg(&a).unwrap()),
                    svg_drawing(&write_svg(&b).unwrap())
                );
            } else {
                assert!(matches!(prepared, std::borrow::Cow::Borrowed(_)));
            }
        }
    }
    let error = prepare_geometry_for_canvas(
        &pruned,
        &CanvasSpec {
            width: 40.,
            height: 30.,
        },
        CANVAS_MATERIALIZATION_CONTRACT,
        &|| true,
    )
    .unwrap_err();
    assert_eq!(error.path(), "evaluation.cancelled");
}

/// Emits a chosen full logical closed/open profile before exact procedural outline materialization.
/// # Panics
/// Panics if the finite fixture fails canonical validation.
fn shaped_outline(
    mode: ConstructionMode,
    points: Vec<Point2>,
    closure: PathClosure,
    zero: bool,
    bias: f64,
) -> GeometryOutput {
    let path = CurvePath::polyline(points, closure).unwrap();
    let mut profile = Vec::new();
    for (i, segment) in path.segments().iter().enumerate() {
        for parameter in [0., 1.] {
            let width = if zero && i % 3 == 1 { 0. } else { 8. };
            profile.push(StrokeProfileSample {
                location: PathLocation::new(i, parameter).unwrap(),
                center: segment.point_at(parameter).unwrap(),
                normalized_thickness: width / 8.,
                width,
            });
        }
    }
    let samples = profile
        .iter()
        .map(|v| VariableWidthPathSample {
            location: v.location,
            width: v.width,
        })
        .collect::<Vec<_>>();
    let context = ConstructionContext::new(
        mode,
        Some(Bounds::new(Point2::new(0., 0.), Point2::new(32., 24.)).unwrap()),
        false,
    );
    let outline = context
        .run(|| {
            build_variable_width_outline_cancellable(
                &path,
                &samples,
                PathStrokeStyle::default(),
                bias,
                0.125,
                VariableWidthOutlineLimits::new(100_000).unwrap(),
                &|| false,
            )
        })
        .unwrap();
    GeometryOutput::CanonicalStrokes(vec![
        CanonicalStroke::new(
            StructuralPathInstanceId::guide_dimension(GuideDimensionId(1), 0, 0),
            None,
            path,
            8.,
            PathStrokeStyle::default(),
            profile,
            outline,
        )
        .unwrap(),
    ])
}
/// Protects closed opposite rails, offscreen enclosers, self crossings, seam/zero runs and large bias.
/// # Panics
/// Panics if exact construction changes native RGBA or discards a canvas-enclosing contour.
#[test]
fn outline_encloser_closed_seam_zero_and_crossing_pixels_are_exact() {
    let cases = [
        (
            vec![
                Point2::new(-30., -30.),
                Point2::new(62., -30.),
                Point2::new(62., 54.),
                Point2::new(-30., 54.),
            ],
            PathClosure::Closed,
        ),
        (
            vec![
                Point2::new(4., 4.),
                Point2::new(-45., -20.),
                Point2::new(-55., 50.),
                Point2::new(28., 20.),
                Point2::new(4., 20.),
                Point2::new(28., 4.),
            ],
            PathClosure::Closed,
        ),
        (
            vec![
                Point2::new(-25., -4.),
                Point2::new(4., -4.),
                Point2::new(28., 4.),
                Point2::new(-45., 4.),
                Point2::new(-55., 12.),
                Point2::new(28., 20.),
            ],
            PathClosure::Open,
        ),
    ];
    for (points, closure) in cases {
        for zero in [false, true] {
            for bias in [-1., 0., 1.] {
                let full = shaped_outline(
                    ConstructionMode::Complete,
                    points.clone(),
                    closure,
                    zero,
                    bias,
                );
                let pruned = shaped_outline(
                    ConstructionMode::Canvas,
                    points.clone(),
                    closure,
                    zero,
                    bias,
                );
                let a = scene(full);
                let b = scene(pruned);
                for target in [
                    OutputRasterTarget::new(32, 24).unwrap(),
                    OutputRasterTarget::new(97, 41).unwrap(),
                ] {
                    assert_eq!(
                        rasterize_output(
                            &a,
                            RasterBackground::Transparent,
                            Some(target),
                            RasterAntialiasing::On
                        )
                        .unwrap()
                        .pixels(),
                        rasterize_output(
                            &b,
                            RasterBackground::Transparent,
                            Some(target),
                            RasterAntialiasing::On
                        )
                        .unwrap()
                        .pixels(),
                        "{closure:?} {zero} {bias}"
                    );
                }
                assert!(write_svg(&b).unwrap().contains("<svg"));
            }
        }
    }
}

/// Checks the same long exterior construction inside the actual mixed connection output variant.
/// # Panics
/// Panics if preparation loses singleton circles, mixed fallback, or the circle-only isolation guard.
#[test]
fn long_connection_and_singleton_mixed_output_preserves_materialization() {
    use toniator_domain::{PatternMechanismId, PatternOutputLayerId};
    let wrap = |geometry| {
        let GeometryOutput::CanonicalStrokes(mut strokes) = geometry else {
            unreachable!()
        };
        let first = FamilySiteId {
            mechanism_id: PatternMechanismId(1),
            ordinal: 0,
        };
        let last = FamilySiteId {
            mechanism_id: PatternMechanismId(1),
            ordinal: 1,
        };
        strokes[0].source_id = CanonicalStrokeSourceId::Connection(ConnectionPathId {
            output_layer_id: PatternOutputLayerId(1),
            component_minimum: first,
            component_ordinal: 0,
            first_endpoint: first,
            last_endpoint: last,
            ordinal: 0,
        });
        GeometryOutput::CanonicalConnections(CanonicalConnections {
            strokes,
            isolated_points: vec![CanonicalMark::Circle {
                source_site_id: last,
                center: Point2::new(16., 11.),
                radius: 1.5,
                scope: SiteScope::Canvas,
                provenance: FamilySiteProvenance::Random {
                    candidate_ordinal: 1,
                    accepted_ordinal: 1,
                    exclusion_neighbor_ordinal: None,
                },
                fill_rule: CanonicalFillRule::EvenOdd,
            }],
        })
    };
    let (full, fc) = fixture(ConstructionMode::Complete, 0.);
    let (pruned, pc) = fixture(ConstructionMode::Canvas, 0.);
    assert!(pc.outline_constructors < fc.outline_constructors / 2);
    let a = wrap(full);
    let b = wrap(pruned);
    let sa = scene(a.clone());
    let sb = scene(b.clone());
    assert_eq!(sa.circular_mark_count(), 2);
    assert_eq!(sb.circular_mark_count(), 2);
    assert_eq!(
        rasterize(&sa, RasterBackground::Transparent)
            .unwrap()
            .pixels(),
        rasterize(&sb, RasterBackground::Transparent)
            .unwrap()
            .pixels()
    );
    let restored =
        prepare_geometry_for_canvas(&b, sa.canvas(), "changed-current-contract", &|| false)
            .unwrap();
    let GeometryOutput::CanonicalConnections(complete) = &a else {
        unreachable!()
    };
    let GeometryOutput::CanonicalConnections(restored) = restored.as_ref() else {
        unreachable!()
    };
    assert_eq!(complete.isolated_points, restored.isolated_points);
    assert_eq!(
        complete.strokes[0].outline.contours,
        restored.strokes[0].outline.contours
    );
    let (_, authored, _) = authored_fixture();
    let GeometryOutput::CanonicalMarks(marks) = authored else {
        unreachable!()
    };
    let invalid = GeometryOutput::CanonicalConnections(CanonicalConnections {
        strokes: Vec::new(),
        isolated_points: vec![marks[2].clone()],
    });
    assert_eq!(
        RenderLayer::new(
            ChannelId(1),
            true,
            ColorValue {
                red: 0.,
                green: 0.,
                blue: 0.,
                alpha: 1.
            },
            1.,
            invalid
        )
        .unwrap_err()
        .path(),
        "scene.layer.connections"
    );
}
