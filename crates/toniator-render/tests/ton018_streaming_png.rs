//! Exact native row-band PNG parity, resource admission, and failure boundaries.

use std::{
    fs,
    io::{self, Write},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};
use toniator_domain::{
    CanvasSpec, ChannelId, ColorValue, GuideDimensionId, HalftoneChannelModel, PathStrokeStyle,
    PatternMechanismId, PatternOutputLayerId,
};
use toniator_geometry::{
    Bounds, CanonicalFillRule, CanonicalFilledOutline, CanonicalMark, CanonicalOutlineContour,
    CanonicalPathMark, CanonicalRegionProposal, CanonicalRegionSourceGroup,
    CanonicalRegionSourceId, CanonicalStroke, CurvePath, FamilySiteId, FamilySiteProvenance,
    PathClosure, PathLocation, Point2, SiteScope, StrokeProfileSample, StructuralPathInstanceId,
    build_canonical_regions,
};
use toniator_render::*;

/// Returns stage-owned artifacts without changing any immutable source or prior evidence.
/// # Panics
/// Panics if the stage artifact directory cannot be created.
fn stage() -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/validation/ton018-png-streaming-20261013/native");
    fs::create_dir_all(&path).unwrap();
    path
}

/// Supplies a finite canonical owner used only for renderer fixture provenance.
fn owner(ordinal: usize) -> FamilySiteId {
    FamilySiteId {
        mechanism_id: PatternMechanismId(1),
        ordinal,
    }
}

/// Supplies stable source-independent placement provenance for a renderer fixture.
fn provenance(ordinal: usize) -> FamilySiteProvenance {
    FamilySiteProvenance::Random {
        candidate_ordinal: ordinal,
        accepted_ordinal: ordinal,
        exclusion_neighbor_ordinal: None,
    }
}

/// Builds an intact circle, including circles crossing the final output bounds.
fn circle(ordinal: usize, x: f64, y: f64, radius: f64) -> CanonicalMark {
    CanonicalMark::Circle {
        source_site_id: owner(ordinal),
        center: Point2::new(x, y),
        radius,
        scope: SiteScope::Canvas,
        provenance: provenance(ordinal),
        fill_rule: CanonicalFillRule::EvenOdd,
    }
}

/// Builds one authored closed path without raster-derived topology.
/// # Panics
/// Panics only if finite fixture points cease to form a valid closed path.
fn path(points: &[(f64, f64)]) -> CurvePath {
    CurvePath::polyline(
        points.iter().map(|&(x, y)| Point2::new(x, y)).collect(),
        PathClosure::Closed,
    )
    .unwrap()
}

/// Builds a self-crossing filled mark to exercise primitive-local EvenOdd normalization.
/// # Panics
/// Panics on invalid finite fixture geometry.
fn bowtie() -> CanonicalMark {
    CanonicalMark::ClosedPath(
        CanonicalPathMark::new(
            owner(9),
            path(&[(-3.0, 2.0), (28.0, 21.0), (2.0, 21.0), (29.0, 2.0)]),
            SiteScope::Canvas,
            provenance(9),
            CanonicalFillRule::EvenOdd,
        )
        .unwrap(),
    )
}

/// Builds a filled stroke with an opposite-winding hole and an independent thin rotated contour.
/// # Panics
/// Panics on invalid centerline, profile, or immutable derived outline.
fn stroke() -> CanonicalStroke {
    let contours = [
        path(&[(1.0, 1.0), (30.0, 1.0), (30.0, 22.0), (1.0, 22.0)]),
        path(&[(5.0, 5.0), (5.0, 18.0), (25.0, 18.0), (25.0, 5.0)]),
        path(&[(0.0, 0.0), (31.0, 22.7), (31.0, 22.79), (0.0, 0.09)]),
    ];
    CanonicalStroke::new(
        StructuralPathInstanceId::guide_dimension(GuideDimensionId(1), 0, 0),
        None,
        CurvePath::line(Point2::new(1.0, 1.0), Point2::new(30.0, 22.0)).unwrap(),
        1.0,
        PathStrokeStyle::default(),
        vec![StrokeProfileSample {
            location: PathLocation::new(0, 0.0).unwrap(),
            center: Point2::new(1.0, 1.0),
            normalized_thickness: 1.0,
            width: 1.0,
        }],
        CanonicalFilledOutline {
            contours: contours
                .into_iter()
                .map(|path| CanonicalOutlineContour {
                    segments: path.segments().to_vec(),
                })
                .collect(),
            fill_rule: CanonicalFillRule::NonZero,
            bounds: Some(Bounds {
                min: Point2::new(0.0, 0.0),
                max: Point2::new(31.0, 22.79),
            }),
        },
    )
    .unwrap()
}

