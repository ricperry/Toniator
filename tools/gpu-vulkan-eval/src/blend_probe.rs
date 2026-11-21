//! Native blend and color-carrier experiments on the two already pinned renderer routes.
use super::*;
use vello_common::{
    kurbo::{Affine, Rect},
    peniko::{BlendMode, Compose, Mix},
};

struct Mode {
    name: &'static str,
    blend: BlendMode,
    opacity: f32,
}

/// Supplies only native exposed blend modes and one deliberately global opacity contrast.
fn modes() -> Vec<Mode> {
    vec![
        Mode {
            name: "src_over",
            blend: BlendMode::new(Mix::Normal, Compose::SrcOver),
            opacity: 1.,
        },
        Mode {
            name: "copy",
            blend: BlendMode::new(Mix::Normal, Compose::Copy),
            opacity: 1.,
        },
        Mode {
            name: "plus_lighter",
            blend: BlendMode::new(Mix::Normal, Compose::PlusLighter),
            opacity: 1.,
        },
        Mode {
            name: "lighten",
            blend: BlendMode::new(Mix::Lighten, Compose::SrcOver),
            opacity: 1.,
        },
        Mode {
            name: "src_over_global_opacity09",
            blend: BlendMode::new(Mix::Normal, Compose::SrcOver),
            opacity: 0.9,
        },
    ]
}

/// Supplies union controls with exact CPU authority, including duplicate and full-pixel contrasts.
fn geometry_controls() -> Vec<Witness> {
    let mut result = witnesses()
        .into_iter()
        .filter(|w| {
            matches!(
                w.name,
                "overlap_union" | "preunion_control" | "touching_halves" | "holes_overlap"
            )
        })
        .collect::<Vec<_>>();
    for (name, paths) in [
        ("full_pixel", vec!["M0 0L1 0L1 1L0 1Z"]),
        (
            "duplicate06",
            vec!["M0 0L0.6 0L0.6 1L0 1Z", "M0 0L0.6 0L0.6 1L0 1Z"],
        ),
        (
            "reversed_overlap",
            vec!["M0.3 0L0.9 0L0.9 1L0.3 1Z", "M0 0L0.6 0L0.6 1L0 1Z"],
        ),
        (
            "opposite_orientation",
            vec!["M0 0L0.6 0L0.6 1L0 1Z", "M0.3 0L0.3 1L0.9 1L0.9 0Z"],
        ),
    ] {
        result.push(Witness {
            name,
            paints: vec![paint(1., 1., 1., 1.); paths.len()],
            paths,
            fill: CanonicalFillRule::NonZero,
            model: None,
        });
    }
    result
}

/// Builds an empty native scene on the existing renderer route without adding a third backend.
/// # Errors
/// Rejects unknown routes before any GPU rendering.
fn empty_scene(route: &str) -> Result<CandidateScene> {
    Ok(match route {
        "hybrid" => {
            let mut s = vello_gpu::Scene::new(SIZE, SIZE);
            s.set_aliasing_threshold(None);
            CandidateScene::Hybrid(Box::new(s))
        }
        "compute-area" => {
            CandidateScene::Compute(Box::new(vello::Scene::new()), vello::AaConfig::Area)
        }
        "compute-msaa16" => {
            CandidateScene::Compute(Box::new(vello::Scene::new()), vello::AaConfig::Msaa16)
        }
        _ => return Err("unknown renderer route".into()),
    })
}

/// Pushes a native full-canvas layer so destructive Copy is tested through its supported API.
fn push(scene: &mut CandidateScene, mode: BlendMode, opacity: f32) {
    match scene {
        CandidateScene::Hybrid(s) => s.push_layer(None, Some(mode), Some(opacity), None, None),
        CandidateScene::Compute(s, _) => s.push_layer(
            Fill::NonZero,
            mode,
            opacity,
            Affine::IDENTITY,
            &Rect::new(0., 0., SIZE.into(), SIZE.into()),
        ),
    }
}

/// Restores exactly one native layer after its matching push.
fn pop(scene: &mut CandidateScene) {
    match scene {
        CandidateScene::Hybrid(s) => s.pop_layer(),
        CandidateScene::Compute(s, _) => s.pop_layer(),
    }
}

/// Draws one canonical exported path with native painting and an explicitly chosen numeric carrier.
/// Numeric-linear payloads are a diagnostic convention; the public color type remains nominal sRGB.
fn draw(scene: &mut CandidateScene, path: &BezPath, color: &ColorValue, numeric_linear: bool) {
    let encode = |value| {
        if numeric_linear {
            value
        } else {
            toniator_render::linear_to_srgb(value)
        }
    };
    let color = Color::new([
        encode(color.red) as f32,
        encode(color.green) as f32,
        encode(color.blue) as f32,
        color.alpha as f32,
    ]);
    match scene {
        CandidateScene::Hybrid(s) => {
            s.set_fill_rule(Fill::NonZero);
            s.set_paint(color);
            s.fill_path(path);
        }
        CandidateScene::Compute(s, _) => s.fill(Fill::NonZero, Affine::IDENTITY, color, None, path),
    }
}

