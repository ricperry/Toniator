//! Current modeled SVG equations with native/stream references and optional independent consumers.
use std::{
    fs,
    io::Cursor,
    path::{Path, PathBuf},
    process::Command,
};
use toniator_domain::{
    CanvasSpec, ChannelId, ColorValue, HalftoneChannelModel, PatternMechanismId,
};
use toniator_geometry::{
    CanonicalFillRule, CanonicalMark, FamilySiteId, FamilySiteProvenance, SiteScope,
};
use toniator_render::*;

/// Builds a fixture's pure linear process paint with canonical opaque source alpha.
fn paint(r: f64, g: f64, b: f64) -> ColorValue {
    ColorValue {
        red: r,
        green: g,
        blue: b,
        alpha: 1.0,
    }
}

/// Builds one intact circle whose center is safely separated from every analytic edge.
fn circle(ordinal: usize) -> CanonicalMark {
    CanonicalMark::Circle {
        source_site_id: FamilySiteId {
            mechanism_id: PatternMechanismId(1),
            ordinal,
        },
        center: toniator_geometry::Point2::new(16.0, 12.0),
        radius: 9.0,
        scope: SiteScope::Canvas,
        provenance: FamilySiteProvenance::Random {
            candidate_ordinal: ordinal,
            accepted_ordinal: ordinal,
            exclusion_neighbor_ordinal: None,
        },
        fill_rule: CanonicalFillRule::EvenOdd,
    }
}

/// Supplies a layer with explicit local source-over multiplicity and presentation.
/// # Panics
/// Panics if finite fixture geometry or presentation fails current scene admission.
fn layer(id: u64, color: ColorValue, opacity: f64, visible: bool, count: usize) -> RenderLayer {
    RenderLayer::new(
        ChannelId(id),
        visible,
        color,
        opacity,
        GeometryOutput::CanonicalMarks((0..count).map(circle).collect()),
    )
    .unwrap()
}

/// Builds one current model scene without source sampling, fitting or authored calibration.
/// # Panics
/// Panics on invalid fixture topology or paint.
fn modeled(model: HalftoneChannelModel, layers: Vec<RenderLayer>) -> RenderScene {
    RenderScene::new_modeled(
        CanvasSpec {
            width: 32.0,
            height: 24.0,
        },
        "B2-model-contract".into(),
        "analytic-circles".into(),
        model,
        layers,
    )
    .unwrap()
}

