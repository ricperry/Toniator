//! Current fill, output identity and finite-transform contracts across renderer consumers.
use std::{collections::HashSet, io::Cursor};
use toniator_domain::{
    CanvasSpec, ChannelId, ColorValue, GuideDimensionId, HalftoneChannelModel, PathStrokeStyle,
    PatternMechanismId, PatternOutputLayerId,
};
use toniator_geometry::*;
use toniator_render::*;

/// Supplies an opaque linear fixture color without altering source alpha semantics.
fn color(red: f64, green: f64, blue: f64) -> ColorValue {
    ColorValue {
        red,
        green,
        blue,
        alpha: 1.0,
    }
}

/// Builds a closed integer-coordinate path with its caller-selected canonical winding rule.
/// # Panics
/// Panics only if finite fixture coordinates cease to form valid canonical geometry.
fn mark(points: &[(i32, i32)], rule: CanonicalFillRule) -> CanonicalMark {
    CanonicalMark::ClosedPath(
        CanonicalPathMark::new(
            FamilySiteId {
                mechanism_id: PatternMechanismId(1),
                ordinal: 0,
            },
            CurvePath::polyline(
                points
                    .iter()
                    .map(|&(x, y)| Point2::new(x as f64, y as f64))
                    .collect(),
                PathClosure::Closed,
            )
            .unwrap(),
            SiteScope::Canvas,
            FamilySiteProvenance::Random {
                candidate_ordinal: 0,
                accepted_ordinal: 0,
                exclusion_neighbor_ordinal: None,
            },
            rule,
        )
        .unwrap(),
    )
}

/// Builds a validated source-over scene with no fitting, sampling or calibration dependency.
/// # Panics
/// Panics on invalid fixture canvas, geometry, or paint.
fn scene(canvas: CanvasSpec, geometry: GeometryOutput) -> RenderScene {
    RenderScene::new(
        canvas,
        "consumer-contract".into(),
        "fixture".into(),
        vec![RenderLayer::new(ChannelId(1), true, color(1.0, 0.0, 0.0), 1.0, geometry).unwrap()],
    )
    .unwrap()
}

/// Supplies doubled, reverse-cancelled and clipped self-crossing paths on integer coordinates.
fn paths() -> Vec<Vec<(i32, i32)>> {
    vec![
        vec![
            (0, 0),
            (4, 0),
            (4, 4),
            (0, 4),
            (0, 0),
            (4, 0),
            (4, 4),
            (0, 4),
        ],
        vec![
            (0, 0),
            (4, 0),
            (4, 4),
            (0, 4),
            (0, 0),
            (0, 4),
            (4, 4),
            (4, 0),
        ],
        vec![
            (-2, -1),
            (5, -1),
            (5, 4),
            (-2, 4),
            (-2, -1),
            (5, -1),
            (5, 4),
            (-2, 4),
        ],
        vec![(-1, -1), (7, 4), (-1, 4), (7, -1)],
    ]
}

/// Computes exact integer ray winding at half-integer pixel centers, independently of f64 walkers.
/// Points on a boundary follow the same half-open crossing convention as a filled polygon.
fn expected(points: &[(i32, i32)], rule: CanonicalFillRule) -> Vec<u8> {
    (0..5)
        .flat_map(|y| {
            (0..6).map(move |x| {
                let p = (2 * x + 1, 2 * y + 1);
                let mut winding = 0_i32;
                for i in 0..points.len() {
                    let a = (2 * points[i].0, 2 * points[i].1);
                    let b = (
                        2 * points[(i + 1) % points.len()].0,
                        2 * points[(i + 1) % points.len()].1,
                    );
                    let cross = i64::from(b.0 - a.0) * i64::from(p.1 - a.1)
                        - i64::from(b.1 - a.1) * i64::from(p.0 - a.0);
                    if a.1 <= p.1 && b.1 > p.1 && cross > 0 {
                        winding += 1;
                    }
                    if b.1 <= p.1 && a.1 > p.1 && cross < 0 {
                        winding -= 1;
                    }
                }
                u8::from(match rule {
                    CanonicalFillRule::EvenOdd => winding % 2 != 0,
                    CanonicalFillRule::NonZero => winding != 0,
                })
            })
        })
        .collect()
}