/// Reads only the existing production consumer's positive compound paths, preserving fill authority.
/// # Errors
/// Returns SVG parsing/path errors or rejects an unexpectedly empty canonical export.
fn exported_paths(scene: &RenderScene) -> Result<(String, Vec<BezPath>)> {
    let svg = toniator_render::write_svg(scene)?;
    let doc = roxmltree::Document::parse(&svg)?;
    let paths = doc
        .descendants()
        .filter(|n| {
            n.has_tag_name("path") && n.attribute("id").is_some_and(|s| s.starts_with("channel-"))
        })
        .map(|n| {
            BezPath::from_svg(n.attribute("d").ok_or("path without geometry")?).map_err(Into::into)
        })
        .collect::<Result<Vec<_>>>()?;
    if paths.is_empty() {
        return Err("no production paths".into());
    }
    Ok((svg, paths))
}

/// Renders canonical CPU output with the existing complete layer/model authority and limits.
/// # Errors
/// Propagates canonical scene and raster failures without changing tolerances or input geometry.
fn oracle(w: &Witness) -> Result<(RenderScene, Vec<u8>)> {
    let scene = cpu_scene(w)?;
    let raster = toniator_render::rasterize_output_cancellable(
        &scene,
        RasterBackground::Transparent,
        Some(OutputRasterTarget::new(SIZE.into(), SIZE.into())?),
        RasterAntialiasing::On,
        RasterizationLimits::default(),
        &|| false,
    )?;
    Ok((scene, raster.pixels().to_vec()))
}

/// Converts straight numerical-linear readback to sRGB only after native blending completes.
/// Alpha is preserved; no geometric coverage or channel-composition formula is altered here.
fn encode_numeric_linear(mut bytes: Vec<u8>) -> Vec<u8> {
    for pixel in bytes.chunks_exact_mut(4) {
        for value in &mut pixel[..3] {
            *value = (toniator_render::linear_to_srgb(f64::from(*value) / 255.) * 255.)
                .round()
                .clamp(0., 255.) as u8;
        }
    }
    bytes
}

/// Retains native/straight/encoded output and compares it against the canonical CPU oracle.
/// # Errors
/// Propagates renderer/readback/encoding/file errors; diagnostic failures never grant integration.
fn save_case(
    gpu: &Gpu,
    scene: &CandidateScene,
    root: &Path,
    name: &str,
    cpu: &[u8],
    numeric_linear: bool,
) -> Result<Value> {
    let straight = gpu.render(scene, &root.join(format!("{name}.gpu-native.rgba")))?;
    fs::write(
        root.join(format!("{name}.gpu-straight-carrier.rgba")),
        &straight,
    )?;
    let encoded = if numeric_linear {
        encode_numeric_linear(straight)
    } else {
        straight
    };
    fs::write(root.join(format!("{name}.gpu-encoded.rgba")), &encoded)?;
    write_png(&root.join(format!("{name}.gpu.png")), encoded.clone())?;
    let mut result = compare(cpu, &encoded);
    result["name"] = json!(name);
    result["numeric_linear_carrier"] = json!(numeric_linear);
    result["cpu_pixel16_16"] = json!(&cpu[(16 * 64 + 16) * 4..(16 * 64 + 16) * 4 + 4]);
    result["gpu_pixel16_16"] = json!(&encoded[(16 * 64 + 16) * 4..(16 * 64 + 16) * 4 + 4]);
    Ok(result)
}

