//! Current sampled finite-predicate and scanline contracts, independent of frozen numeric evidence.
use std::io::Cursor;
use toniator_domain::{CanvasSpec, ChannelId, ColorValue, PatternMechanismId};
use toniator_geometry::*;
use toniator_render::*;

/// Supplies intact source-independent canonical closed marks or a caller-owned stroke outline.
/// # Panics
/// Panics if the finite public fixture fails its existing canonical admission.
fn geometry(points: Vec<Point2>, rule: CanonicalFillRule, stroke: bool) -> GeometryOutput {
    let path = CurvePath::polyline(points, PathClosure::Closed).unwrap();
    if !stroke {
        return GeometryOutput::CanonicalMarks(vec![CanonicalMark::ClosedPath(
            CanonicalPathMark::new(
                FamilySiteId {
                    mechanism_id: PatternMechanismId(1),
                    ordinal: 0,
                },
                path,
                SiteScope::Guard,
                FamilySiteProvenance::Random {
                    candidate_ordinal: 0,
                    accepted_ordinal: 0,
                    exclusion_neighbor_ordinal: None,
                },
                rule,
            )
            .unwrap(),
        )]);
    }
    let bounds = path.bounds().unwrap();
    GeometryOutput::CanonicalStrokes(vec![
        CanonicalStroke::new(
            StructuralPathInstanceId::guide_dimension(toniator_domain::GuideDimensionId(1), 0, 0),
            None,
            CurvePath::line(Point2::new(0., 0.), Point2::new(1., 0.)).unwrap(),
            1.0,
            Default::default(),
            vec![StrokeProfileSample {
                location: PathLocation::new(0, 0.).unwrap(),
                center: Point2::new(0., 0.),
                normalized_thickness: 1.,
                width: 1.,
            }],
            CanonicalFilledOutline {
                full_segment_count: None,
                prepared: None,
                coverage: Default::default(),
                contours: vec![CanonicalOutlineContour {
                    segments: path.segments().to_vec(),
                }],
                fill_rule: CanonicalFillRule::NonZero,
                bounds: Some(bounds),
            },
        )
        .unwrap(),
    ])
}

