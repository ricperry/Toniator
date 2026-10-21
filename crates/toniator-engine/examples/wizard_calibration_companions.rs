//! Offline genuine bundled-companion inventory and typed resource generation.
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{error::Error, fmt::Write as _, fs, path::Path, time::Instant};
use toniator_domain::*;
use toniator_patterns::PresetRegistry;
type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// Materializes an ordinary catalog recipe on the fixed reference without source or GTK state.
/// # Errors
/// Returns domain construction or recipe materialization diagnostics.
fn reference(recipe: &PatternDefinitionRecipe) -> Result<Document> {
    let document = Document::new_default_document(
        CanvasSpec {
            width: 1024.0,
            height: 1024.0,
        },
        SourceReference::Unassigned,
    )?;
    let base = document.pattern_settings().clone();
    let base_definition = document
        .pattern_definition_bundles()
        .iter()
        .find(|entry| entry.id == base.definition_id)
        .ok_or("missing base")?
        .definition
        .clone();
    Ok(document
        .apply_command(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base,
            base_definition,
            recipe: recipe.clone(),
        })?
        .0)
}

/// Emits exact floating observations as typed Rust literals; no binding or profile is relabeled.
/// # Errors
/// Returns formatting diagnostics only.
fn resource(id: &str, artifact: &SavedCalibration) -> Result<String> {
    let mut text = format!(
        "        {id:?} => (\n            {:?},\n            {:?},\n            &[\n",
        format!("{:?}", artifact.binding()),
        artifact.profile()
    );
    for knot in artifact.raw() {
        writeln!(
            text,
            "                CalibrationKnot {{ input: {:?}, coverage: {:?} }},",
            knot.input, knot.coverage
        )?;
    }
    text.push_str("            ],\n");
    if let Some(v) = artifact.variation() {
        writeln!(
            text,
            "            Some(({:?}, {:?}, {:?}, {:?}, {:?}, {:?})),",
            v.raw_absolute(),
            v.raw_fraction_of_span(),
            v.normalized_tone_absolute(),
            v.low_tone_relative(),
            v.uniform_tone_absolute(),
            v.uniform_low_tone_relative()
        )?;
    } else {
        text.push_str("            None,\n");
    }
    if let Some(uniform) = artifact.uniform() {
        writeln!(
            text,
            "            Some(({:?}, &[",
            uniform.radius_per_nominal_spacing()
        )?;
        for knot in uniform.inverse() {
            writeln!(
                text,
                "                CalibrationKnot {{ input: {:?}, coverage: {:?} }},",
                knot.input, knot.coverage
            )?;
        }
        text.push_str("            ])),\n");
    } else {
        text.push_str("            None,\n");
    }
    text.push_str("        ),\n");
    Ok(text)
}

