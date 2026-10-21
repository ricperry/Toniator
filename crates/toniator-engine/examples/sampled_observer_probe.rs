//! Fresh immutable-artwork sampled measurement baselines with exact bundled saved authority.
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, error::Error, fs, path::PathBuf};
use toniator_domain::*;
use toniator_engine::*;
type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// Serializes all accessible saved numeric observations as exact little-endian f64 bits.
/// Binding/profile/full artifact debug bytes are retained separately; no measurement/refit occurs.
fn numeric_bytes(artifact: &SavedCalibration) -> Vec<u8> {
    let mut values = artifact
        .raw()
        .iter()
        .flat_map(|k| [k.input, k.coverage])
        .collect::<Vec<_>>();
    if let Some(v) = artifact.variation() {
        values.extend([
            v.raw_absolute(),
            v.raw_fraction_of_span(),
            v.normalized_tone_absolute(),
            v.low_tone_relative(),
        ]);
        for optional in [v.uniform_tone_absolute(), v.uniform_low_tone_relative()] {
            values.extend(optional.map_or([0.0, 0.0], |value| [1.0, value]));
        }
    }
    if let Some(v) = artifact.uniform() {
        values.push(v.radius_per_nominal_spacing());
        values.extend(v.inverse().iter().flat_map(|k| [k.input, k.coverage]));
    }
    values
        .into_iter()
        .flat_map(|v| v.to_bits().to_le_bytes())
        .collect()
}