/// Creates a paint-only wrapper around unchanged canonical geometry and the exact4x output frame.
/// # Panics
/// Panics if valid public fixture scene admission fails.
fn scene(geometry: GeometryOutput) -> RenderScene {
    RenderScene::new(
        CanvasSpec {
            width: 2.,
            height: 2.,
        },
        "sampled-finite".into(),
        "exact-halfplane".into(),
        vec![
            RenderLayer::new(
                ChannelId(1),
                true,
                ColorValue {
                    red: 1.,
                    green: 0.,
                    blue: 0.,
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

/// Decodes stream bytes without replacing hidden RGB or alpha.
/// # Panics
/// Panics if a successful PNG encoder produces malformed output.
fn decode(png: &[u8]) -> Vec<u8> {
    let mut reader = png::Decoder::new(Cursor::new(png)).read_info().unwrap();
    let mut out = vec![0; reader.output_buffer_size().unwrap()];
    let frame = reader.next_frame(&mut out).unwrap();
    out.truncate(frame.buffer_size());
    out
}

/// Requires in-domain finite-cancellation marks and stroke scanlines to return correct pixels, not errors.
/// Reversal/reflection and both winding rules retain analytic off-diagonal local halfplane truth.
/// # Panics
/// Panics on error, wrong sample ownership, occupancy/multiplicity drift or stream-band differences.
#[test]
fn finite_cancellation_has_correct_public_pixels_and_band_parity() {
    let target = OutputRasterTarget::new(8, 8).unwrap();
    let limits = RasterizationLimits::new(100)
        .unwrap()
        .with_max_boolean_operations(0);
    for r in [1e18, 1e100] {
        for reverse in [false, true] {
            for reflect in [false, true] {
                let mut points = vec![
                    Point2::new(r, -r),
                    Point2::new(2. * r, 0.),
                    Point2::new(r, r),
                    Point2::new(0., 0.),
                ];
                if reflect {
                    for p in &mut points {
                        std::mem::swap(&mut p.x, &mut p.y);
                    }
                }
                if reverse {
                    points.reverse();
                }
                for rule in [CanonicalFillRule::EvenOdd, CanonicalFillRule::NonZero] {
                    for stroke in [false, true] {
                        let g = geometry(points.clone(), rule, stroke);
                        let scene = scene(g.clone());
                        let full = rasterize_output_cancellable(
                            &scene,
                            RasterBackground::Transparent,
                            Some(target),
                            RasterAntialiasing::Off,
                            limits,
                            &|| false,
                        )
                        .unwrap();
                        let occupancy = rasterize_geometry_occupancy(
                            &g,
                            scene.canvas(),
                            target,
                            limits,
                            &|| false,
                        )
                        .unwrap();
                        let multiplicity = if stroke {
                            None
                        } else {
                            Some(
                                rasterize_mark_multiplicity(
                                    &g,
                                    scene.canvas(),
                                    target,
                                    limits,
                                    &|| false,
                                )
                                .unwrap(),
                            )
                        };
                        for y in 0..8 {
                            for x in 0..8 {
                                if x == y {
                                    continue;
                                }
                                let expected = u8::from(if reflect { x < y } else { x > y });
                                let i = y * 8 + x;
                                assert_eq!(
                                    u8::from(full.pixels()[i * 4 + 3] > 0),
                                    expected,
                                    "r{r} reverse{reverse} reflect{reflect} stroke{stroke}"
                                );
                                assert_eq!(occupancy.pixels()[i], expected);
                                if let Some(m) = &multiplicity {
                                    assert_eq!(m.counts()[i], u32::from(expected));
                                }
                            }
                        }
                        for rows in [1, 3, 8] {
                            let mut png = Vec::new();
                            write_png_streaming(
                                &scene,
                                StreamingRasterTarget::new(8, 8).unwrap(),
                                PngStreamOptions {
                                    antialiasing: RasterAntialiasing::Off,
                                    maximum_band_rows: rows,
                                    ..Default::default()
                                },
                                limits,
                                &mut png,
                                &|| false,
                                &|_, _| {},
                            )
                            .unwrap();
                            assert_eq!(decode(&png), full.pixels());
                        }
                    }
                }
            }
        }
    }
}

/// Refuses unsupported common predicate frames and versions only the sampled final-consumer identity.
/// # Panics
/// Panics on silently accepted unsupported signs, changed area identity or mutated canonical authority.
#[test]
fn unsupported_frame_errors_and_only_sampled_identity_changes() {
    let r = 1e200;
    let scene = scene(geometry(
        vec![
            Point2::new(r, -r),
            Point2::new(2. * r, 0.),
            Point2::new(r, r),
            Point2::new(0., 0.),
        ],
        CanonicalFillRule::NonZero,
        false,
    ));
    let before = scene.identity().clone();
    let error = rasterize_output(
        &scene,
        RasterBackground::Transparent,
        Some(OutputRasterTarget::new(8, 8).unwrap()),
        RasterAntialiasing::Off,
    )
    .unwrap_err();
    assert_eq!(error.path(), "raster.sample.numeric");
    assert_eq!(scene.identity(), &before);
    assert!(
        raster_output_identity(
            &scene,
            RasterBackground::Transparent,
            None,
            RasterAntialiasing::On
        )
        .contains(":toniator-raster-output-area-v2:")
    );
    assert!(
        raster_output_identity(
            &scene,
            RasterBackground::Transparent,
            None,
            RasterAntialiasing::Off
        )
        .contains(":toniator-raster-output-sampled-v3:")
    );
}

/// Computes independent robust right-ray parity or signed winding in the ordinary safe domain.
fn oracle(points: &[Point2], p: Point2, rule: CanonicalFillRule) -> bool {
    let mut winding = 0;
    let mut parity = false;
    for i in 0..points.len() {
        let a = points[i];
        let b = points[(i + 1) % points.len()];
        if (a.y > p.y) == (b.y > p.y) {
            continue;
        }
        let sign = robust::orient2d(
            robust::Coord { x: a.x, y: a.y },
            robust::Coord { x: b.x, y: b.y },
            robust::Coord { x: p.x, y: p.y },
        );
        if (b.y > a.y && sign > 0.0) || (b.y < a.y && sign < 0.0) {
            parity = !parity;
            winding += if b.y > a.y { 1 } else { -1 };
        }
    }
    match rule {
        CanonicalFillRule::EvenOdd => parity,
        CanonicalFillRule::NonZero => winding != 0,
    }
}

/// Preserves ordinary holes, shared vertices, self-crossing and exact boundary ownership in both rules.
/// # Panics
/// Panics on disagreement with independent mature membership or full/stream band differences.
#[test]
fn holes_shared_vertices_and_boundary_ties_match_mature_membership() {
    let fixtures = [
        vec![
            (0., 0.),
            (2., 0.),
            (2., 2.),
            (0., 2.),
            (0., 0.),
            (0.5, 0.5),
            (0.5, 1.5),
            (1.5, 1.5),
            (1.5, 0.5),
            (0.5, 0.5),
            (0., 0.),
        ],
        vec![(0., 0.), (2., 2.), (0., 2.), (2., 0.)],
        vec![
            (0., 0.),
            (1., 0.),
            (1., 1.),
            (0., 1.),
            (0., 0.),
            (1., 1.),
            (2., 1.),
            (2., 2.),
            (1., 2.),
            (1., 1.),
        ],
    ];
    for fixture in fixtures {
        for reverse in [false, true] {
            for rule in [CanonicalFillRule::EvenOdd, CanonicalFillRule::NonZero] {
                let mut points = fixture
                    .iter()
                    .map(|&(x, y)| Point2::new(x, y))
                    .collect::<Vec<_>>();
                if reverse {
                    points.reverse();
                }
                let scene = scene(geometry(points.clone(), rule, false));
                let full = rasterize_output(
                    &scene,
                    RasterBackground::Transparent,
                    Some(OutputRasterTarget::new(8, 8).unwrap()),
                    RasterAntialiasing::Off,
                )
                .unwrap();
                for y in 0..8 {
                    for x in 0..8 {
                        let expected = oracle(
                            &points,
                            Point2::new((x as f64 + 0.5) / 4., (y as f64 + 0.5) / 4.),
                            rule,
                        );
                        assert_eq!(
                            full.pixels()[(y * 8 + x) * 4 + 3] > 0,
                            expected,
                            "{rule:?} reverse{reverse} at{x},{y}"
                        );
                    }
                }
                for rows in [1, 3] {
                    let mut png = Vec::new();
                    write_png_streaming(
                        &scene,
                        StreamingRasterTarget::new(8, 8).unwrap(),
                        PngStreamOptions {
                            antialiasing: RasterAntialiasing::Off,
                            maximum_band_rows: rows,
                            ..Default::default()
                        },
                        RasterizationLimits::default(),
                        &mut png,
                        &|| false,
                        &|_, _| {},
                    )
                    .unwrap();
                    assert_eq!(decode(&png), full.pixels());
                }
            }
        }
    }
}

/// Propagates cancellation and the existing edge ledger without a new sampled-query allowance.
/// Streaming failure may leave staging bytes with its caller; no zero-byte guarantee is asserted.
/// # Panics
/// Panics on accepted cancellation/edge exhaustion or a changed diagnostic authority.
#[test]
fn cancellation_and_edge_exhaustion_never_publish_a_surface() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let r = 1e100;
    let scene = scene(geometry(
        vec![
            Point2::new(r, -r),
            Point2::new(2. * r, 0.),
            Point2::new(r, r),
            Point2::new(0., 0.),
        ],
        CanonicalFillRule::NonZero,
        true,
    ));
    let target = OutputRasterTarget::new(8, 8).unwrap();
    for threshold in [0, 64] {
        let calls = AtomicUsize::new(0);
        let cancel = || calls.fetch_add(1, Ordering::SeqCst) >= threshold;
        let error = rasterize_output_cancellable(
            &scene,
            RasterBackground::Transparent,
            Some(target),
            RasterAntialiasing::Off,
            RasterizationLimits::new(100).unwrap(),
            &cancel,
        )
        .unwrap_err();
        assert_eq!(error.path(), "evaluation.cancelled");
    }
    let error = rasterize_output_cancellable(
        &scene,
        RasterBackground::Transparent,
        Some(target),
        RasterAntialiasing::Off,
        RasterizationLimits::new(1).unwrap(),
        &|| false,
    )
    .unwrap_err();
    assert_eq!(error.path(), "raster.limits.flattened_edges");
}