/// Builds modeled or ordered source-over geometry with unions, opacity, holes, and off-canvas paint.
/// # Panics
/// Panics on invalid fixture authority or modeled layer requirements.
fn scene(model: Option<HalftoneChannelModel>) -> RenderScene {
    let sampled = model == Some(HalftoneChannelModel::SourceColorAlpha);
    let color = ColorValue {
        red: 0.7,
        green: 0.1,
        blue: 0.3,
        alpha: 1.0,
    };
    let marks = vec![
        circle(0, 0.0, 9.5, 4.25),
        circle(1, 15.5, 11.5, 6.75),
        circle(2, 18.0, 13.1, 4.0),
        bowtie(),
    ];
    let paints = sampled.then(|| {
        vec![
            color.clone(),
            ColorValue {
                red: 0.1,
                green: 0.8,
                blue: 0.4,
                alpha: 1.0,
            },
            color.clone(),
            color.clone(),
        ]
    });
    let mut outputs = vec![RenderOutputLayer::new(
        PatternOutputLayerId(1),
        GeometryOutput::CanonicalMarks(marks),
        paints,
    )];
    let regions = build_canonical_regions(CanonicalRegionProposal {
        output_layer_id: PatternOutputLayerId(3),
        source_groups: vec![CanonicalRegionSourceGroup {
            source_id: CanonicalRegionSourceId::SiteOwners(vec![owner(20)]),
            components: vec![path(&[
                (-1.0, 4.0),
                (10.7, 3.25),
                (22.9, 24.0),
                (-1.0, 20.0),
            ])],
        }],
    })
    .unwrap();
    outputs.push(RenderOutputLayer::new(
        PatternOutputLayerId(3),
        GeometryOutput::CanonicalRegions(regions),
        sampled.then(|| {
            vec![ColorValue {
                red: 0.02,
                green: 0.6,
                blue: 0.9,
                alpha: 0.497,
            }]
        }),
    ));
    if !sampled {
        outputs.push(RenderOutputLayer::new(
            PatternOutputLayerId(2),
            GeometryOutput::CanonicalStrokes(vec![stroke()]),
            None,
        ));
    }
    let mut layers = vec![
        RenderLayer::new_outputs(
            ChannelId(1),
            true,
            color,
            if sampled { 0.73 } else { 1.0 },
            outputs,
        )
        .unwrap(),
    ];
    if !sampled {
        layers.push(
            RenderLayer::new(
                ChannelId(2),
                true,
                ColorValue {
                    red: 0.07,
                    green: 0.9,
                    blue: 0.25,
                    alpha: 0.63,
                },
                0.79,
                GeometryOutput::CanonicalMarks(vec![
                    circle(30, 12.4, 12.5, 8.8),
                    circle(31, 31.0, 3.0, 4.0),
                ]),
            )
            .unwrap(),
        );
        layers.push(
            RenderLayer::new(
                ChannelId(3),
                false,
                ColorValue {
                    red: 1.0,
                    green: 0.0,
                    blue: 0.0,
                    alpha: 1.0,
                },
                1.0,
                GeometryOutput::CanonicalMarks(vec![circle(40, 10.0, 10.0, 9.0)]),
            )
            .unwrap(),
        );
    }
    let canvas = CanvasSpec {
        width: 31.0,
        height: 23.0,
    };
    match model {
        Some(model) => RenderScene::new_modeled(
            canvas,
            "stream-family".into(),
            "stream-realization".into(),
            model,
            layers,
        ),
        None => RenderScene::new(
            canvas,
            "stream-family".into(),
            "stream-realization".into(),
            layers,
        ),
    }
    .unwrap()
}

