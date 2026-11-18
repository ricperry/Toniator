use super::*;
use crate::{RenderLayer, RenderOutputLayer};
use toniator_domain::{
    CanvasSpec, ChannelId, ColorValue, PatternMechanismId, PatternOutputLayerId,
};
use toniator_geometry::{CanonicalFillRule, FamilySiteId, FamilySiteProvenance, SiteScope};

/// Builds one canonical circle with stable site identity and unchanged authored radius.
fn circle(ordinal: usize, x: f64, y: f64, radius: f64) -> CanonicalMark {
    CanonicalMark::Circle {
        source_site_id: FamilySiteId {
            mechanism_id: PatternMechanismId(1),
            ordinal,
        },
        center: Point2::new(x, y),
        radius,
        scope: SiteScope::Canvas,
        provenance: FamilySiteProvenance::Random {
            candidate_ordinal: ordinal,
            accepted_ordinal: ordinal,
            exclusion_neighbor_ordinal: None,
        },
        fill_rule: CanonicalFillRule::NonZero,
    }
}
/// Builds a small current modeled scene without any pattern-name or source-specific consumer branch.
/// # Panics
/// Panics if the canonical fixture violates the current RenderScene authority.
fn scene(model: HalftoneChannelModel, marks: Vec<CanonicalMark>, opacity: f64) -> Arc<RenderScene> {
    Arc::new(
        RenderScene::new_modeled(
            CanvasSpec {
                width: 32.0,
                height: 32.0,
            },
            "fast-test-family".into(),
            "fast-test-realization".into(),
            model,
            vec![
                RenderLayer::new(
                    ChannelId(1),
                    true,
                    ColorValue {
                        red: 0.3,
                        green: 0.6,
                        blue: 0.9,
                        alpha: 1.0,
                    },
                    opacity,
                    GeometryOutput::CanonicalMarks(marks),
                )
                .unwrap(),
            ],
        )
        .unwrap(),
    )
}