/// Supplies known process/opacity/alpha-clamp, layer-over, visibility and order cases.
/// # Panics
/// Panics if source-colored fixture cardinality is invalid.
fn fixtures() -> Vec<(&'static str, RenderScene, Option<[u8; 4]>)> {
    let rgb = HalftoneChannelModel::Rgb;
    vec![
        (
            "rgb-pure-red",
            modeled(rgb, vec![layer(1, paint(1.0, 0.0, 0.0), 1.0, true, 1)]),
            Some([255, 0, 0, 255]),
        ),
        (
            "rgb-pure-white",
            modeled(
                rgb,
                vec![
                    layer(1, paint(1.0, 0.0, 0.0), 1.0, true, 1),
                    layer(2, paint(0.0, 1.0, 0.0), 1.0, true, 1),
                    layer(3, paint(0.0, 0.0, 1.0), 1.0, true, 1),
                ],
            ),
            Some([255, 255, 255, 255]),
        ),
        (
            "rgb-fractional-rg",
            modeled(
                rgb,
                vec![
                    layer(1, paint(1.0, 0.0, 0.0), 0.5, true, 1),
                    layer(2, paint(0.0, 1.0, 0.0), 0.5, true, 1),
                ],
            ),
            Some([188, 188, 0, 255]),
        ),
        (
            "rgb-fractional-reversed",
            modeled(
                rgb,
                vec![
                    layer(2, paint(0.0, 1.0, 0.0), 0.5, true, 1),
                    layer(1, paint(1.0, 0.0, 0.0), 0.5, true, 1),
                ],
            ),
            Some([188, 188, 0, 255]),
        ),
        (
            "rgb-single-half",
            modeled(rgb, vec![layer(1, paint(1.0, 0.0, 0.0), 0.5, true, 1)]),
            Some([255, 0, 0, 128]),
        ),
        (
            "rgb-alpha-clamp",
            modeled(
                rgb,
                vec![
                    layer(1, paint(1.0, 0.0, 0.0), 0.75, true, 1),
                    layer(2, paint(1.0, 0.0, 0.0), 0.75, true, 1),
                ],
            ),
            Some([255, 0, 0, 255]),
        ),
        (
            "rgb-layer-local-over",
            modeled(
                rgb,
                vec![
                    layer(1, paint(1.0, 0.0, 0.0), 0.5, true, 2),
                    layer(2, paint(0.0, 1.0, 0.0), 0.25, true, 1),
                ],
            ),
            Some([225, 137, 0, 255]),
        ),
        (
            "rgb-hidden-zero",
            modeled(
                rgb,
                vec![
                    layer(1, paint(1.0, 0.0, 0.0), 0.5, true, 1),
                    layer(2, paint(0.0, 1.0, 0.0), 1.0, false, 1),
                    layer(3, paint(0.0, 0.0, 1.0), 0.0, true, 1),
                ],
            ),
            Some([255, 0, 0, 128]),
        ),
        (
            "rgb-all-hidden",
            modeled(rgb, vec![layer(1, paint(1.0, 0.0, 0.0), 1.0, false, 1)]),
            Some([0, 0, 0, 0]),
        ),
        (
            "cmyk-control",
            modeled(
                HalftoneChannelModel::Cmyk,
                vec![
                    layer(1, paint(0.0, 1.0, 1.0), 0.5, true, 1),
                    layer(2, paint(1.0, 0.0, 1.0), 0.5, true, 1),
                ],
            ),
            None,
        ),
        (
            "source-alpha-colored-local-over",
            modeled(
                HalftoneChannelModel::SourceColorAlpha,
                vec![
                    RenderLayer::new_source_color_geometry(
                        ChannelId(1),
                        true,
                        0.5,
                        vec![circle(0), circle(1)],
                        vec![paint(1.0, 0.0, 0.0), paint(0.0, 1.0, 0.0)],
                    )
                    .unwrap(),
                ],
            ),
            None, // Existing SourceAlpha local-color correspondence is observed separately from B2 RGB repair.
        ),
        (
            "source-alpha-control",
            modeled(
                HalftoneChannelModel::SourceColorAlpha,
                vec![
                    RenderLayer::new_source_color_geometry(
                        ChannelId(1),
                        true,
                        0.5,
                        vec![circle(0)],
                        vec![paint(0.0, 0.0, 1.0)],
                    )
                    .unwrap(),
                ],
            ),
            Some([0, 0, 255, 128]),
        ),
    ]
}

/// Decodes native straight RGBA without background substitution or hidden-RGB cleanup.
/// # Panics
/// Panics on invalid fixture PNG or a non-RGBA output format.
fn decode(bytes: &[u8]) -> Vec<u8> {
    let mut reader = png::Decoder::new(Cursor::new(bytes)).read_info().unwrap();
    let mut data = vec![0; reader.output_buffer_size().unwrap()];
    let frame = reader.next_frame(&mut data).unwrap();
    assert_eq!(frame.color_type, png::ColorType::Rgba);
    data.truncate(frame.buffer_size());
    data
}

/// Reads an interior pixel from a32×24 fixture, independent of edge antialiasing implementations.
/// # Panics
/// Panics if a consumer returns a wrong-sized image.
fn center(pixels: &[u8]) -> [u8; 4] {
    pixels[4 * (12 * 32 + 16)..4 * (12 * 32 + 16) + 4]
        .try_into()
        .unwrap()
}