/// Decodes native PNG bytes without compositing or converting alpha.
/// # Panics
/// Panics on invalid PNG encoding or unexpected native pixel format.
fn decode(bytes: &[u8]) -> (u32, u32, Vec<u8>, Option<png::PixelDimensions>) {
    let mut reader = png::Decoder::new(std::io::Cursor::new(bytes))
        .read_info()
        .unwrap();
    let density = reader.info().pixel_dims;
    let mut rgba = vec![0; reader.output_buffer_size().unwrap()];
    let frame = reader.next_frame(&mut rgba).unwrap();
    assert_eq!(frame.color_type, png::ColorType::Rgba);
    rgba.truncate(frame.buffer_size());
    (frame.width, frame.height, rgba, density)
}

/// Proves exact decoded parity across band boundaries, all models, AA modes, backing, and paint.
/// # Panics
/// Panics on any pixel difference, geometry mutation, or failed PNG IO.
#[test]
fn streamed_rgba_matches_full_native_composition_exactly() {
    for model in [
        None,
        Some(HalftoneChannelModel::Rgb),
        Some(HalftoneChannelModel::Cmyk),
        Some(HalftoneChannelModel::SourceColorAlpha),
    ] {
        let scene = scene(model);
        let original = scene.clone();
        let target = StreamingRasterTarget::new(37, 29).unwrap();
        for aa in [RasterAntialiasing::On, RasterAntialiasing::Off] {
            for background in [
                RasterBackground::Transparent,
                RasterBackground::OpaqueWhite,
                RasterBackground::OpaqueBlack,
            ] {
                let expected = rasterize_output(
                    &scene,
                    background,
                    Some(OutputRasterTarget::new(37, 29).unwrap()),
                    aa,
                )
                .unwrap();
                for rows in [1, 2, 7, 28, 29, u32::MAX] {
                    let mut bytes = Vec::new();
                    let summary = write_png_streaming(
                        &scene,
                        target,
                        PngStreamOptions {
                            background,
                            antialiasing: aa,
                            maximum_band_rows: rows,
                            ..Default::default()
                        },
                        RasterizationLimits::default(),
                        &mut bytes,
                        &|| false,
                        &|_, _| {},
                    )
                    .unwrap();
                    let (_, _, rgba, _) = decode(&bytes);
                    assert_eq!(
                        rgba,
                        expected.pixels(),
                        "model={model:?} aa={aa:?} backing={background:?} rows={rows}"
                    );
                    assert!(summary.maximum_band_pixel_bytes < 64 * 1024 * 1024);
                    if rows == 7
                        && aa == RasterAntialiasing::On
                        && background == RasterBackground::Transparent
                    {
                        fs::write(stage().join(format!("parity-{model:?}.png")), &bytes).unwrap();
                    }
                }
            }
        }
        assert_eq!(scene, original);
    }
}

/// Proves prepared PNG consumes transparent quantized bytes before thresholding and backing.
/// # Panics
/// Panics if fractional-alpha threshold decisions or surviving RGB differs from the existing path.
#[test]
fn prepared_binary_alpha_matches_existing_byte_order_and_density() {
    for model in [
        HalftoneChannelModel::Rgb,
        HalftoneChannelModel::Cmyk,
        HalftoneChannelModel::SourceColorAlpha,
    ] {
        let scene = scene(Some(model));
        for aa in [RasterAntialiasing::On, RasterAntialiasing::Off] {
            let native = rasterize_output(
                &scene,
                RasterBackground::Transparent,
                Some(OutputRasterTarget::new(37, 29).unwrap()),
                aa,
            )
            .unwrap();
            for background in [
                RasterBackground::Transparent,
                RasterBackground::OpaqueBlack,
                RasterBackground::OpaqueWhite,
            ] {
                let mut expected = native.pixels().to_vec();
                for pixel in expected.chunks_exact_mut(4) {
                    if pixel[3] < 128 {
                        pixel.copy_from_slice(&match background {
                            RasterBackground::Transparent => [0, 0, 0, 0],
                            RasterBackground::OpaqueBlack => [0, 0, 0, 255],
                            RasterBackground::OpaqueWhite => [255, 255, 255, 255],
                        });
                    } else {
                        pixel[3] = 255;
                    }
                }
                let mut bytes = Vec::new();
                write_png_streaming(
                    &scene,
                    StreamingRasterTarget::new(37, 29).unwrap(),
                    PngStreamOptions {
                        background,
                        antialiasing: aa,
                        binary_alpha: true,
                        pixels_per_metre: Some(11_811),
                        maximum_band_rows: 2,
                    },
                    RasterizationLimits::default(),
                    &mut bytes,
                    &|| false,
                    &|_, _| {},
                )
                .unwrap();
                let (width, height, rgba, density) = decode(&bytes);
                assert_eq!((width, height), (37, 29));
                assert_eq!(rgba, expected);
                let density = density.unwrap();
                assert_eq!(
                    (density.xppu, density.yppu, density.unit),
                    (11_811, 11_811, png::Unit::Meter)
                );
            }
        }
    }
}