/// Proves every complete-scene capability refusal occurs before native encoding or device initialization.
/// Invisible layers and empty sampled-paint metadata are checked as carefully as visible geometry.
/// # Panics
/// Panics if unsupported canonical structure is silently dropped or partially admitted.
#[test]
fn fast_preview_capability_matrix_is_whole_scene() {
    let base = scene(
        HalftoneChannelModel::Rgb,
        vec![circle(0, 16.0, 16.0, 2.0)],
        1.0,
    );
    for model in [HalftoneChannelModel::Rgb, HalftoneChannelModel::Cmyk] {
        let mut value = base.as_ref().clone();
        value.model = Some(model);
        assert!(validate_scene(&value).is_ok());
        value.layers[0].outputs[0].geometry = Arc::new(GeometryOutput::CircularMarks(Vec::new()));
        assert!(validate_scene(&value).is_ok());
        value.layers[0].outputs[0].geometry = Arc::new(GeometryOutput::CircularMarks(vec![
            toniator_geometry::CanonicalCircleMark {
                source_site_id: toniator_geometry::SiteId {
                    first_dimension_id: 1,
                    first_index: 0,
                    second_dimension_id: 2,
                    second_index: 0,
                },
                center: Point2::new(-1.0, 3.0),
                radius: 2.0,
                scope: SiteScope::Guard,
                provenance: toniator_geometry::GuideIntersectionProvenance {
                    contributors: Vec::new(),
                },
            },
        ]));
        assert!(
            FastPreviewPlan::new(
                Arc::new(value),
                1.0,
                RasterBackground::Transparent,
                FastPreviewLimits::default(),
                &|| false
            )
            .unwrap()
            .reservation()
            .path_elements
                > 0
        );
    }
    for model in [None, Some(HalftoneChannelModel::SourceColorAlpha)] {
        let mut value = base.as_ref().clone();
        value.model = model;
        assert!(matches!(
            validate_scene(&value),
            Err(FastPreviewError::UnsupportedScene(_))
        ));
    }
    let closed = toniator_geometry::CurvePath::new(
        vec![toniator_geometry::CurveSegment::CubicBezier(
            toniator_geometry::CubicBezierSegment::new(
                Point2::new(1.0, 1.0),
                Point2::new(2.0, 4.0),
                Point2::new(4.0, 2.0),
                Point2::new(1.0, 1.0),
            )
            .unwrap(),
        )],
        toniator_geometry::PathClosure::Closed,
    )
    .unwrap();
    let noncircle = CanonicalMark::ClosedPath(
        toniator_geometry::CanonicalPathMark::new(
            FamilySiteId {
                mechanism_id: PatternMechanismId(2),
                ordinal: 0,
            },
            closed,
            SiteScope::Canvas,
            FamilySiteProvenance::Random {
                candidate_ordinal: 0,
                accepted_ordinal: 0,
                exclusion_neighbor_ordinal: None,
            },
            CanonicalFillRule::NonZero,
        )
        .unwrap(),
    );
    let unsupported = [
        GeometryOutput::CanonicalMarks(vec![noncircle]),
        GeometryOutput::CanonicalRegions(toniator_geometry::CanonicalRegionSet::empty()),
        GeometryOutput::CanonicalStrokes(Vec::new()),
        GeometryOutput::CanonicalConnections(toniator_geometry::CanonicalConnections {
            strokes: Vec::new(),
            isolated_points: Vec::new(),
        }),
    ];
    for geometry in unsupported {
        let mut value = base.as_ref().clone();
        value.layers[0].visible = false;
        value.layers[0].outputs[0].geometry = Arc::new(geometry);
        assert!(matches!(
            FastPreviewPlan::new(
                Arc::new(value),
                1.0,
                RasterBackground::Transparent,
                FastPreviewLimits::default(),
                &|| false
            ),
            Err(FastPreviewError::UnsupportedScene(_))
        ));
    }
    for count in [0, 2] {
        let mut value = base.as_ref().clone();
        value.layers[0].outputs = (0..count)
            .map(|i| {
                RenderOutputLayer::new(
                    PatternOutputLayerId(i),
                    GeometryOutput::CanonicalMarks(Vec::new()),
                    None,
                )
            })
            .collect();
        assert!(validate_scene(&value).is_err());
    }
    let mut value = base.as_ref().clone();
    value.layers[0].visible = false;
    value.layers[0].outputs[0].primitive_paints = Some(Arc::new(Vec::new()));
    assert!(validate_scene(&value).is_err());
    let mut value = base.as_ref().clone();
    let mut invisible = value.layers[0].clone();
    invisible.channel_id = ChannelId(2);
    invisible.visible = false;
    invisible.outputs[0].primitive_paints = Some(Arc::new(Vec::new()));
    value.layers.push(invisible);
    assert!(validate_scene(&value).is_err());
}

/// Proves admission preserves zero/off-canvas circle populations and rejects invalid/over-budget work.
/// # Panics
/// Panics if optional-consumer resource refusal mutates canonical geometry or admits unbounded work.
#[test]
fn fast_preview_counts_guards_and_refuses_before_allocation() {
    let canonical = scene(
        HalftoneChannelModel::Cmyk,
        vec![circle(0, -1.0, 16.0, 2.0), circle(1, 16.0, 16.0, 0.0)],
        1.0,
    );
    let identity = canonical.identity().scene_fingerprint().to_owned();
    let plan = FastPreviewPlan::new(
        Arc::clone(&canonical),
        1.0,
        RasterBackground::Transparent,
        FastPreviewLimits::default(),
        &|| false,
    )
    .unwrap();
    assert!(plan.reservation.path_elements > 0);
    assert!(plan.reservation.gpu_bytes > 512 * MIB);
    assert_eq!(canonical.circular_mark_count(), 2);
    assert_eq!(canonical.identity().scene_fingerprint(), identity);
    assert!(matches!(
        FastPreviewPlan::new(
            Arc::clone(&canonical),
            0.0,
            RasterBackground::Transparent,
            FastPreviewLimits::default(),
            &|| false
        ),
        Err(FastPreviewError::Limits(_))
    ));
    assert!(matches!(
        FastPreviewPlan::new(
            Arc::clone(&canonical),
            1.0,
            RasterBackground::Transparent,
            FastPreviewLimits {
                available_payload_bytes: 1,
                gpu_reservation_bytes: 1
            },
            &|| false
        ),
        Err(FastPreviewError::Limits(_))
    ));
    assert!(matches!(
        FastPreviewPlan::new(
            canonical,
            1.0,
            RasterBackground::Transparent,
            FastPreviewLimits::default(),
            &|| true
        ),
        Err(FastPreviewError::Cancelled)
    ));
    let huge_center = scene(
        HalftoneChannelModel::Rgb,
        vec![circle(0, 1.0e300, 1.0, 1.0)],
        1.0,
    );
    assert!(matches!(
        FastPreviewPlan::new(
            huge_center,
            1.0,
            RasterBackground::Transparent,
            FastPreviewLimits::default(),
            &|| false
        ),
        Err(FastPreviewError::Limits(_))
    ));
    let huge = scene(
        HalftoneChannelModel::Rgb,
        vec![circle(0, 16.0, 16.0, 1.0e100)],
        1.0,
    );
    assert!(matches!(
        FastPreviewPlan::new(
            huge,
            1.0,
            RasterBackground::Transparent,
            FastPreviewLimits::default(),
            &|| false
        ),
        Err(FastPreviewError::Limits(_))
    ));
}