/// Runs only owned headless independent consumers with four disposable profile roots and no desktop bus.
/// # Panics
/// Panics on unavailable explicitly requested tools, profile I/O or failed headless export.
fn consume(root: &Path, svg: &Path, kind: &str) -> Vec<u8> {
    let out = root.join(format!("{kind}.png"));
    let mut command = if kind == "resvg" {
        let mut c =
            Command::new(std::env::var_os("TONIATOR_B2_RESVG").expect("explicit owned resvg tool"));
        c.arg(svg).arg(&out);
        c
    } else {
        let mut c = Command::new("inkscape");
        c.arg(svg)
            .args([
                "--batch-process",
                "--app-id-tag=toniator-b2-owned",
                "--export-type=png",
                "--export-area-page",
            ])
            .arg(format!("--export-filename={}", out.display()));
        c
    };
    for (variable, folder) in [
        ("XDG_CONFIG_HOME", "config"),
        ("XDG_DATA_HOME", "data"),
        ("XDG_CACHE_HOME", "cache"),
        ("XDG_STATE_HOME", "state"),
    ] {
        let path = root.join("profile").join(folder);
        fs::create_dir_all(&path).unwrap();
        command.env(variable, path);
    }
    let runtime = root.join("profile/runtime");
    fs::create_dir_all(&runtime).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700)).unwrap();
    }
    command
        .env("XDG_RUNTIME_DIR", &runtime)
        .env("INKSCAPE_PROFILE_DIR", root.join("profile/inkscape"))
        .env(
            "DBUS_SESSION_BUS_ADDRESS",
            format!("unix:path={}/unavailable-bus", runtime.display()),
        )
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY");
    let result = command.output().unwrap();
    fs::write(root.join(format!("{kind}.log")), &result.stderr).unwrap();
    assert!(
        result.status.success(),
        "{kind}: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    decode(&fs::read(out).unwrap())
}