/// Executes the authorized tiny native blend alternatives and separate linear-color mapping probe.
/// # Errors
/// Rejects existing output paths and propagates setup/geometry/native rendering failures.
/// The fixed input matrix cannot request dense artwork or change production behavior.
pub(super) fn run(root: &Path, route: &str) -> Result<()> {
    if root.exists() {
        return Err("output exists; preserve prior blend evidence".into());
    }
    fs::create_dir_all(root)?;
    let gpu = pollster::block_on(Gpu::new())?;
    fs::write(
        root.join("adapter.json"),
        serde_json::to_vec_pretty(&gpu.info)?,
    )?;
    let mut geometry_results = Vec::new();
    for w in geometry_controls() {
        let (cpu_scene, cpu) = oracle(&w)?;
        let (svg, compound) = exported_paths(&cpu_scene)?;
        fs::write(root.join(format!("{}.svg", w.name)), svg)?;
        write_png(&root.join(format!("{}.cpu.png", w.name)), cpu.clone())?;
        fs::write(root.join(format!("{}.cpu.rgba", w.name)), &cpu)?;
        let separate = w
            .paths
            .iter()
            .map(|path| {
                let single = Witness {
                    name: w.name,
                    paths: vec![*path],
                    fill: w.fill,
                    paints: vec![paint(1., 1., 1., 1.)],
                    model: None,
                };
                exported_paths(&super::cpu_scene(&single)?).map(|(_, paths)| paths)
            })
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        for (submission, paths) in [("compound", &compound), ("separate", &separate)] {
            for mode in modes() {
                let mut scene = empty_scene(route)?;
                push(&mut scene, BlendMode::default(), mode.opacity);
                for path in paths {
                    push(&mut scene, mode.blend, 1.);
                    draw(&mut scene, path, &paint(1., 1., 1., 1.), false);
                    pop(&mut scene);
                }
                pop(&mut scene);
                let name = format!("{}-{submission}-{}", w.name, mode.name);
                let mut report = save_case(&gpu, &scene, root, &name, &cpu, false)?;
                report["control"] = json!(w.name);
                report["submission"] = json!(submission);
                report["mode"] = json!(mode.name);
                report["global_opacity"] = json!(mode.opacity);
                geometry_results.push(report);
            }
        }
    }
    let mut color_results = Vec::new();
    for (name, model, mode) in [
        (
            "rgb",
            HalftoneChannelModel::Rgb,
            BlendMode::new(Mix::Normal, Compose::PlusLighter),
        ),
        (
            "cmyk",
            HalftoneChannelModel::Cmyk,
            BlendMode::new(Mix::Multiply, Compose::SrcOver),
        ),
        (
            "source",
            HalftoneChannelModel::SourceColorAlpha,
            BlendMode::default(),
        ),
    ] {
        let source = matches!(model, HalftoneChannelModel::SourceColorAlpha);
        let colors = vec![
            paint(0.2, 0.4, 0.1, if source { 1. } else { 0.35 }),
            paint(0.6, 0.1, 0.5, if source { 1. } else { 0.65 }),
        ];
        let w = Witness {
            name: "color",
            paths: vec![
                "M0.25 0.25L40.75 0.25L40.75 40.75L0.25 40.75Z",
                "M10.25 10.25L50.75 10.25L50.75 50.75L10.25 50.75Z",
            ],
            fill: CanonicalFillRule::NonZero,
            paints: colors.clone(),
            model: Some(model),
        };
        let (cpu_scene, cpu) = oracle(&w)?;
        fs::write(
            root.join(format!("color-{name}.svg")),
            toniator_render::write_svg(&cpu_scene)?,
        )?;
        write_png(&root.join(format!("color-{name}.cpu.png")), cpu.clone())?;
        fs::write(root.join(format!("color-{name}.cpu.rgba")), &cpu)?;
        let mappings = if source {
            vec![("model-blend", mode)]
        } else {
            vec![
                ("default-src-over", BlendMode::default()),
                ("model-blend", mode),
            ]
        };
        for (mapping, mode) in mappings {
            for numeric_linear in [false, true] {
                let mut scene = empty_scene(route)?;
                for (path, color) in w.paths.iter().zip(&colors) {
                    let mut color = color.clone();
                    if source {
                        color.alpha = 0.5;
                    }
                    push(&mut scene, mode, 1.);
                    draw(
                        &mut scene,
                        &BezPath::from_svg(path)?,
                        &color,
                        numeric_linear,
                    );
                    pop(&mut scene);
                }
                let carrier = if numeric_linear {
                    "numeric-linear"
                } else {
                    "native-srgb"
                };
                let mut report = save_case(
                    &gpu,
                    &scene,
                    root,
                    &format!("color-{name}-{mapping}-{carrier}"),
                    &cpu,
                    numeric_linear,
                )?;
                report["model"] = json!(name);
                report["blend_mapping"] = json!(mapping);
                report["mode"] = json!(format!("{mode:?}"));
                color_results.push(report);
            }
        }
    }
    fs::write(
        root.join("results.json"),
        serde_json::to_vec_pretty(
            &json!({"route":route,"geometry":geometry_results,"colors":color_results,"contract":"Native blend alternatives only; numerical-linear carrier is flat-color integration experiment with explicit final transfer; no geometry/mask/CPU composition compensation; no guarantees relaxed"}),
        )?,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Proves the blend probe's geometry controls retain the public CPU union contract.
    /// # Panics
    /// Panics if full/overlap/duplicate/touching control construction changes its oracle coverage.
    #[test]
    fn geometry_authority_controls() {
        for (name, alpha) in [
            ("full_pixel", 255),
            ("overlap_union", 230),
            ("duplicate06", 153),
            ("touching_halves", 255),
            ("opposite_orientation", 230),
        ] {
            let w = geometry_controls()
                .into_iter()
                .find(|w| w.name == name)
                .unwrap();
            let (_, cpu) = oracle(&w).unwrap();
            assert_eq!(cpu[3], alpha, "{name}");
        }
    }
}