/// Proves Fit reuses the existing target-transform authority and final background policy.
/// # Panics
/// Panics if the consumer invents a different scale or letterbox origin.
#[test]
fn fast_preview_fit_uses_renderer_transform() {
    let scene = scene(HalftoneChannelModel::Rgb, Vec::new(), 1.0);
    let target = PreviewRasterTarget::new(96, 64)
        .unwrap()
        .with_background(RasterBackground::OpaqueBlack);
    let expected = crate::PreviewTransform::for_scene(&scene, target);
    let plan = FastPreviewPlan::for_target(scene, target, FastPreviewLimits::default(), &|| false)
        .unwrap();
    assert_eq!(plan.frame.scale.to_bits(), expected.scale.to_bits());
    assert_eq!(plan.offset, (expected.offset_x, expected.offset_y));
    assert_eq!(plan.frame.background, RasterBackground::OpaqueBlack);
}

/// Proves final backdrops apply to premultiplied LINEAR pixels before unassociation/transfer.
/// # Panics
/// Panics if hidden RGB leaks through zero alpha or the backdrop is applied in sRGB/native blending.
#[test]
fn fast_preview_linear_backdrop_and_hidden_rgb() {
    assert_eq!(
        linear_pixel_to_srgba(&[255, 255, 255, 0], RasterBackground::Transparent),
        [0, 0, 0, 0]
    );
    assert_eq!(
        linear_pixel_to_srgba(&[0, 0, 0, 0], RasterBackground::OpaqueWhite),
        [255, 255, 255, 255]
    );
    assert_eq!(
        linear_pixel_to_srgba(&[0, 0, 0, 0], RasterBackground::OpaqueBlack),
        [0, 0, 0, 255]
    );
    let black = linear_pixel_to_srgba(&[64, 0, 0, 128], RasterBackground::OpaqueBlack);
    let white = linear_pixel_to_srgba(&[64, 0, 0, 128], RasterBackground::OpaqueWhite);
    assert_eq!(black, [137, 0, 0, 255]);
    assert_eq!(white, [224, 187, 187, 255]);
}