/// Observes fixed256-long-axis center phase on both immutable assets and two genuine catalog recipes.
/// Each scene is evaluated once with for_scene and unchanged compatible bundled artifacts.
/// The configuration requests no fitting; no runtime fit counter is collected. No relabel,
/// source edit or calibration update is requested. All destinations must be fresh.
/// # Errors
/// Returns source/domain/binding/evaluation/measurement or fresh evidence I/O diagnostics.
fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() != 2 {
        return Err("usage: sampled_observer_probe FRESH_OUTPUT".into());
    }
    let out = PathBuf::from(&args[1]);
    if out.exists() {
        return Err("fresh output required".into());
    }
    fs::create_dir_all(&out)?;
    let registry = toniator_patterns::PresetRegistry::bundled();
    let mut records = Vec::new();
    for (label, path, hint, format) in [
        (
            "raster",
            "assets/raster-sample.png",
            SourceFormatHint::Png,
            toniator_io::EmbeddedSourceFormat::Png,
        ),
        (
            "vector",
            "assets/vector-sample.svg",
            SourceFormatHint::Svg,
            toniator_io::EmbeddedSourceFormat::Svg,
        ),
    ] {
        let bytes = fs::read(path)?;
        let decoded = toniator_sampling::decode_source(&bytes, hint)?;
        let canvas = CanvasSpec {
            width: f64::from(decoded.identity().width),
            height: f64::from(decoded.identity().height),
        };
        let width = if canvas.width >= canvas.height {
            256
        } else {
            (256.0 * canvas.width / canvas.height).round() as u32
        };
        let height = if canvas.height >= canvas.width {
            256
        } else {
            (256.0 * canvas.height / canvas.width).round() as u32
        };
        for name in ["straight-grid-circles", "triagrid-spanning-tree"] {
            let folder = out.join(format!("{label}-{name}"));
            fs::create_dir(&folder)?;
            let source_id = SourceReferenceId::new("sampled-observer-immutable")?;
            let base = Document::new_default_document(
                canvas.clone(),
                SourceReference::Assigned(source_id.clone()),
            )?;
            let mut history = DocumentHistory::new(DocumentSession::new(base)?);
            registry.apply_to_document_base(&mut history, name)?;
            let artifact = registry
                .find(name)
                .and_then(|e| e.saved_calibration.as_ref())
                .ok_or("current bundled companion required")?;
            let mut document = history.document().clone();
            let channels = document
                .channel_topology()
                .ok_or("channels")?
                .channels()
                .iter()
                .map(|c| c.id)
                .collect::<Vec<_>>();
            for channel in channels {
                let binding = CalibrationBinding::for_channel(&document, channel)?;
                if !artifact.binding().is_compatible_with(&binding) {
                    return Err("bundled binding mismatch".into());
                }
                document = document.with_saved_calibration(channel, artifact.clone())?;
            }
            document = document.with_saved_baseline_calibration(artifact.clone())?;
            document.require_saved_calibration_authority()?;
            fs::write(
                folder.join("saved-authority.txt"),
                format!(
                    "binding={:#?}\nprofile={}\nall_saved={:#?}\nbaseline={:#?}\n",
                    artifact.binding(),
                    artifact.profile(),
                    document.saved_calibrations(),
                    document.baseline_saved_calibration()?
                ),
            )?;
            fs::write(
                folder.join("calibration-numeric.bin"),
                numeric_bytes(artifact),
            )?;
            let sources = toniator_io::SourceBundle::new([toniator_io::EmbeddedSource::new(
                source_id.clone(),
                format,
                bytes.clone(),
                None,
            )?])?;
            toniator_io::save(&folder.join("fixture.toniator"), &document, &sources)?;
            if toniator_io::load(&folder.join("fixture.toniator"))?.document() != &document {
                return Err("saved authority reopened differently".into());
            }
            let request = EvaluationRequest::new(
                DocumentSession::new(document)?.document_evaluation_snapshot(),
                ResolvedSource::new(source_id, bytes.clone(), hint)?,
            )
            .for_scene()?;
            let evaluated = evaluate_with_limits(request, EvaluationLimits::default())?;
            let scene = evaluated.scene();
            let target = OutputRasterTarget::new(width, height)?;
            let limits =
                RasterizationLimits::new(EvaluationLimits::default().max_flattened_raster_edges())?;
            let mut measurements = Vec::new();
            for layer in scene.layers() {
                for (ordinal, output) in layer.outputs().iter().enumerate() {
                    let prefix = format!("channel-{}-output-{ordinal}", layer.channel_id().0);
                    let mask = toniator_render::rasterize_geometry_occupancy(
                        output.geometry(),
                        &canvas,
                        target,
                        limits,
                        &|| false,
                    )?;
                    fs::write(
                        folder.join(format!("{prefix}-occupancy.bin")),
                        mask.pixels(),
                    )?;
                    let multiplicity = match toniator_render::rasterize_mark_multiplicity(
                        output.geometry(),
                        &canvas,
                        target,
                        limits,
                        &|| false,
                    ) {
                        Ok(mask) => {
                            let mut histogram = BTreeMap::new();
                            for &count in mask.counts() {
                                *histogram.entry(count).or_insert(0usize) += 1;
                            }
                            let data = mask
                                .counts()
                                .iter()
                                .flat_map(|n| n.to_le_bytes())
                                .collect::<Vec<_>>();
                            fs::write(folder.join(format!("{prefix}-multiplicity.bin")), &data)?;
                            json!({"sha256":format!("{:x}",Sha256::digest(data)),"histogram":histogram})
                        }
                        Err(error) if error.path() == "density.uniform.geometry" => {
                            json!({"not_applicable":error.to_string()})
                        }
                        Err(error) => return Err(error.into()),
                    };
                    measurements.push(json!({"channel":layer.channel_id().0,"output":ordinal,"primitives":output.geometry().primitive_count(),"occupancy_sha256":format!("{:x}",Sha256::digest(mask.pixels())),"covered_pixels":mask.covered_pixels(),"fraction_bits":mask.fraction().to_bits(),"multiplicity":multiplicity}));
                }
            }
            for aa in [RasterAntialiasing::Off, RasterAntialiasing::On] {
                let raster = toniator_render::rasterize_output(
                    scene,
                    RasterBackground::Transparent,
                    Some(target),
                    aa,
                )?;
                fs::write(folder.join(format!("raster-{aa:?}.rgba")), raster.pixels())?;
            }
            let record = json!({"source":path,"source_sha256":format!("{:x}",Sha256::digest(&bytes)),"recipe":name,"target":[width,height],"phase":"global physical half-integer centers, origin0, explicit anisotropic target","scene_identity":format!("{:?}",scene.identity()),"calibration_source":"genuine compatible bundled saved authority; no fitting requested (configuration, not an observed counter)","measurements":measurements});
            fs::write(
                folder.join("result.json"),
                serde_json::to_vec_pretty(&record)?,
            )?;
            records.push(record);
        }
    }
    fs::write(
        out.join("results.json"),
        serde_json::to_vec_pretty(&records)?,
    )?;
    Ok(())
}
