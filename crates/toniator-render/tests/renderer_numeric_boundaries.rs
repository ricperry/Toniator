//! Focused public-consumer reproductions for admitted finite geometry at numeric boundaries.
use std::{fs, io::Cursor, path::PathBuf};
use toniator_domain::{CanvasSpec, ChannelId, ColorValue, PatternMechanismId};
use toniator_geometry::*;
use toniator_render::*;

/// Supplies stable source-independent fixture provenance without a calibration dependency.
fn provenance() -> FamilySiteProvenance {
    FamilySiteProvenance::Random {
        candidate_ordinal: 0,
        accepted_ordinal: 0,
        exclusion_neighbor_ordinal: None,
    }
}

/// Builds an admitted finite closed polygon using the actual public canonical constructor.
/// # Panics
/// Panics if fixture admission fails; this is distinct from a consumer numeric reproduction.
fn polygon(points: &[(f64, f64)], rule: CanonicalFillRule) -> GeometryOutput {
    GeometryOutput::CanonicalMarks(vec![CanonicalMark::ClosedPath(
        CanonicalPathMark::new(
            FamilySiteId {
                mechanism_id: PatternMechanismId(1),
                ordinal: 0,
            },
            CurvePath::polyline(
                points.iter().map(|&(x, y)| Point2::new(x, y)).collect(),
                PathClosure::Closed,
            )
            .unwrap(),
            SiteScope::Guard,
            provenance(),
            rule,
        )
        .unwrap(),
    )])
}

/// Builds a finite circle through the public scene-admitted canonical representation.
fn circle(x: f64, radius: f64) -> GeometryOutput {
    GeometryOutput::CanonicalMarks(vec![CanonicalMark::Circle {
        source_site_id: FamilySiteId {
            mechanism_id: PatternMechanismId(1),
            ordinal: 0,
        },
        center: Point2::new(x, 0.0),
        radius,
        scope: SiteScope::Guard,
        provenance: provenance(),
        fill_rule: CanonicalFillRule::EvenOdd,
    }])
}

/// Decodes exact native streamed RGBA without flattening or cleaning hidden RGB.
/// # Panics
/// Panics if a successful PNG writer returns malformed or non-RGBA data.
fn decode(bytes: &[u8]) -> Vec<u8> {
    let mut reader = png::Decoder::new(Cursor::new(bytes)).read_info().unwrap();
    let mut rgba = vec![0; reader.output_buffer_size().unwrap()];
    let frame = reader.next_frame(&mut rgba).unwrap();
    assert_eq!(frame.color_type, png::ColorType::Rgba);
    rgba.truncate(frame.buffer_size());
    rgba
}

/// Retains bounded consumer evidence without introducing a test-only serialization dependency.
#[derive(Debug)]
struct NumericObservation {
    consumer: String,
    outcome: &'static str,
    acceptable: bool,
    error: Option<(String, String)>,
    tested_pixels: usize,
    mismatches: Vec<(usize, usize, u8, u8)>,
    actual_mask: Vec<u8>,
}

/// Records success/error and exact off-boundary visible truth for one public consumer.
/// # Panics
/// Panics only if an 8×8 fixture returns a malformed cardinality.
fn observation(
    name: &str,
    outcome: Result<Vec<u8>, RenderError>,
    truth: &[Option<u8>],
) -> NumericObservation {
    match outcome {
        Err(error) => NumericObservation {
            consumer: name.into(),
            outcome: "error",
            acceptable: matches!(
                error.path(),
                "raster.flatten.numeric"
                    | "raster.sample.numeric"
                    | "raster.area.coordinate"
                    | "raster.area.circle"
                    | "raster.area.precision"
                    | "raster.output_transform"
                    | "raster.geometry.numeric"
                    | "png.target"
            ),
            error: Some((error.path().into(), error.message().into())),
            tested_pixels: 0,
            mismatches: Vec::new(),
            actual_mask: Vec::new(),
        },
        Ok(mask) => {
            assert_eq!(mask.len(), truth.len());
            let mismatches = mask
                .iter()
                .zip(truth)
                .enumerate()
                .filter_map(|(i, (&actual, &expected))| {
                    expected
                        .filter(|&v| v != actual)
                        .map(|expected| (i % 8, i / 8, actual, expected))
                })
                .collect::<Vec<_>>();
            NumericObservation {
                consumer: name.into(),
                outcome: "success",
                acceptable: mismatches.is_empty(),
                error: None,
                tested_pixels: truth.iter().filter(|v| v.is_some()).count(),
                mismatches,
                actual_mask: mask,
            }
        }
    }
}