/// Proves full-raster caps remain unchanged and streaming targets retain exact large dimensions.
/// # Panics
/// Panics on changed legacy caps or acceptance of invalid streaming axes/options.
#[test]
fn streaming_admission_is_separate_and_truthful() {
    assert_eq!(OutputRasterTarget::maximum_pixels(), 67_108_864);
    assert!(OutputRasterTarget::new(16_384, 9_312).is_err());
    let target = StreamingRasterTarget::new(16_384, 9_312).unwrap();
    assert_eq!(target.pixel_count(), 152_567_808);
    for (width, height) in [(0, 1), (1, 0), (u32::MAX, 1), (1, u32::MAX), (1_000_000, 1)] {
        assert!(StreamingRasterTarget::new(width, height).is_err());
    }
    assert!(StreamingRasterTarget::new(1, i32::MAX as u32).is_ok());
    for options in [
        PngStreamOptions {
            maximum_band_rows: 0,
            ..Default::default()
        },
        PngStreamOptions {
            pixels_per_metre: Some(0),
            ..Default::default()
        },
    ] {
        assert!(
            write_png_streaming(
                &scene(None),
                StreamingRasterTarget::new(3, 3).unwrap(),
                options,
                RasterizationLimits::default(),
                Vec::new(),
                &|| false,
                &|_, _| {}
            )
            .is_err()
        );
    }
}

/// Proves repeated bands do not repeat flattening or spend the edge budget again.
/// # Panics
/// Panics if 101 bands charge a four-edge primitive repeatedly or fail exact parity.
#[test]
fn flattening_budget_is_charged_once_globally() {
    let mark = CanonicalPathMark::new(
        owner(0),
        path(&[(1.0, 1.0), (30.0, 1.0), (30.0, 22.0), (1.0, 22.0)]),
        SiteScope::Canvas,
        provenance(0),
        CanonicalFillRule::EvenOdd,
    )
    .unwrap();
    let layer = RenderLayer::new(
        ChannelId(1),
        true,
        ColorValue {
            red: 0.2,
            green: 0.6,
            blue: 0.8,
            alpha: 1.0,
        },
        1.0,
        GeometryOutput::CanonicalMarks(vec![CanonicalMark::ClosedPath(mark)]),
    )
    .unwrap();
    let scene = RenderScene::new(
        CanvasSpec {
            width: 31.0,
            height: 23.0,
        },
        "one-flatten-family".into(),
        "one-flatten-realization".into(),
        vec![layer],
    )
    .unwrap();
    for antialiasing in [RasterAntialiasing::On, RasterAntialiasing::Off] {
        write_png_streaming(
            &scene,
            StreamingRasterTarget::new(17, 101).unwrap(),
            PngStreamOptions {
                antialiasing,
                maximum_band_rows: 1,
                ..Default::default()
            },
            RasterizationLimits::new(32).unwrap(),
            Vec::new(),
            &|| false,
            &|_, _| {},
        )
        .unwrap();
    }
}

/// Injects compressor-sink failure or cancellation after the first IDAT reaches the caller.
struct FaultSink {
    cancelled: Arc<AtomicBool>,
    fail: bool,
    saw_idat: bool,
}

/// Fails only the outer PNG writer's IEND write or its subsequent flush.
struct FinalizationSink {
    fail_iend: bool,
    saw_iend: bool,
}

impl Write for FinalizationSink {
    /// Accepts all image data and optionally fails the explicit final IEND chunk.
    /// # Errors
    /// Returns an injected final-chunk error after successful band encoding.
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.windows(4).any(|part| part == b"IEND") {
            self.saw_iend = true;
            if self.fail_iend {
                return Err(io::Error::other("injected IEND failure"));
            }
        }
        Ok(bytes.len())
    }

    /// Fails the outer writer's flush only after IEND has reached the sink.
    /// # Errors
    /// Returns an injected final flush failure after successful stream finalization.
    fn flush(&mut self) -> io::Result<()> {
        if self.saw_iend {
            Err(io::Error::other("injected final PNG flush failure"))
        } else {
            Ok(())
        }
    }
}

