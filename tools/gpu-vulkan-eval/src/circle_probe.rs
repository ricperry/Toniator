//! Bounded radius/phase diagnostics: same canonical circles, two fixed display scales, no fitted geometry.
use super::*;
use std::sync::Arc;
use toniator_render::preview_tiles::{PreviewRoot, Region};
use vello_common::kurbo::Circle;
/// Measures isolated circle coverage against analytic area and the actual native Bezier bridge.
/// # Errors
/// Rejects existing output/route, malformed canonical inputs, authority or native rendering errors.
/// Circle radii remain authored document units at both scales; these synthetic inputs never fit artworks.
pub(super) fn run(path: &Path, route: &str) -> Result<()> {
    if path.exists() {
        return Err("fresh circle output required".into());
    }
    if !matches!(route, "hybrid" | "compute-area" | "hybrid-refined") {
        return Err("circle route unsupported".into());
    }
    fs::create_dir_all(path)?;
    let radii = [0.1, 0.2, 0.25, 0.5, 1., 2.];
    let phases = [0., 0.25, 0.5];
    let mut marks = Vec::new();
    for (row, phase) in phases.iter().copied().enumerate() {
        for (col, radius) in radii.iter().copied().enumerate() {
            let ordinal = row * 6 + col;
            marks.push(CanonicalMark::Circle {
                source_site_id: FamilySiteId {
                    mechanism_id: PatternMechanismId(1),
                    ordinal,
                },
                center: Point2::new(col as f64 * 16. + 8. + phase, row as f64 * 16. + 8. + phase),
                radius,
                scope: SiteScope::Canvas,
                provenance: FamilySiteProvenance::Random {
                    candidate_ordinal: ordinal,
                    accepted_ordinal: ordinal,
                    exclusion_neighbor_ordinal: None,
                },
                fill_rule: CanonicalFillRule::NonZero,
            });
        }
    }
    let scene = Arc::new(RenderScene::new_modeled(
        CanvasSpec {
            width: 96.,
            height: 48.,
        },
        "circle-radius-phase".into(),
        "unfitted-synthetic".into(),
        HalftoneChannelModel::Rgb,
        vec![RenderLayer::new(
            ChannelId(1),
            true,
            paint(1., 1., 1., 1.),
            1.,
            GeometryOutput::CanonicalMarks(marks),
        )?],
    )?);
    let mut records = Vec::new();
    for scale in [1., 4.] {
        fs::write(path.join("phase.txt"), format!("circle-scale{scale}"))?;
        let width = (96. * scale) as u16;
        let height = (48. * scale) as u16;
        let mut root = PreviewRoot::new(
            scene.clone(),
            scale,
            RasterBackground::Transparent,
            RasterizationLimits::new(
                toniator_engine::EvaluationLimits::default().max_flattened_raster_edges(),
            )?,
            &|| false,
        )?;
        let region = Region {
            x: 0,
            y: 0,
            width: width.into(),
            height: height.into(),
        };
        let cert = root.certify(region, &|| false)?;
        let snap = root.snapshot(&cert)?;
        let tile = snap.paint(region, &cert, &|| false)?;
        let cpu = tile.surface(&root, &cert)?.pixels().to_vec();
        let (encoded, bridge) =
            perf_probe::encode_scene_scaled(&scene, route, None, width, height, scale)?;
        let gpu = pollster::block_on(Gpu::new())?;
        fs::write(
            path.join(format!("adapter-scale{scale}.json")),
            serde_json::to_vec_pretty(&gpu.info)?,
        )?;
        let mut resident = perf_probe::Resident::new(gpu, encoded, width, height)?;
        let raw = resident.render()?;
        fs::write(
            path.join(format!("scale{scale}.gpu-native-linear.rgba")),
            &raw,
        )?;
        let gpu = resident.encode(raw);
        perf_probe::save(
            path,
            &format!("scale{scale}.cpu"),
            width,
            height,
            cpu.clone(),
        )?;
        perf_probe::save(
            path,
            &format!("scale{scale}.gpu"),
            width,
            height,
            gpu.clone(),
        )?;
        for (row, phase) in phases.iter().copied().enumerate() {
            for (col, radius) in radii.iter().copied().enumerate() {
                let x = col * 16 * scale as usize;
                let y = row * 16 * scale as usize;
                let side = 16 * scale as usize;
                let sum = |bytes: &[u8]| {
                    let mut a = 0u64;
                    for yy in y..y + side {
                        for xx in x..x + side {
                            a += u64::from(bytes[(yy * usize::from(width) + xx) * 4 + 3]);
                        }
                    }
                    a as f64 / 255.
                };
                let native_path = Circle::new(
                    (col as f64 * 16. + 8. + phase, row as f64 * 16. + 8. + phase),
                    radius,
                )
                .to_path(1. / (64. * scale));
                records.push(json!({"scale":scale,"radius_document_units":radius,"phase_document_units":phase,"analytic_area_output_pixels":std::f64::consts::PI*radius*radius*scale*scale,"native_bezier_area_output_pixels":native_path.area().abs()*scale*scale,"refined_flattened_area_output_pixels":perf_probe::native_circle_path(Point2::new(col as f64*16.+8.+phase,row as f64*16.+8.+phase),radius,scale,true).area().abs()*scale*scale,"cpu_alpha_area_output_pixels":sum(&cpu),"gpu_alpha_area_output_pixels":sum(&gpu),"window_xywh":[x,y,side,side]}));
            }
        }
        fs::write(
            path.join(format!("bridge-scale{scale}.json")),
            serde_json::to_vec_pretty(&bridge)?,
        )?;
    }
    fs::write(
        path.join("results.json"),
        serde_json::to_vec_pretty(
            &json!({"route":route,"canonical":perf_probe::audit(&scene)?,"records":records,"interpretation":"Unfitted diagnostic. Analytic/Bezierarea isolates upstream nativeflatten/raster from exactradiusbridge; quantizedalpha-sum isnotanalyticcertificate. Phases aredocumentunits, scale4 preservesauthoredcenter/radius."}),
        )?,
    )?;
    Ok(())
}