/// Checks conservative projection bounds against the actual pinned public line/tile/strip generator.
/// It includes overlapping opaque contours, translucent draws, zero/off-canvas circles and phases.
/// # Panics
/// Panics if native line/tile/alpha/strip counts exceed the pre-encoding input-size reservations.
#[test]
fn fast_preview_resource_bounds_cover_pinned_native_generation() {
    use vello_common::{
        fearless_simd::Level,
        geometry::RectU16,
        strip_generator::{GenerationMode, StripGenerator, StripStorage},
    };
    let level = Level::try_detect().unwrap_or(Level::baseline());
    for opacity in [1.0, 0.5] {
        for scale in [1.0, 4.0] {
            let canonical = scene(
                HalftoneChannelModel::Rgb,
                vec![
                    circle(0, 16.25, 16.5, 8.0),
                    circle(1, 16.25, 16.5, 8.0),
                    circle(2, -1.0, 3.0, 2.0),
                    circle(3, 0.0, 0.0, 0.0),
                ],
                opacity,
            );
            let plan = FastPreviewPlan::new(
                Arc::clone(&canonical),
                scale,
                RasterBackground::Transparent,
                FastPreviewLimits::default(),
                &|| false,
            )
            .unwrap();
            let mut generator =
                StripGenerator::new(plan.frame.width as u16, plan.frame.height as u16, level);
            let mut storage = StripStorage::new(GenerationMode::Append);
            let mut compound = Vec::new();
            compound.reserve_exact(plan.per_channel_elements[0]);
            let mut bound_tiles = 0_u64;
            visit_circles(&canonical.layers()[0], |center, radius| {
                let (count, tile_bound) = projected_path(center, radius, scale, (0.0, 0.0), None)?;
                bound_tiles += tile_bound;
                let mut path = Vec::with_capacity(count as usize);
                projected_path(center, radius, scale, (0.0, 0.0), Some(&mut path))?;
                if opacity == 1.0 {
                    compound.extend(path);
                } else {
                    generator.generate_filled_path(
                        path,
                        Fill::NonZero,
                        Affine::scale(scale),
                        None,
                        &mut storage,
                        None,
                    );
                }
                Ok(())
            })
            .unwrap();
            if opacity == 1.0 {
                let mut lines = Vec::new();
                vello_common::flatten::fill(
                    level,
                    compound.iter().copied(),
                    Affine::scale(scale),
                    &mut lines,
                    &mut vello_common::flatten::FlattenCtx::default(),
                    RectU16::new(0, 0, plan.frame.width as u16, plan.frame.height as u16),
                );
                let mut tiles = vello_common::tile::Tiles::new(
                    level,
                    plan.frame.width as u16,
                    plan.frame.height as u16,
                );
                tiles.make_tiles_analytic_aa(
                    level,
                    &lines,
                    plan.frame.width as u16,
                    plan.frame.height as u16,
                );
                assert!(u64::from(tiles.len()) <= bound_tiles);
                assert!(lines.len() <= plan.per_channel_elements[0]);
                generator.generate_filled_path(
                    compound,
                    Fill::NonZero,
                    Affine::scale(scale),
                    None,
                    &mut storage,
                    None,
                );
            }
            assert!(storage.alphas.len() as u64 <= plan.reservation.alpha_bytes_upper_bound);
            assert!(storage.strips.len() as u64 <= plan.reservation.strip_count_upper_bound);
        }
    }
}