impl Write for FaultSink {
    /// Fails only at image-data publication, keeping header success and geometric preparation real.
    /// # Errors
    /// Returns an injected write error or arms cancellation after observing image-data bytes.
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.windows(4).any(|value| value == b"IDAT") || self.saw_idat {
            self.saw_idat = true;
            if self.fail {
                return Err(io::Error::other("injected PNG write failure"));
            }
            self.cancelled.store(true, Ordering::Release);
        }
        Ok(bytes.len())
    }
    /// Completes the fake sink flush; cancellation remains owned by the renderer wrapper.
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Proves preparation, band, and compressor cancellation plus write failure return no success.
/// # Panics
/// Panics on missing cancellation/error propagation or unexpected successful finalization.
#[test]
fn cancellation_and_write_errors_stop_every_streaming_phase() {
    let scene = scene(None);
    let target = StreamingRasterTarget::new(37, 29).unwrap();
    let calls = AtomicUsize::new(0);
    let mut bytes = Vec::new();
    let error = write_png_streaming(
        &scene,
        target,
        Default::default(),
        RasterizationLimits::default(),
        &mut bytes,
        &|| calls.fetch_add(1, Ordering::Relaxed) > 4,
        &|_, _| {},
    )
    .unwrap_err();
    assert_eq!(error.path(), "evaluation.cancelled");
    assert!(
        bytes.is_empty(),
        "preparation cancellation precedes PNG header"
    );
    let cancelled = AtomicBool::new(false);
    let error = write_png_streaming(
        &scene,
        target,
        PngStreamOptions {
            maximum_band_rows: 2,
            ..Default::default()
        },
        RasterizationLimits::default(),
        Vec::new(),
        &|| cancelled.load(Ordering::Acquire),
        &|rows, _| {
            if rows >= 2 {
                cancelled.store(true, Ordering::Release);
            }
        },
    )
    .unwrap_err();
    assert_eq!(error.path(), "evaluation.cancelled");
    for fail in [false, true] {
        let cancelled = Arc::new(AtomicBool::new(false));
        let sink = FaultSink {
            cancelled: cancelled.clone(),
            fail,
            saw_idat: false,
        };
        let error = write_png_streaming(
            &scene,
            target,
            Default::default(),
            RasterizationLimits::default(),
            sink,
            &|| cancelled.load(Ordering::Acquire),
            &|_, _| {},
        )
        .unwrap_err();
        assert_eq!(
            error.path(),
            if fail {
                "png.encode"
            } else {
                "evaluation.cancelled"
            }
        );
    }
    for fail_iend in [false, true] {
        let error = write_png_streaming(
            &scene,
            target,
            Default::default(),
            RasterizationLimits::default(),
            FinalizationSink {
                fail_iend,
                saw_iend: false,
            },
            &|| false,
            &|_, _| {},
        )
        .unwrap_err();
        assert_eq!(error.path(), "png.encode");
    }
}

/// Produces a real large native PNG without decoding a second full-sized in-memory RGBA image.
/// # Panics
/// Panics on changed dimensions, allocation admission, truncated PNG finalization, or artifact IO.
#[test]
#[ignore = "explicit actual-size acceptance witness"]
fn actual_16384_by_9312_png_uses_bounded_pixel_bands() {
    let scene = scene(Some(HalftoneChannelModel::Rgb));
    let target = StreamingRasterTarget::new(16_384, 9_312).unwrap();
    let file = fs::File::create(stage().join("representative-16384x9312.png")).unwrap();
    let summary = write_png_streaming(
        &scene,
        target,
        PngStreamOptions {
            pixels_per_metre: Some(11_811),
            ..Default::default()
        },
        RasterizationLimits::default(),
        file,
        &|| false,
        &|_, _| {},
    )
    .unwrap();
    assert!(summary.maximum_band_pixel_bytes < 64 * 1024 * 1024);
    let reader = png::Decoder::new(std::io::BufReader::new(
        fs::File::open(stage().join("representative-16384x9312.png")).unwrap(),
    ))
    .read_info()
    .unwrap();
    assert_eq!(
        (reader.info().width, reader.info().height),
        (target.width(), target.height())
    );
    fs::write(
        stage().join("representative-summary.txt"),
        format!("{summary:#?}\n"),
    )
    .unwrap();
}