/// Summarizes bounded failure categories without repeating masks or per-pixel records in logs.
fn summary(records: &[NumericObservation]) -> String {
    records
        .iter()
        .map(|r| {
            format!(
                "{}: {} acceptable={} mismatches={}/{} path={}",
                r.consumer,
                r.outcome,
                r.acceptable,
                r.mismatches.len(),
                r.tested_pixels,
                r.error.as_ref().map_or("", |(path, _)| path.as_str())
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// Exercises full/stream AA-off, geometry-only and AA-on consumers, retaining bounded evidence.
/// Errors are acceptable only when stable numeric diagnostics replace an uncertain successful fill.
/// The exact complete authored fixture is never clipped or edited before scene admission.
/// # Panics
/// Panics on invalid fixture admission, malformed successful output, or evidence I/O failure.
fn exercise(
    name: &str,
    geometry: GeometryOutput,
    canvas: CanvasSpec,
    truth: Vec<Option<u8>>,
    metadata: String,
) -> Vec<NumericObservation> {
    let root = numeric_output_root().join(name);
    fs::create_dir_all(&root).unwrap();
    let scene = RenderScene::new(
        canvas.clone(),
        "numeric-boundary".into(),
        name.into(),
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
                1.0,
                geometry.clone(),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let authored_bounds = match &geometry {
        GeometryOutput::CanonicalMarks(marks) => marks
            .iter()
            .map(|mark| match mark {
                CanonicalMark::ClosedPath(mark) => [
                    mark.bounds.min.x,
                    mark.bounds.min.y,
                    mark.bounds.max.x,
                    mark.bounds.max.y,
                ],
                CanonicalMark::Circle { center, radius, .. } => [
                    center.x - radius,
                    center.y - radius,
                    center.x + radius,
                    center.y + radius,
                ],
                CanonicalMark::DeferredPath(_) => {
                    unreachable!("numeric fixtures use public concrete marks")
                }
            })
            .collect::<Vec<_>>(),
        GeometryOutput::CanonicalStrokes(strokes) => strokes
            .iter()
            .map(|s| {
                let bounds = s.outline.bounds.unwrap();
                [bounds.min.x, bounds.min.y, bounds.max.x, bounds.max.y]
            })
            .collect(),
        _ => unreachable!("numeric fixtures use concrete canonical marks or strokes"),
    };
    assert!(authored_bounds.iter().flatten().all(|v| v.is_finite()));
    let mut records = Vec::new();
    for aa in [RasterAntialiasing::Off, RasterAntialiasing::On] {
        let aa_truth = if name == "healthy-tiny-canvas" && aa == RasterAntialiasing::On {
            vec![None; 64]
        } else {
            truth.clone()
        };
        let consumer = format!("full-{aa:?}");
        let rendered = rasterize_output(
            &scene,
            RasterBackground::Transparent,
            Some(OutputRasterTarget::new(8, 8).unwrap()),
            aa,
        );
        let mask = rendered.map(|surface| {
            fs::write(root.join(format!("{consumer}.rgba")), surface.pixels()).unwrap();
            fs::write(
                root.join(format!("{consumer}.png")),
                encode_png(&surface).unwrap(),
            )
            .unwrap();
            surface
                .pixels()
                .chunks_exact(4)
                .map(|p| u8::from(p[3] != 0))
                .collect()
        });
        records.push(observation(&consumer, mask, &aa_truth));
        let consumer = format!("stream-{aa:?}");
        let mut png = Vec::new();
        let result = write_png_streaming(
            &scene,
            StreamingRasterTarget::new(8, 8).unwrap(),
            PngStreamOptions {
                antialiasing: aa,
                maximum_band_rows: 2,
                ..Default::default()
            },
            RasterizationLimits::default(),
            &mut png,
            &|| false,
            &|_, _| {},
        );
        let mask = result.map(|_| {
            fs::write(root.join(format!("{consumer}.png")), &png).unwrap();
            let rgba = decode(&png);
            fs::write(root.join(format!("{consumer}.rgba")), &rgba).unwrap();
            rgba.chunks_exact(4).map(|p| u8::from(p[3] != 0)).collect()
        });
        records.push(observation(&consumer, mask, &aa_truth));
    }
    records.push(observation(
        "occupancy",
        rasterize_geometry_occupancy(
            &geometry,
            &canvas,
            OutputRasterTarget::new(8, 8).unwrap(),
            RasterizationLimits::default(),
            &|| false,
        )
        .map(|mask| mask.pixels().to_vec()),
        &truth,
    ));
    if matches!(
        geometry,
        GeometryOutput::CanonicalMarks(_) | GeometryOutput::CircularMarks(_)
    ) {
        records.push(observation(
            "multiplicity",
            rasterize_mark_multiplicity(
                &geometry,
                &canvas,
                OutputRasterTarget::new(8, 8).unwrap(),
                RasterizationLimits::default(),
                &|| false,
            )
            .map(|mask| {
                mask.counts()
                    .iter()
                    .map(|&v| u8::try_from(v).unwrap())
                    .collect()
            }),
            &truth,
        ));
    }
    fs::write(root.join("results.txt"), format!("admitted=true\nauthored_bounds={authored_bounds:?}\ncanvas={canvas:?}\ntarget=8x8\nscale={:?}\nidentity={:?}\n{metadata}\ntruth={truth:?}\n{records:#?}\n", [8.0/canvas.width,8.0/canvas.height], scene.identity())).unwrap();
    let mut table = String::from(
        "consumer\toutcome\tacceptable\terror_path\ttested_pixels\tmismatches\tactual_mask\n",
    );
    for record in &records {
        let mask = record
            .actual_mask
            .iter()
            .map(u8::to_string)
            .collect::<String>();
        table.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            record.consumer,
            record.outcome,
            record.acceptable,
            record.error.as_ref().map_or("", |(path, _)| path.as_str()),
            record.tested_pixels,
            record.mismatches.len(),
            mask
        ));
    }
    fs::write(root.join("results.tsv"), table).unwrap();
    records
}

/// Rejects silent loss when a valid enormous enclosing diamond overflows only after finite scaling.
/// Every target cell lies strictly inside |x|+|y|<M because canonical x+y<4 and M>4.
/// # Panics
/// Panics if any admitted consumer successfully returns wrong fill instead of a stable error.
#[test]
fn finite_diamond_mapping_overflow_never_silently_empties() {
    let m = f64::MAX / 2.0;
    assert!(m.is_finite() && m > 4.0 && !(4.0 * m).is_finite());
    let points = [(0.0, -m), (m, 0.0), (0.0, m), (-m, 0.0)];
    let mut all = Vec::new();
    for rule in [CanonicalFillRule::EvenOdd, CanonicalFillRule::NonZero] {
        all.extend(exercise(
            &format!("mapping-diamond-{rule:?}"),
            polygon(&points, rule),
            CanvasSpec {
                width: 2.0,
                height: 2.0,
            },
            vec![Some(1); 64],
            format!(
                "M_bits={} points={points:?} fill_rule={rule:?}; oracle: x+y<4<M",
                m.to_bits()
            ),
        ));
    }
    assert!(all.iter().all(|v| v.acceptable), "{}", summary(&all));
}

/// Rejects silent loss when an admitted huge circle's finite center and radii map to infinity.
/// Bounds[0,MAX]×[-M,M] are finite; x>=1/8 and x²+y²<8 imply2Mx>x²+y² for M>32.
/// # Panics
/// Panics if a public consumer silently loses the strictly covered visible pixels.
#[test]
fn finite_circle_mapping_overflow_never_silently_empties() {
    let m = f64::MAX / 2.0;
    assert!((m + m).is_finite() && (m - m).is_finite() && m > 32.0);
    let records = exercise(
        "mapping-circle",
        circle(m, m),
        CanvasSpec {
            width: 2.0,
            height: 2.0,
        },
        vec![Some(1); 64],
        format!(
            "center=({m},0) radius={m} bounds={:?}; oracle:2Mx>x²+y²",
            [m - m, -m, m + m, m]
        ),
    );
    assert!(
        records.iter().all(|v| v.acceptable),
        "{}",
        summary(&records)
    );
}

/// Rejects wrong ray/cross-product predicates even when every mapped vertex remains finite.
/// Exact local halfplanes near(0,0) give x>y inside and x<y outside; diagonal ties are excluded.
/// # Panics
/// Panics if any consumer publishes wrong off-diagonal fill rather than a stable numeric error.
#[test]
fn finite_vertices_predicate_overflow_preserves_local_halfplane_truth() {
    let truth = (0..8)
        .flat_map(|y| (0..8).map(move |x| if x == y { None } else { Some(u8::from(x > y)) }))
        .collect::<Vec<_>>();
    let mut all = Vec::new();
    for r in [1.0e18_f64, 1.0e100_f64, 1.0e200_f64] {
        let points = [(r, -r), (2.0 * r, 0.0), (r, r), (0.0, 0.0)];
        assert!(
            points
                .iter()
                .all(|&(x, y)| (4.0 * x).is_finite() && (4.0 * y).is_finite())
        );
        for rule in [CanonicalFillRule::EvenOdd, CanonicalFillRule::NonZero] {
            all.extend(exercise(&format!("predicate-diamond-{r:e}-{rule:?}"), polygon(&points, rule),
                CanvasSpec { width: 2.0, height: 2.0 }, truth.clone(),
                format!("R_bits={} points={points:?} fill_rule={rule:?}; oracle:off-diagonal x>y inside, x<y outside", r.to_bits())));
        }
    }
    assert!(all.iter().all(|v| v.acceptable), "{}", summary(&all));
}

/// Keeps healthy ordinary, representable tiny-canvas and large off-canvas observations accepted.
/// # Panics
/// Panics if the numeric reproductions require an arbitrary size cap or change healthy footprints.
#[test]
fn numeric_boundary_healthy_controls_remain_correct() {
    let ordinary = exercise(
        "healthy-ordinary",
        polygon(
            &[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
            CanonicalFillRule::NonZero,
        ),
        CanvasSpec {
            width: 2.0,
            height: 2.0,
        },
        (0..8)
            .flat_map(|y| (0..8).map(move |x| Some(u8::from(x < 4 && y < 4))))
            .collect(),
        "oracle: canonical unit square at exact 4x pixel phase".into(),
    );
    let far = exercise(
        "healthy-large-offcanvas",
        circle(1.0e100, 1.0e99),
        CanvasSpec {
            width: 2.0,
            height: 2.0,
        },
        vec![Some(0); 64],
        "oracle: finite circle minimum x=9e99 strictly outside canvas".into(),
    );
    // Center at origin; radius maps to2, so exact half-integer sample squares are small integers.
    let truth = (0_i32..8)
        .flat_map(|y| {
            (0_i32..8).map(move |x| Some(u8::from((2 * x + 1).pow(2) + (2 * y + 1).pow(2) < 16)))
        })
        .collect();
    let tiny = exercise(
        "healthy-tiny-canvas",
        circle(0.0, 2.5e-201),
        CanvasSpec {
            width: 1.0e-200,
            height: 1.0e-200,
        },
        truth,
        "oracle: mapped quarter circle center0 radius2; exact integer sample squares".into(),
    );
    let tiny_radius = exercise("healthy-tiny-positive-radius", circle(0.0, 1.0e-200),
        CanvasSpec { width: 2.0, height: 2.0 }, vec![Some(0);64],
        "oracle: tiny positive radius near origin covers no half-integer center; infinite positive norm is safely outside".into());
    for record in ordinary
        .into_iter()
        .chain(far)
        .chain(tiny)
        .chain(tiny_radius)
    {
        assert!(
            record.acceptable && record.outcome == "success",
            "{}",
            summary(&[record])
        );
    }
    let root = numeric_output_root().join("healthy-tiny-canvas");
    assert_eq!(
        fs::read(root.join("full-On.rgba")).unwrap(),
        fs::read(root.join("stream-On.rgba")).unwrap()
    );
}

/// Builds a public stroke using an intact caller-specified filled outline, without renderer clipping.
/// # Panics
/// Panics on invalid finite centerline/profile/outline fixture admission.
fn outlined_stroke(path: CurvePath) -> GeometryOutput {
    let bounds = path.bounds().unwrap();
    GeometryOutput::CanonicalStrokes(vec![
        CanonicalStroke::new(
            StructuralPathInstanceId::guide_dimension(toniator_domain::GuideDimensionId(1), 0, 0),
            None,
            CurvePath::line(Point2::new(0.0, 0.0), Point2::new(1.0, 0.0)).unwrap(),
            1.0,
            toniator_domain::PathStrokeStyle::default(),
            vec![StrokeProfileSample {
                location: PathLocation::new(0, 0.0).unwrap(),
                center: Point2::new(0.0, 0.0),
                normalized_thickness: 1.0,
                width: 1.0,
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

/// Exercises the same overflow witness through the stroke's sorted scanline crossing consumer.
/// # Panics
/// Panics if finite-overflow crossing arithmetic can publish wrong off-diagonal stroke pixels.
#[test]
fn stroke_scanline_numeric_overflow_never_sorts_invalid_crossings() {
    let r = 1.0e200;
    let path = CurvePath::polyline(
        vec![
            Point2::new(r, -r),
            Point2::new(2.0 * r, 0.0),
            Point2::new(r, r),
            Point2::new(0.0, 0.0),
        ],
        PathClosure::Closed,
    )
    .unwrap();
    let truth = (0..8)
        .flat_map(|y| (0..8).map(move |x| if x == y { None } else { Some(u8::from(x > y)) }))
        .collect();
    let records = exercise("predicate-stroke", outlined_stroke(path), CanvasSpec { width:2.0,height:2.0 }, truth,
        "oracle: nonzero stroke outline has same off-diagonal wedge x>y; no multiplicity consumer for strokes".into());
    assert!(
        records.iter().all(|v| v.acceptable),
        "{}",
        summary(&records)
    );
}

/// Checks mapped cubic controls and false zero flatness from an overflowing squared chord length.
/// # Panics
/// Panics if an admitted intersecting cubic bypasses sampled numeric guards via finite endpoints.
#[test]
fn sampled_cubic_controls_and_flatness_intermediates_fail_closed() {
    for (name, r, height, canvas_size) in [
        ("mapped-cubic-control", 1.0e100, 1.0e100, 1.0e-210),
        ("false-flatness-cubic", 1.0e100, 1.0, 1.0e-100),
    ] {
        let path = CurvePath::new(
            vec![
                CurveSegment::CubicBezier(
                    CubicBezierSegment::new(
                        Point2::new(0.0, 0.0),
                        Point2::new(0.0, height),
                        Point2::new(r, height),
                        Point2::new(r, 0.0),
                    )
                    .unwrap(),
                ),
                CurveSegment::Line(
                    LineSegment::new(Point2::new(r, 0.0), Point2::new(0.0, 0.0)).unwrap(),
                ),
            ],
            PathClosure::Closed,
        )
        .unwrap();
        let geometry = GeometryOutput::CanonicalMarks(vec![CanonicalMark::ClosedPath(
            CanonicalPathMark::new(
                FamilySiteId {
                    mechanism_id: PatternMechanismId(1),
                    ordinal: 0,
                },
                path,
                SiteScope::Guard,
                provenance(),
                CanonicalFillRule::NonZero,
            )
            .unwrap(),
        )]);
        let records = exercise(
            name,
            geometry,
            CanvasSpec {
                width: canvas_size,
                height: canvas_size,
            },
            vec![None; 64],
            format!(
                "r={r:e} height={height:e}; only numeric error contract asserted, no independent cubic fill oracle"
            ),
        );
        for record in records.iter().filter(|v| !v.consumer.ends_with("On")) {
            assert!(
                record.outcome == "error"
                    && record.error.as_ref().unwrap().0 == "raster.sample.numeric",
                "{}",
                summary(&records)
            );
        }
    }
}

/// Chooses explicit current evidence or process-local disposable scratch, never frozen numeric results.
fn numeric_output_root() -> PathBuf {
    std::env::var_os("TONIATOR_NUMERIC_OUTPUT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/validation/test-scratch")
                .join(format!("renderer-numeric-{}", std::process::id()))
        })
}

/// Rejects phantom first-column coverage outside an admitted enormous left-hand circle.
/// The exact canonical rightmost support is0; every positive visible sample lies outside.
/// # Panics
/// Panics on wrong successful coverage or a nonnumeric diagnostic.
#[test]
fn huge_left_circle_does_not_round_outside_samples_onto_boundary() {
    let mut all = Vec::new();
    for r in [1.0e18, 1.0e100] {
        all.extend(exercise(
            &format!("left-circle-{r:e}"),
            circle(-r, r),
            CanvasSpec {
                width: 2.0,
                height: 2.0,
            },
            vec![Some(0); 64],
            "oracle: canonical x>0 is strictly beyond center.x+radius=0".into(),
        ));
    }
    assert!(
        all.iter()
            .filter(|v| !v.consumer.ends_with("-On"))
            .all(|v| v.acceptable),
        "{}",
        summary(&all)
    );
}

/// Distinguishes authored positive radius from an underflowed mapped radius at the exact sample center.
/// The canonical sample equals the circle center, so empty success is never an acceptable result.
/// # Panics
/// Panics on invalid admission, evidence I/O or a silent empty public-consumer result.
#[test]
fn positive_authored_circle_radius_underflow_never_silently_disappears() {
    let mut geometry = circle(1.0e200, 1.0e-200);
    if let GeometryOutput::CanonicalMarks(marks) = &mut geometry
        && let CanonicalMark::Circle { center, .. } = &mut marks[0]
    {
        center.y = 1.0e200;
    }
    let canvas = CanvasSpec {
        width: 2.0e200,
        height: 2.0e200,
    };
    let scene = RenderScene::new(
        canvas.clone(),
        "underflow".into(),
        "positive-radius".into(),
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
                1.0,
                geometry.clone(),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let target = OutputRasterTarget::new(1, 1).unwrap();
    let mut observations = vec![(
        "full",
        rasterize_output(
            &scene,
            RasterBackground::Transparent,
            Some(target),
            RasterAntialiasing::Off,
        )
        .map(|s| u32::from(s.pixels()[3] > 0)),
    )];
    let mut png = Vec::new();
    let streamed = write_png_streaming(
        &scene,
        StreamingRasterTarget::new(1, 1).unwrap(),
        PngStreamOptions {
            antialiasing: RasterAntialiasing::Off,
            ..Default::default()
        },
        RasterizationLimits::default(),
        &mut png,
        &|| false,
        &|_, _| {},
    )
    .map(|_| u32::from(decode(&png)[3] > 0));
    observations.push(("stream", streamed));
    observations.push((
        "occupancy",
        rasterize_geometry_occupancy(
            &geometry,
            &canvas,
            target,
            RasterizationLimits::default(),
            &|| false,
        )
        .map(|s| u32::from(s.pixels()[0])),
    ));
    observations.push((
        "multiplicity",
        rasterize_mark_multiplicity(
            &geometry,
            &canvas,
            target,
            RasterizationLimits::default(),
            &|| false,
        )
        .map(|s| s.counts()[0]),
    ));
    let out = numeric_output_root().join("positive-derived-zero");
    fs::create_dir_all(&out).unwrap();
    fs::write(out.join("results.txt"),format!("admitted=true\ncanvas={canvas:?}\nexpected=1 or stable numeric error\n{observations:#?}\n")).unwrap();
    assert!(
        observations.iter().all(|(_, r)| match r {
            Ok(n) => *n == 1,
            Err(e) => e.path() == "raster.sample.numeric",
        }),
        "{observations:?}"
    );
}