/// Inventories all immutable recipes and optionally measures missing exact producer bindings once.
/// Existing projects contribute only exact compatible artifacts accepted by current domain loading.
/// Generate reuses current artifacts; verify genuinely remeasures every changed producer even
/// after installation while preserving the four explicitly unchanged historical measurements.
/// Outputs are fresh evidence and generated typed data, never user files or a runtime cache.
/// # Errors
/// Returns arguments, IO, binding, genuine bounded fitter, or typed publication diagnostics.
fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    if args.get(1).map(String::as_str) == Some("consumers") && args.len() == 4 {
        return consumers(Path::new(&args[2]), Path::new(&args[3]));
    }
    if args.len() != 4 {
        return Err(
            "usage: wizard_calibration_companions inventory|generate|verify FRESH_OUTPUT DONOR_PATHS_FILE"
                .into(),
        );
    }
    if !matches!(args[1].as_str(), "inventory" | "generate" | "verify") {
        return Err("unknown offline companion operation".into());
    }
    let out = Path::new(&args[2]);
    if out.exists() {
        return Err("fresh output required".into());
    }
    fs::create_dir_all(out)?;
    let mut donors = Vec::new();
    let mut rejected = Vec::new();
    for name in fs::read_to_string(&args[3])?.lines() {
        match toniator_io::load(Path::new(name)) {
            Ok(project) => {
                let digest = format!("{:x}", Sha256::digest(fs::read(name)?));
                for entry in project.document().saved_calibrations() {
                    donors.push((name.to_owned(), digest.clone(), entry.artifact.clone()));
                }
                if let Some(artifact) = project.document().baseline_saved_calibration()? {
                    donors.push((name.to_owned(), digest, artifact.clone()));
                }
            }
            Err(error) => rejected.push(json!({"path":name,"error":error.to_string()})),
        }
    }
    let registry = PresetRegistry::bundled();
    let mut rows = Vec::new();
    let mut generated = String::from(
        "//! Genuine current producer measurements; generated by wizard_calibration_companions.\n//! Provenance: crates/toniator-patterns/resources/builtin-calibration-provenance.json.\nuse super::{CalibrationKnot, MeasuredData};\n\n/// Returns frozen complete binding, producer profile and observations for a catalog resource.\n/// Unknown resources have no companion; this function never measures or changes documents.\npub(super) fn data(id: &str) -> Option<MeasuredData<'static>> {\n    Some(match id {\n",
    );
    for entry in registry.entries() {
        let document = reference(&entry.recipe)?;
        let binding = CalibrationBinding::for_channel(&document, ChannelId(1))?;
        let candidate = donors
            .iter()
            .find(|(_, _, artifact)| artifact.binding() == &binding);
        let mut row = json!({"id":entry.metadata.id,"name":entry.metadata.name,"binding":format!("{binding:?}"),"expected_profile":binding.profile_for_reference(CALIBRATION_PROFILE),"bundled_companion":entry.saved_calibration.is_some(),"exact_existing_donor":candidate.map(|(path,digest,artifact)|json!({"path":path,"sha256":digest,"profile":artifact.profile()}))});
        if args[1] == "generate" || args[1] == "verify" {
            let started = Instant::now();
            let preserve_unchanged = matches!(
                entry.metadata.id.as_str(),
                "even-random-circles"
                    | "source-weighted-dispersion-voronoi"
                    | "straight-grid-circles"
                    | "triagrid-custom-shape-marks"
            );
            let (artifact, origin) = if let Some(artifact) = entry
                .saved_calibration
                .as_ref()
                .filter(|_| args[1] != "verify" || preserve_unchanged)
            {
                (
                    artifact.clone(),
                    json!({"kind":"unchanged-current-bundled"}),
                )
            } else if let Some((path, digest, artifact)) = candidate.filter(|_| args[1] != "verify")
            {
                (
                    artifact.clone(),
                    json!({"kind":"exact-current-project","path":path,"sha256":digest}),
                )
            } else {
                eprintln!("measuring {}", entry.metadata.id);
                (
                    toniator_engine::reference_calibration::calibrate_saved_binding(
                        &binding,
                        toniator_engine::EvaluationLimits::default(),
                        &|| false,
                    )?,
                    json!({"kind":"fresh-genuine-fit","authority":"reference_calibration::calibrate_saved_binding"}),
                )
            };
            if artifact.binding() != &binding {
                return Err("artifact binding differs from exact canonical recipe".into());
            }
            let source_id = SourceReferenceId::new("companion-measurement-witness")?;
            let installed = document
                .with_saved_calibration(ChannelId(1), artifact.clone())?
                .with_saved_baseline_calibration(artifact.clone())?
                .apply_command(&DocumentCommand::SetSourceReference {
                    source: SourceReference::Assigned(source_id.clone()),
                })?
                .0;
            let sources = toniator_io::SourceBundle::new([toniator_io::EmbeddedSource::new(
                source_id,
                toniator_io::EmbeddedSourceFormat::Png,
                fs::read("assets/raster-sample.png")?,
                None,
            )?])?;
            let path = out.join(format!("{}.toniator", entry.metadata.id));
            toniator_io::save(&path, &installed, &sources)?;
            if toniator_io::load(&path)?.document() != &installed {
                return Err("reopen differs".into());
            }
            let arm = resource(&entry.metadata.id, &artifact)?;
            row["origin"] = origin;
            row["exact_existing_artifact_equal"] =
                json!(candidate.map(|(_, _, prior)| prior == &artifact));
            row["seconds"] = json!(started.elapsed().as_secs_f64());
            row["profile"] = json!(artifact.profile());
            row["knots"] = json!(artifact.raw().len());
            row["numeric_rust_sha256"] = json!(format!("{:x}", Sha256::digest(arm.as_bytes())));
            row["artifact_sha256"] = json!(format!("{:x}", Sha256::digest(fs::read(&path)?)));
            generated.push_str(&arm);
            eprintln!(
                "ready {} {:.3}s",
                entry.metadata.id,
                started.elapsed().as_secs_f64()
            );
        }
        rows.push(row);
        fs::write(
            out.join("inventory.json"),
            serde_json::to_vec_pretty(&rows)?,
        )?;
    }
    generated.push_str("        _ => return None,\n    })\n}\n");
    fs::write(
        out.join("rejected-donors.json"),
        serde_json::to_vec_pretty(&rejected)?,
    )?;
    if args[1] == "generate" || args[1] == "verify" {
        fs::write(out.join("current_builtin_calibrations.rs"), generated)?;
    }
    println!(
        "{} catalog recipes; {} loaded donor assignments",
        rows.len(),
        donors.len()
    );
    Ok(())
}