/// Exercises genuine hardware GPU output under the external finite no-swap resource supervisor.
/// Full/crop equality proves retained native offsets; duplicate circles explicitly retain the known
/// winding-area coverage approximation, while translucent full interiors follow fixed model equations.
/// # Panics
/// Panics on native failure, wrong composition/backdrop, crop mismatch, or accidental accuracy claims.
/// Writes the admitted union witness only when its explicit validation-directory environment is set.
#[test]
#[ignore = "requires the bounded external GPU validation supervisor"]
fn fast_preview_gpu_native_regions_composition_and_known_union_error() {
    let first = scene(
        HalftoneChannelModel::Rgb,
        vec![circle(0, 16.0, 16.0, 8.0)],
        1.0,
    );
    let plan = FastPreviewPlan::new(
        first,
        1.0,
        RasterBackground::Transparent,
        FastPreviewLimits::default(),
        &|| false,
    )
    .unwrap();
    let mut device = FastPreviewDevice::new(&plan, &|| false).unwrap();
    for model in [HalftoneChannelModel::Rgb, HalftoneChannelModel::Cmyk] {
        for opacity in [1.0, 0.5] {
            for background in [
                RasterBackground::Transparent,
                RasterBackground::OpaqueBlack,
                RasterBackground::OpaqueWhite,
            ] {
                let canonical = scene(
                    model,
                    vec![
                        circle(0, 16.0, 16.0, 8.0),
                        circle(1, 16.0, 16.0, 8.0),
                        circle(2, -1.0, 3.0, 2.0),
                        circle(3, 0.0, 0.0, 0.0),
                    ],
                    opacity,
                );
                let cpu = crate::rasterize(&canonical, background).unwrap();
                let plan = FastPreviewPlan::new(
                    canonical,
                    1.0,
                    background,
                    FastPreviewLimits::default(),
                    &|| false,
                )
                .unwrap();
                let root = device.prepare(plan, &|| false).unwrap();
                let full = device
                    .render_region(
                        &root,
                        Region {
                            x: 0,
                            y: 0,
                            width: 32,
                            height: 32,
                        },
                        &|| false,
                    )
                    .unwrap();
                let crop = device
                    .render_region(
                        &root,
                        Region {
                            x: 7,
                            y: 7,
                            width: 18,
                            height: 18,
                        },
                        &|| false,
                    )
                    .unwrap();
                for y in 0..18 {
                    assert_eq!(
                        &crop.surface.pixels()[y * 18 * 4..(y + 1) * 18 * 4],
                        &full.surface.pixels()[((y + 7) * 32 + 7) * 4..((y + 7) * 32 + 25) * 4]
                    );
                }
                let p = (16 * 32 + 16) * 4;
                assert!(
                    cpu.pixels()[p..p + 4]
                        .iter()
                        .zip(&full.surface.pixels()[p..p + 4])
                        .all(|(a, b)| a.abs_diff(*b) <= 3)
                );
                assert_eq!(full.provenance.algorithm, ALGORITHM_VERSION);
                assert_eq!(full.provenance.root_id, crop.provenance.root_id);
                assert!(matches!(
                    device.render_region(
                        &root,
                        Region {
                            x: 32,
                            y: 0,
                            width: 1,
                            height: 1
                        },
                        &|| false
                    ),
                    Err(FastPreviewError::Limits(_))
                ));
                assert!(matches!(
                    device.render_region(
                        &root,
                        Region {
                            x: 0,
                            y: 0,
                            width: 1,
                            height: 1
                        },
                        &|| true
                    ),
                    Err(FastPreviewError::Cancelled)
                ));
            }
        }
    }
    for model in [HalftoneChannelModel::Rgb, HalftoneChannelModel::Cmyk] {
        let first = scene(model, vec![circle(0, 16.0, 16.0, 8.0)], 1.0);
        let second = RenderLayer::new(
            ChannelId(2),
            true,
            ColorValue {
                red: 0.2,
                green: 0.2,
                blue: 0.2,
                alpha: 1.0,
            },
            1.0,
            GeometryOutput::CanonicalMarks(vec![circle(0, 16.0, 16.0, 8.0)]),
        )
        .unwrap();
        let canonical = Arc::new(
            RenderScene::new_modeled(
                first.canvas().clone(),
                "multichannel-family".into(),
                "multichannel-realization".into(),
                model,
                vec![first.layers()[0].clone(), second],
            )
            .unwrap(),
        );
        let cpu = crate::rasterize(&canonical, RasterBackground::Transparent).unwrap();
        let root = device
            .prepare(
                FastPreviewPlan::new(
                    canonical,
                    1.0,
                    RasterBackground::Transparent,
                    FastPreviewLimits::default(),
                    &|| false,
                )
                .unwrap(),
                &|| false,
            )
            .unwrap();
        let gpu = device
            .render_region(
                &root,
                Region {
                    x: 0,
                    y: 0,
                    width: 32,
                    height: 32,
                },
                &|| false,
            )
            .unwrap();
        let p = (16 * 32 + 16) * 4;
        assert!(
            cpu.pixels()[p..p + 4]
                .iter()
                .zip(&gpu.surface.pixels()[p..p + 4])
                .all(|(a, b)| a.abs_diff(*b) <= 3)
        );
    }
    let canonical = scene(
        HalftoneChannelModel::Rgb,
        vec![circle(0, 16.5, 16.5, 0.5), circle(1, 16.5, 16.5, 0.5)],
        1.0,
    );
    let cpu = crate::rasterize(&canonical, RasterBackground::Transparent).unwrap();
    let root = device
        .prepare(
            FastPreviewPlan::new(
                canonical,
                1.0,
                RasterBackground::Transparent,
                FastPreviewLimits::default(),
                &|| false,
            )
            .unwrap(),
            &|| false,
        )
        .unwrap();
    let gpu = device
        .render_region(
            &root,
            Region {
                x: 0,
                y: 0,
                width: 32,
                height: 32,
            },
            &|| false,
        )
        .unwrap();
    let p = (16 * 32 + 16) * 4 + 3;
    assert!(cpu.pixels()[p] < 230);
    assert_eq!(gpu.surface.pixels()[p], 255);
    if let Some(directory) = std::env::var_os("TONIATOR_FAST_PREVIEW_TEST_ARTIFACTS") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir(&directory).unwrap();
        std::fs::write(
            directory.join("cpu-duplicate-circle.png"),
            crate::encode_png(&cpu).unwrap(),
        )
        .unwrap();
        std::fs::write(
            directory.join("fast-duplicate-circle.png"),
            crate::encode_png(&gpu.surface).unwrap(),
        )
        .unwrap();
        std::fs::write(directory.join("coverage.txt"),format!("Native admitted duplicate circles, radius0.5 center16.5,16.5. Pixel16,16 alpha: CPU={}, Fast={}. No coverage certificate.\n",cpu.pixels()[p],gpu.surface.pixels()[p])).unwrap();
    }

    let too_large = scene(
        HalftoneChannelModel::Rgb,
        vec![circle(0, 16.0, 16.0, 0.1)],
        1.0,
    );
    assert!(matches!(
        FastPreviewPlan::new(
            too_large,
            160.0,
            RasterBackground::Transparent,
            FastPreviewLimits::default(),
            &|| false
        ),
        Err(FastPreviewError::Limits(_))
    ));
    let large_virtual = scene(
        HalftoneChannelModel::Rgb,
        vec![circle(0, 16.0, 16.0, 0.1)],
        1.0,
    );
    let plan = FastPreviewPlan::new(
        large_virtual,
        100.0,
        RasterBackground::Transparent,
        FastPreviewLimits::default(),
        &|| false,
    )
    .unwrap();
    assert_eq!(plan.frame().width, 3200);
    let large_root = device.prepare(plan, &|| false).unwrap();
    let small_region = device
        .render_region(
            &large_root,
            Region {
                x: 1584,
                y: 1584,
                width: 32,
                height: 32,
            },
            &|| false,
        )
        .unwrap();
    assert!(
        small_region
            .surface
            .pixels()
            .chunks_exact(4)
            .any(|pixel| pixel[3] > 0)
    );
    let high_water = device.reservation();
    let small_plan = FastPreviewPlan::new(
        scene(HalftoneChannelModel::Rgb, Vec::new(), 1.0),
        1.0,
        RasterBackground::Transparent,
        FastPreviewLimits::default(),
        &|| false,
    )
    .unwrap();
    let _small_root = device.prepare(small_plan, &|| false).unwrap();
    assert!(device.reservation().host_bytes >= high_water.host_bytes);
    assert_eq!(device.reservation().gpu_bytes, 2 * 1024 * MIB);
    let polls = std::sync::atomic::AtomicUsize::new(0);
    // This closure retires only at the first cancellation check AFTER queue submission.
    assert!(matches!(
        device.render_region(
            &large_root,
            Region {
                x: 1584,
                y: 1584,
                width: 32,
                height: 32
            },
            &|| polls.fetch_add(1, Ordering::Relaxed) >= 2
        ),
        Err(FastPreviewError::Cancelled)
    ));
    assert!(polls.load(Ordering::Relaxed) >= 3);
    assert!(matches!(
        device.render_region(
            &root,
            Region {
                x: 0,
                y: 0,
                width: 32,
                height: 32
            },
            &|| false
        ),
        Err(FastPreviewError::Device(_))
    ));
}