/// Encodes and decodes native straight RGBA bands without a background composite.
/// # Panics
/// Panics on invalid fixture encoding or unexpected PNG format.
fn streamed(scene: &RenderScene, width: u32, height: u32, aa: RasterAntialiasing) -> Vec<u8> {
    let mut bytes = Vec::new();
    write_png_streaming(
        scene,
        StreamingRasterTarget::new(width, height).unwrap(),
        PngStreamOptions {
            antialiasing: aa,
            maximum_band_rows: 2,
            ..Default::default()
        },
        RasterizationLimits::default(),
        &mut bytes,
        &|| false,
        &|_, _| {},
    )
    .unwrap();
    let mut reader = png::Decoder::new(Cursor::new(bytes)).read_info().unwrap();
    let mut rgba = vec![0; reader.output_buffer_size().unwrap()];
    let frame = reader.next_frame(&mut rgba).unwrap();
    assert_eq!(frame.color_type, png::ColorType::Rgba);
    rgba.truncate(frame.buffer_size());
    rgba
}

/// Proves full AA-off and geometry-only observations honor both canonical fill rules.
/// # Panics
/// Panics on changed winding truth, count multiplicity, clipping, or scene identity.
#[test]
fn aa_off_fill_rules_match_exact_winding_and_occupancy() {
    let canvas = CanvasSpec {
        width: 6.0,
        height: 5.0,
    };
    let target = OutputRasterTarget::new(6, 5).unwrap();
    for points in paths() {
        for rule in [CanonicalFillRule::EvenOdd, CanonicalFillRule::NonZero] {
            let geometry = GeometryOutput::CanonicalMarks(vec![mark(&points, rule)]);
            let scene = scene(canvas.clone(), geometry.clone());
            let identity = scene.identity().clone();
            let truth = expected(&points, rule);
            let surface = rasterize_output(
                &scene,
                RasterBackground::Transparent,
                Some(target),
                RasterAntialiasing::Off,
            )
            .unwrap();
            assert_eq!(
                surface
                    .pixels()
                    .chunks_exact(4)
                    .map(|p| u8::from(p[3] != 0))
                    .collect::<Vec<_>>(),
                truth,
                "full {rule:?} {points:?}"
            );
            let mask = rasterize_geometry_occupancy(
                &geometry,
                &canvas,
                target,
                RasterizationLimits::default(),
                &|| false,
            )
            .unwrap();
            assert_eq!(mask.pixels(), truth);
            let window = rasterize_geometry_occupancy_window(
                &geometry,
                &canvas,
                OccupancyWindow::new(0.0, 0.0, 3.0, 5.0).unwrap(),
                OutputRasterTarget::new(3, 5).unwrap(),
                RasterizationLimits::default(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                window.pixels(),
                truth
                    .chunks_exact(6)
                    .flat_map(|row| row[..3].iter().copied())
                    .collect::<Vec<_>>()
            );
            let counts = rasterize_mark_multiplicity(
                &geometry,
                &canvas,
                target,
                RasterizationLimits::default(),
                &|| false,
            )
            .unwrap();
            assert_eq!(
                counts.counts(),
                truth.iter().map(|&v| u32::from(v)).collect::<Vec<_>>()
            );
            assert_eq!(scene.identity(), &identity);
        }
    }
}

/// Proves streamed AA-off bands preserve winding truth and exact full-surface RGBA.
/// # Panics
/// Panics on incorrect NonZero behavior, band differences, or altered hidden RGB.
#[test]
fn streamed_aa_off_fill_rules_match_exact_winding() {
    for points in paths() {
        for rule in [CanonicalFillRule::EvenOdd, CanonicalFillRule::NonZero] {
            let scene = scene(
                CanvasSpec {
                    width: 6.0,
                    height: 5.0,
                },
                GeometryOutput::CanonicalMarks(vec![mark(&points, rule)]),
            );
            let rgba = streamed(&scene, 6, 5, RasterAntialiasing::Off);
            assert_eq!(
                rgba.chunks_exact(4)
                    .map(|p| u8::from(p[3] != 0))
                    .collect::<Vec<_>>(),
                expected(&points, rule),
                "stream {rule:?}"
            );
            assert_eq!(
                rgba,
                rasterize_output(
                    &scene,
                    RasterBackground::Transparent,
                    Some(OutputRasterTarget::new(6, 5).unwrap()),
                    RasterAntialiasing::Off
                )
                .unwrap()
                .pixels()
            );
        }
    }
}

/// Preserves canonical AA-on and editable SVG winding meaning for integer-boundary fills.
/// # Panics
/// Panics if either winding rule loses interior coverage, alpha parity, or SVG fill metadata.
#[test]
fn aa_on_and_svg_preserve_canonical_winding_meaning() {
    for points in paths().into_iter().take(3) {
        for rule in [CanonicalFillRule::EvenOdd, CanonicalFillRule::NonZero] {
            let geometry = GeometryOutput::CanonicalMarks(vec![mark(&points, rule)]);
            let scene = scene(
                CanvasSpec {
                    width: 6.0,
                    height: 5.0,
                },
                geometry.clone(),
            );
            let full = rasterize_output(
                &scene,
                RasterBackground::Transparent,
                Some(OutputRasterTarget::new(6, 5).unwrap()),
                RasterAntialiasing::On,
            )
            .unwrap();
            assert_eq!(
                full.pixels()
                    .chunks_exact(4)
                    .map(|p| p[3])
                    .collect::<Vec<_>>(),
                expected(&points, rule)
                    .into_iter()
                    .map(|v| v * 255)
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                streamed(&scene, 6, 5, RasterAntialiasing::On),
                full.pixels()
            );
            // Translucency preserves the ordered editable path instead of using opaque joins.
            let layer =
                RenderLayer::new(ChannelId(1), true, color(1.0, 0.0, 0.0), 0.5, geometry).unwrap();
            let ordered = RenderScene::new(
                CanvasSpec {
                    width: 6.0,
                    height: 5.0,
                },
                "fill-rule".into(),
                "svg".into(),
                vec![layer],
            )
            .unwrap();
            let svg = write_svg(&ordered).unwrap();
            let fill = match rule {
                CanonicalFillRule::EvenOdd => "evenodd",
                CanonicalFillRule::NonZero => "nonzero",
            };
            assert!(svg.contains(&format!("fill-rule=\"{fill}\"")));
        }
    }
}

/// Builds an immutable stroke whose simple filled outline is independent of the centerline.
/// # Panics
/// Panics if finite fixture geometry or profile validation fails.
fn stroke() -> CanonicalStroke {
    let path = CurvePath::polyline(
        vec![
            Point2::new(1.0, 1.0),
            Point2::new(4.0, 1.0),
            Point2::new(4.0, 3.0),
            Point2::new(1.0, 3.0),
        ],
        PathClosure::Closed,
    )
    .unwrap();
    CanonicalStroke::new(
        StructuralPathInstanceId::guide_dimension(GuideDimensionId(1), 0, 0),
        None,
        CurvePath::line(Point2::new(1.0, 2.0), Point2::new(4.0, 2.0)).unwrap(),
        1.0,
        PathStrokeStyle::default(),
        vec![StrokeProfileSample {
            location: PathLocation::new(0, 0.0).unwrap(),
            center: Point2::new(1.0, 2.0),
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
            bounds: Some(Bounds {
                min: Point2::new(1.0, 1.0),
                max: Point2::new(4.0, 3.0),
            }),
        },
    )
    .unwrap()
}

/// Extracts serializer-owned IDs in document order without assigning a second identity authority.
fn ids(svg: &str) -> Vec<&str> {
    svg.split(" id=\"")
        .skip(1)
        .map(|part| part.split('"').next().unwrap())
        .collect()
}

/// Proves translucent multioutput connections and unmodeled strokes have deterministic unique IDs.
/// # Panics
/// Panics on duplicate/reordered IDs, lost geometry, or changed successful serialization.
#[test]
fn svg_ordered_outputs_have_unique_ids_including_unmodeled_strokes() {
    let geometry = GeometryOutput::CanonicalConnections(CanonicalConnections {
        strokes: vec![stroke()],
        isolated_points: vec![CanonicalMark::Circle {
            source_site_id: FamilySiteId {
                mechanism_id: PatternMechanismId(1),
                ordinal: 0,
            },
            center: Point2::new(2.0, 2.0),
            radius: 0.5,
            scope: SiteScope::Canvas,
            provenance: FamilySiteProvenance::Random {
                candidate_ordinal: 0,
                accepted_ordinal: 0,
                exclusion_neighbor_ordinal: None,
            },
            fill_rule: CanonicalFillRule::EvenOdd,
        }],
    });
    let outputs = (0..2)
        .map(|i| RenderOutputLayer::new(PatternOutputLayerId(i), geometry.clone(), None))
        .collect();
    let layer =
        RenderLayer::new_outputs(ChannelId(1), true, color(1.0, 0.0, 0.0), 0.5, outputs).unwrap();
    for modeled in [false, true] {
        let canvas = CanvasSpec {
            width: 6.0,
            height: 5.0,
        };
        let scene = if modeled {
            RenderScene::new_modeled(
                canvas,
                "id-fixture".into(),
                "connections".into(),
                HalftoneChannelModel::Rgb,
                vec![layer.clone()],
            )
        } else {
            RenderScene::new(
                canvas,
                "id-fixture".into(),
                "connections".into(),
                vec![layer.clone()],
            )
        }
        .unwrap();
        let svg = write_svg(&scene).unwrap();
        assert_eq!(svg, write_svg(&scene).unwrap());
        let all = ids(&svg);
        assert_eq!(
            all.len(),
            all.iter().collect::<HashSet<_>>().len(),
            "{all:?}"
        );
        assert!(svg.contains("channel-1-output-0-stroke-0"));
        assert!(svg.contains("channel-1-output-1-stroke-0"));
        if modeled {
            assert!(svg.contains("channel-1-output-0-mark-0"));
            assert!(svg.contains("channel-1-output-1-mark-0"));
        }
        assert_eq!(svg.matches("fill-rule=\"nonzero\"").count(), 2);
        assert_eq!(svg.matches("<circle").count(), 2);
    }
}

/// Proves omitted primitives keep original paint indices and ordered output ordinals in IDs.
/// # Panics
/// Panics on duplicate IDs, paint/index shifts, lost order, or nondeterministic markup.
#[test]
fn svg_sampled_outputs_preserve_omitted_indices_and_paints() {
    let empty = RenderOutputLayer::new(
        PatternOutputLayerId(0),
        GeometryOutput::CanonicalMarks(vec![]),
        Some(vec![]),
    );
    let mut outputs = vec![empty];
    for i in 1..3 {
        let hidden = mark(
            &[(20, 20), (21, 20), (21, 21), (20, 21)],
            CanonicalFillRule::EvenOdd,
        );
        let visible = mark(
            &[(1, 1), (3, 1), (3, 3), (1, 3)],
            CanonicalFillRule::NonZero,
        );
        outputs.push(RenderOutputLayer::new(
            PatternOutputLayerId(i),
            GeometryOutput::CanonicalMarks(vec![hidden, visible]),
            Some(vec![
                color(1.0, 0.0, 0.0),
                if i == 1 {
                    color(0.0, 1.0, 0.0)
                } else {
                    color(0.0, 0.0, 1.0)
                },
            ]),
        ));
    }
    let layer =
        RenderLayer::new_outputs(ChannelId(4), true, color(0.0, 0.0, 0.0), 0.5, outputs).unwrap();
    let scene = RenderScene::new_modeled(
        CanvasSpec {
            width: 6.0,
            height: 5.0,
        },
        "id-fixture".into(),
        "sampled".into(),
        HalftoneChannelModel::SourceColorAlpha,
        vec![layer],
    )
    .unwrap();
    let svg = write_svg(&scene).unwrap();
    assert_eq!(svg, write_svg(&scene).unwrap());
    let all = ids(&svg);
    assert_eq!(
        all.len(),
        all.iter().collect::<HashSet<_>>().len(),
        "{all:?}"
    );
    let paths = svg
        .lines()
        .filter(|l| l.starts_with("<path id="))
        .collect::<Vec<_>>();
    assert_eq!(paths.len(), 2);
    assert!(
        paths[0].contains("channel-4-output-1-mark-1") && paths[0].contains("fill=\"#00ff00\"")
    );
    assert!(
        paths[1].contains("channel-4-output-2-mark-1") && paths[1].contains("fill=\"#0000ff\"")
    );
    assert!(!svg.contains("output-1-mark-0"));
    for path in paths {
        assert!(path.contains("d=\"M1 1L3 1L3 3L1 3L1 1Z\""));
        assert!(path.contains("fill-rule=\"nonzero\"") && path.contains("fill-opacity=\"0.5\""));
    }
}

/// Rejects unrepresentable resolved transforms consistently before publishing sampled or AA pixels.
/// # Panics
/// Panics if a positive subnormal canvas silently yields output or stable consumer errors change.
#[test]
fn output_transform_rejects_subnormal_overflow_in_all_consumers() {
    let canvas = CanvasSpec {
        width: f64::from_bits(1),
        height: 1.0,
    };
    let geometry = GeometryOutput::CanonicalMarks(vec![]);
    let scene = scene(canvas.clone(), geometry.clone());
    let target = OutputRasterTarget::new(2, 2).unwrap();
    for aa in [RasterAntialiasing::Off, RasterAntialiasing::On] {
        assert_eq!(
            rasterize_output(&scene, RasterBackground::Transparent, Some(target), aa)
                .unwrap_err()
                .path(),
            "raster.output_transform"
        );
    }
    assert_eq!(
        rasterize_geometry_occupancy(
            &geometry,
            &canvas,
            target,
            RasterizationLimits::default(),
            &|| false
        )
        .unwrap_err()
        .path(),
        "raster.output_transform"
    );
    assert_eq!(
        rasterize_mark_multiplicity(
            &geometry,
            &canvas,
            target,
            RasterizationLimits::default(),
            &|| false
        )
        .unwrap_err()
        .path(),
        "raster.output_transform"
    );
    assert_eq!(
        rasterize_geometry_occupancy_window(
            &geometry,
            &canvas,
            OccupancyWindow::new(0.0, 0.0, canvas.width, canvas.height).unwrap(),
            target,
            RasterizationLimits::default(),
            &|| false
        )
        .unwrap_err()
        .path(),
        "raster.output_transform"
    );
    let mut sink = Vec::new();
    let error = write_png_streaming(
        &scene,
        StreamingRasterTarget::new(2, 2).unwrap(),
        Default::default(),
        RasterizationLimits::default(),
        &mut sink,
        &|| false,
        &|_, _| {},
    )
    .unwrap_err();
    assert_eq!(error.path(), "png.target");
    assert!(sink.is_empty());
}

/// Retains valid small/anisotropic output and the preview's extra resolved-edge checks.
/// # Panics
/// Panics if finite representable transforms gain an arbitrary minimum size or change pixel phase.
#[test]
fn output_transform_retains_small_anisotropic_targets_and_preview_checks() {
    for canvas in [
        CanvasSpec {
            width: 1.0e-200,
            height: 1.0e-200,
        },
        CanvasSpec {
            width: 6.0,
            height: 5.0,
        },
    ] {
        let scene = scene(canvas.clone(), GeometryOutput::CanonicalMarks(vec![]));
        for aa in [RasterAntialiasing::Off, RasterAntialiasing::On] {
            let full = rasterize_output(
                &scene,
                RasterBackground::Transparent,
                Some(OutputRasterTarget::new(12, 5).unwrap()),
                aa,
            )
            .unwrap();
            assert!(full.pixels().iter().all(|v| *v == 0));
            assert_eq!(streamed(&scene, 12, 5, aa), full.pixels());
        }
    }
    let geometry = GeometryOutput::CanonicalMarks(vec![mark(
        &[(0, 0), (3, 0), (3, 5), (0, 5)],
        CanonicalFillRule::NonZero,
    )]);
    let ordinary = scene(
        CanvasSpec {
            width: 6.0,
            height: 5.0,
        },
        geometry,
    );
    let full = rasterize_output(
        &ordinary,
        RasterBackground::Transparent,
        Some(OutputRasterTarget::new(12, 5).unwrap()),
        RasterAntialiasing::Off,
    )
    .unwrap();
    for row in full.pixels().chunks_exact(12 * 4) {
        for (x, p) in row.chunks_exact(4).enumerate() {
            assert_eq!(p[3], if x < 6 { 255 } else { 0 });
        }
    }
    let scene = scene(
        CanvasSpec {
            width: f64::MAX,
            height: 1.0,
        },
        GeometryOutput::CanonicalMarks(vec![]),
    );
    let invalid = PreviewRasterTarget::for_viewport(2, 2, -f64::MAX, 0.0, 1.0).unwrap();
    assert_eq!(
        rasterize_preview(&scene, invalid).unwrap_err().path(),
        "preview.target"
    );
}