/// Compares the actual pre-fix Wizard measurement with its bundled equivalent on both immutable sources.
/// Documents use natural artwork dimensions, shared canonical evaluation and raw native RGBA/PNG/SVG.
/// No fit is invoked: strict saved authority is mandatory before every production evaluation.
/// # Errors
/// Returns fresh-directory, source, domain, exact artifact equality, render or output diagnostics.
fn consumers(out: &Path, before_project: &Path) -> Result<()> {
    use toniator_engine::*;
    if out.exists() {
        return Err("fresh consumer directory required".into());
    }
    fs::create_dir_all(out)?;
    let before = toniator_io::load(before_project)?;
    let measured = before
        .document()
        .baseline_saved_calibration()?
        .ok_or("missing measured baseline")?;
    let registry = PresetRegistry::bundled();
    let bundled = registry
        .find("clustered-dispersion-random-links")
        .and_then(|p| p.saved_calibration.as_ref())
        .ok_or("missing bundled companion")?;
    if measured != bundled {
        return Err("actual before Wizard artifact differs from bundled measurement".into());
    }
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
        let id = SourceReferenceId::new("immutable-consumer-artwork")?;
        let base = Document::new_default_document(
            CanvasSpec {
                width: f64::from(decoded.identity().width),
                height: f64::from(decoded.identity().height),
            },
            SourceReference::Assigned(id.clone()),
        )?;
        let sources = toniator_io::SourceBundle::new([toniator_io::EmbeddedSource::new(
            id.clone(),
            format,
            bytes.clone(),
            None,
        )?])?;
        let mut history = DocumentHistory::new(DocumentSession::new(base)?);
        registry.apply_to_document_base(&mut history, "clustered-dispersion-random-links")?;
        let mut outputs = Vec::new();
        for (kind, artifact) in [("before", measured), ("after", bundled)] {
            let mut document = history.document().clone();
            for channel in document
                .channel_topology()
                .ok_or("channels")?
                .channels()
                .iter()
                .map(|c| c.id)
                .collect::<Vec<_>>()
            {
                document = document.with_saved_calibration(channel, artifact.clone())?;
            }
            document = document.with_saved_baseline_calibration(artifact.clone())?;
            document.require_saved_calibration_authority()?;
            let dir = out.join(format!("{label}-{kind}"));
            fs::create_dir(&dir)?;
            toniator_io::save(&dir.join("fixture.toniator"), &document, &sources)?;
            let reopened = toniator_io::load(&dir.join("fixture.toniator"))?;
            if reopened.document() != &document {
                return Err("saved consumer document differs".into());
            }
            let request = EvaluationRequest::new(
                DocumentSession::new(document)?.document_evaluation_snapshot(),
                ResolvedSource::new(id.clone(), bytes.clone(), hint)?,
            )
            .for_scene()?;
            let evaluated = evaluate_with_limits(request, EvaluationLimits::default())?;
            let scene = evaluated.scene();
            let native = toniator_render::rasterize(scene, RasterBackground::Transparent)?;
            let png = encode_png(&native)?;
            let svg = write_svg(scene)?;
            let preview = toniator_render::rasterize_preview(
                scene,
                PreviewRasterTarget::new(1024, 1024)?
                    .with_background(RasterBackground::OpaqueBlack),
            )?;
            fs::write(dir.join("native.rgba"), native.pixels())?;
            fs::write(dir.join("native.png"), png)?;
            fs::write(dir.join("native.svg"), &svg)?;
            fs::write(dir.join("preview.png"), encode_png(&preview)?)?;
            let svg_consumer = toniator_sampling::reduced_preview_png(
                svg.as_bytes(),
                SourceFormatHint::Svg,
                f64::from(decoded.identity().width.max(decoded.identity().height)) as u32,
            )?;
            fs::write(dir.join("svg-consumer.png"), svg_consumer.png_bytes)?;
            outputs.push((native.pixels().to_vec(), svg, preview.pixels().to_vec()));
        }
        if outputs[0] != outputs[1] {
            return Err("native/preview/SVG consumer pair differs".into());
        }
        records.push(json!({"source":path,"source_sha256":format!("{:x}",Sha256::digest(&bytes)),"natural_dimensions":[decoded.identity().width,decoded.identity().height],"native_rgba_exact":true,"preview_rgba_exact":true,"editable_svg_exact":true,"actual_before_wizard_artifact_equals_bundle":true,"new_fit_calls":0}));
    }
    fs::write(
        out.join("result.json"),
        serde_json::to_vec_pretty(&records)?,
    )?;
    Ok(())
}