/// Checks premultiplied linear RGB addition, alpha clamp, local over and unchanged control consumers.
/// Optional independent consumers verify interior process values within two observed straight-RGB codes and one alpha code;
/// they do not assert universal cross-engine antialiasing byte identity.
/// # Panics
/// Panics on equation, stream/native parity, editable-geometry or independent interior mismatch.
#[test]
fn modeled_svg_matches_fixed_linear_composition_contract() {
    let out = output_root();
    fs::create_dir_all(&out).unwrap();
    let independent = std::env::var_os("TONIATOR_B2_CONSUMERS").is_some();
    let mut failures = Vec::new();
    let mut observations =
        String::from("case\tnative_center\texpected\tresvg_center\tinkscape_center\n");
    let sampled = RenderLayer::new_source_color_geometry(
        ChannelId(1),
        true,
        0.5,
        vec![circle(0), circle(1)],
        vec![paint(1.0, 0.0, 0.0), paint(0.0, 1.0, 0.0)],
    )
    .unwrap();
    let rejected = RenderScene::new_modeled(
        CanvasSpec {
            width: 32.0,
            height: 24.0,
        },
        "admission".into(),
        "sampled RGB".into(),
        HalftoneChannelModel::Rgb,
        vec![sampled],
    )
    .unwrap_err();
    fs::write(out.join("rgb-sampled-admission.txt"), rejected.to_string()).unwrap();
    for (name, scene, expected) in fixtures() {
        let root = out.join(name);
        fs::create_dir_all(&root).unwrap();
        let svg = write_svg(&scene).unwrap();
        let path = root.join("editable.svg");
        fs::write(&path, &svg).unwrap();
        assert!(!svg.contains("<image"));
        let surface = rasterize_output(
            &scene,
            RasterBackground::Transparent,
            Some(OutputRasterTarget::new(32, 24).unwrap()),
            RasterAntialiasing::On,
        )
        .unwrap();
        fs::write(root.join("native.rgba"), surface.pixels()).unwrap();
        fs::write(root.join("native.png"), encode_png(&surface).unwrap()).unwrap();
        let mut png = Vec::new();
        write_png_streaming(
            &scene,
            StreamingRasterTarget::new(32, 24).unwrap(),
            PngStreamOptions {
                maximum_band_rows: 3,
                ..Default::default()
            },
            RasterizationLimits::default(),
            &mut png,
            &|| false,
            &|_, _| {},
        )
        .unwrap();
        assert_eq!(decode(&png), surface.pixels());
        fs::write(root.join("stream.png"), png).unwrap();
        let native = center(surface.pixels());
        if let Some(expected) = expected {
            assert_eq!(native, expected, "native {name}");
        }
        let mut values = Vec::new();
        if independent {
            for kind in ["resvg", "inkscape"] {
                let pixels = consume(&root, &path, kind);
                fs::write(root.join(format!("{kind}.rgba")), &pixels).unwrap();
                let actual = center(&pixels);
                values.push(actual);
                if let Some(expected) = expected {
                    let differs = if expected[3] == 0 {
                        actual[3] != 0 // Independent hidden RGB is retained and reported, not normalized.
                    } else {
                        actual[..3]
                            .iter()
                            .zip(expected[..3].iter())
                            .any(|(&a, &e)| a.abs_diff(e) > 2)
                            || actual[3].abs_diff(expected[3]) > 1
                    };
                    if differs {
                        failures.push(format!("{name} {kind}: {actual:?}, expected{expected:?}"));
                    }
                }
            }
        }
        observations.push_str(&format!(
            "{name}\t{native:?}\t{expected:?}\t{:?}\t{:?}\n",
            values.first(),
            values.get(1)
        ));
        if name.starts_with("rgb-") && svg.contains("mix-blend-mode:screen") {
            failures.push(format!("{name}: obsolete screen compositor"));
        }
    }
    fs::write(out.join("observations.tsv"), observations).unwrap();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Rejects channel-relative fractional capture fringes for identical full-canvas RGB/CMYK geometry.
/// Independent consumer raw RGBA and coverage are retained; this does not require universal
/// antialiasing or alpha-byte equality across consumers.
/// # Panics
/// Panics on fixture admission, missing explicit consumer configuration or artifact I/O.
#[test]
fn modeled_live_input_fractional_registration() {
    if std::env::var_os("TONIATOR_B2_CONSUMERS").is_none() {
        return;
    }
    let root = output_root().join("fractional-registration");
    fs::create_dir_all(&root).unwrap();
    let mut report = String::from(
        "model\tchannels\twidth\theight\tconsumer\trgb_spread_pixels\tmax_spread\tcorners_rgba\n",
    );
    let mut failures = Vec::new();
    for (model, channel_count) in [
        (HalftoneChannelModel::Rgb, 3),
        (HalftoneChannelModel::Cmyk, 3),
        (HalftoneChannelModel::Cmyk, 4),
    ] {
        let mut colors = if model == HalftoneChannelModel::Rgb {
            vec![
                paint(1.0, 0.0, 0.0),
                paint(0.0, 1.0, 0.0),
                paint(0.0, 0.0, 1.0),
            ]
        } else {
            vec![
                paint(0.0, 1.0, 1.0),
                paint(1.0, 0.0, 1.0),
                paint(1.0, 1.0, 0.0),
                paint(0.0, 0.0, 0.0),
            ]
        };
        colors.truncate(channel_count);
        let layers = colors
            .into_iter()
            .enumerate()
            .map(|(index, color)| {
                let mut mark = circle(index);
                if let CanonicalMark::Circle { radius, .. } = &mut mark {
                    *radius = 40.0;
                }
                RenderLayer::new(
                    ChannelId(index as u64 + 1),
                    true,
                    color,
                    1.0,
                    GeometryOutput::CanonicalMarks(vec![mark]),
                )
                .unwrap()
            })
            .collect();
        let scene = modeled(model, layers);
        let svg = write_svg(&scene).unwrap();
        assert_live_channel_inputs(&svg, channel_count);
        for width in [33.0_f64, 31.0, 32.0, 32.0 / 3.0, 40.0, 64.0] {
            let height = width * 0.75;
            let folder = root.join(format!("{model:?}{channel_count}-{width}"));
            fs::create_dir_all(&folder).unwrap();
            let derivative = svg.replacen(
                "width=\"32\" height=\"24\"",
                &format!("width=\"{width}\" height=\"{height}\""),
                1,
            );
            let path = folder.join("editable.svg");
            fs::write(&path, derivative).unwrap();
            for consumer in ["resvg", "inkscape"] {
                let pixels = consume(&folder, &path, consumer);
                fs::write(folder.join(format!("{consumer}.rgba")), &pixels).unwrap();
                let mut count = 0;
                let mut maximum = 0;
                for p in pixels.chunks_exact(4) {
                    let spread = p[..3].iter().max().unwrap() - p[..3].iter().min().unwrap();
                    count += usize::from(spread > 2);
                    maximum = maximum.max(spread);
                }
                if count > 0 {
                    failures.push(format!("{model:?} at{width}x{height} {consumer}: {count} colored fringe pixels, max spread{maximum}"));
                }
                let w = width.round() as usize;
                let h = pixels.len() / 4 / w;
                let corners = [0, w - 1, (h - 1) * w, h * w - 1].map(|i| &pixels[i * 4..i * 4 + 4]);
                report.push_str(&format!(
                    "{model:?}\t{channel_count}\t{width}\t{height}\t{consumer}\t{count}\t{maximum}\t{corners:?}\n"
                ));
            }
        }
    }
    fs::write(root.join("observations.tsv"), report).unwrap();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Serializes admitted finite huge empty RGB/CMYK scenes without derived atlas overflow.
/// Root and filter frames use only the original positive finite canvas dimensions.
/// # Panics
/// Panics if current admission, serialization, or finite serialized dimensions regress.
#[test]
fn huge_finite_empty_modeled_svg_keeps_original_frame() {
    for (model, count) in [
        (HalftoneChannelModel::Rgb, 1),
        (HalftoneChannelModel::Rgb, 3),
        (HalftoneChannelModel::Rgb, 5),
        (HalftoneChannelModel::Cmyk, 1),
        (HalftoneChannelModel::Cmyk, 4),
        (HalftoneChannelModel::Cmyk, 5),
    ] {
        let layers = (0..count)
            .map(|i| {
                RenderLayer::new(
                    ChannelId(i + 1),
                    true,
                    paint(1.0, 1.0, 1.0),
                    1.0,
                    GeometryOutput::CanonicalMarks(Vec::new()),
                )
                .unwrap()
            })
            .collect();
        let scene = RenderScene::new_modeled(
            CanvasSpec {
                width: f64::MAX * 0.75,
                height: f64::MAX * 0.75,
            },
            "huge-finite-empty".into(),
            "current-admission".into(),
            model,
            layers,
        )
        .unwrap();
        let svg = write_svg(&scene).unwrap();
        let lowercase = svg.to_ascii_lowercase();
        assert!(
            !lowercase.contains("inf") && !lowercase.contains("nan"),
            "{model:?}: {svg}"
        );
        assert!(!svg.contains("feOffset") && !svg.contains("atlas"));
        assert_eq!(svg.matches("<feImage ").count(), count as usize);
    }
}

/// Checks original-frame live channel references without duplicate geometry or a local clip layer.
/// # Panics
/// Panics on stale atlas capture, missing namespace/references, or per-channel clipping.
fn assert_live_channel_inputs(svg: &str, count: usize) {
    assert_eq!(svg.matches("<feImage ").count(), count);
    assert!(svg.contains("xmlns:xlink=\"http://www.w3.org/1999/xlink\""));
    assert!(!svg.contains("SourceGraphic") && !svg.contains("feOffset") && !svg.contains("atlas"));
    assert_eq!(svg.matches("clip-path=\"url(#canvas-clip)\"").count(), 1);
    for i in 1..=count {
        assert_eq!(svg.matches(&format!("id=\"channel-{i}\"")).count(), 1);
        assert!(svg.contains(&format!(
            "href=\"#channel-{i}\" xlink:href=\"#channel-{i}\""
        )));
    }
}

/// Preserves independently positioned channels and a cross-canvas boundary at fractional output scale.
/// Native center paint is exact; independent consumer samples lie inside each distinct circle.
/// # Panics
/// Panics on changed positions, channel/leaf duplication, clipping, or interior paint.
#[test]
fn live_input_channel_positions_and_canvas_clip() {
    let centers = [0.0, 16.0, 26.0];
    let colors = [
        paint(1.0, 0.0, 0.0),
        paint(0.0, 1.0, 0.0),
        paint(0.0, 0.0, 1.0),
    ];
    let layers = centers
        .into_iter()
        .zip(colors)
        .enumerate()
        .map(|(i, (x, color))| {
            let mut mark = circle(i);
            if let CanonicalMark::Circle { center, radius, .. } = &mut mark {
                *center = toniator_geometry::Point2::new(x, 12.0);
                *radius = 4.0;
            }
            RenderLayer::new(
                ChannelId(i as u64 + 1),
                true,
                color,
                1.0,
                GeometryOutput::CanonicalMarks(vec![mark]),
            )
            .unwrap()
        })
        .collect();
    let scene = modeled(HalftoneChannelModel::Rgb, layers);
    let svg = write_svg(&scene).unwrap();
    assert_live_channel_inputs(&svg, 3);
    assert_eq!(svg.matches("<path ").count(), 3);
    let surface = rasterize_output(
        &scene,
        RasterBackground::Transparent,
        Some(OutputRasterTarget::new(32, 24).unwrap()),
        RasterAntialiasing::On,
    )
    .unwrap();
    for (i, x) in [1, 16, 26].into_iter().enumerate() {
        let p = &surface.pixels()[(12 * 32 + x) * 4..(12 * 32 + x) * 4 + 4];
        assert_eq!(p[i], 255);
        assert_eq!(p[3], 255);
    }
    if std::env::var_os("TONIATOR_B2_CONSUMERS").is_none() {
        return;
    }
    let root = output_root().join("position-clip");
    for width in [31, 32, 33] {
        let folder = root.join(width.to_string());
        fs::create_dir_all(&folder).unwrap();
        let path = folder.join("editable.svg");
        fs::write(
            &path,
            svg.replacen(
                "width=\"32\" height=\"24\"",
                &format!("width=\"{width}\" height=\"{}\"", f64::from(width) * 0.75),
                1,
            ),
        )
        .unwrap();
        for consumer in ["resvg", "inkscape"] {
            let pixels = consume(&folder, &path, consumer);
            fs::write(folder.join(format!("{consumer}.rgba")), &pixels).unwrap();
            let h = pixels.len() / 4 / width as usize;
            for (i, x) in [1.0, 16.0, 26.0].into_iter().enumerate() {
                let column = (x * f64::from(width) / 32.0) as usize;
                let offset = (h / 2 * width as usize + column) * 4;
                let p = &pixels[offset..offset + 4];
                assert!(p[i] >= 250 && p[3] >= 250, "{consumer} {width} {i}: {p:?}");
                assert!((0..3).filter(|j| *j != i).all(|j| p[j] <= 2));
            }
        }
    }
}

/// Selects explicit gate evidence or disposable process-local test scratch, never a frozen bundle.
/// Callers own artifact I/O; this path selection does not modify authored project resources.
fn output_root() -> PathBuf {
    std::env::var_os("TONIATOR_B2_OUTPUT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/validation/test-scratch")
                .join(format!("svg-model-composition-{}", std::process::id()))
        })
}
