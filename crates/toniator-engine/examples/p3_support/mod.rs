//! Shared opt-in profiling support; this module never enters a product build.
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    env,
    error::Error,
    fs,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    time::Instant,
};
use toniator_domain::*;
use toniator_engine::*;
use toniator_patterns::{self as patterns, FamilySiteProvenance, TypedFamilyOutput};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// Emits independent JSON records outside measured family operations.
pub fn emit(value: Value) {
    println!("{value}");
}

/// Hashes exact output bytes without a compositing or normalization step.
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Loads a source-free current Preset onto a fresh immutable intrinsic source.
///
/// # Errors
/// Returns current schema, calibration, source or binding diagnostics without changing inputs.
fn fixture(source: &str, recipe: &str, scale: &str) -> Result<(Document, ResolvedSource)> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (name, w, h, hint) = match source {
        "png" => ("raster-sample.png", 1024., 1024., SourceFormatHint::Png),
        "svg" => ("vector-sample.svg", 900., 620., SourceFormatHint::Svg),
        _ => return Err("source must be png or svg".into()),
    };
    let id = SourceReferenceId::new(format!("p3-{source}"))?;
    let fresh = Document::new_default_document(
        CanvasSpec {
            width: w,
            height: h,
        },
        SourceReference::Assigned(id.clone()),
    )?;
    let path = root.join(format!(
        "fixtures/performance/{recipe}-{source}-f{scale}.toniator-preset"
    ));
    let document = toniator_io::load_document_preset(&path, &fresh)?.bind(&fresh)?;
    document.require_saved_calibration_authority()?;
    Ok((
        document,
        ResolvedSource::new(id, fs::read(root.join("assets").join(name))?, hint)?,
    ))
}

/// Derives clean-checkout source-free fixtures from current accepted native recipe bytes.
///
/// # Errors
/// Returns load, current-schema, missing-file, write or roundtrip mismatch diagnostics;
/// create-only guards prevent overwriting any destination.
fn prepare(directory: &Path) -> Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for recipe in [
        "even-random-circles",
        "clustered-dispersion-random-links",
        "source-weighted-dispersion-voronoi",
    ] {
        for (source, legacy_name) in [("png", "raster"), ("svg", "vector")] {
            for scale in ["1", "025"] {
                let input = root.join(format!("target/validation/ton018-family-regression-20261013/native-projects/{recipe}-{legacy_name}-f{scale}.toniator"));
                let loaded = toniator_io::load(&input)?;
                let config = DocumentConfiguration::capture(loaded.document());
                let output = root.join(format!(
                    "fixtures/performance/{recipe}-{source}-f{scale}.toniator-preset"
                ));
                let guard = toniator_io::capture_document_preset_destination(&output)?;
                if guard.is_existing() {
                    return Err("prepare refuses existing fixtures".into());
                }
                toniator_io::save_document_preset(&output, &config, &guard)?;
                let decoded = toniator_io::load_document_preset(&output, loaded.document())?;
                if config != decoded {
                    return Err("configuration roundtrip differs".into());
                }
                emit(
                    json!({"kind":"fixture", "input":input, "input_sha256":hash(&fs::read(&input)?), "output":output, "output_sha256":hash(&fs::read(&output)?), "roundtrip_configuration_equal":true, "schema":"current-only"}),
                );
            }
        }
    }
    fs::write(
        directory.join("fixture-authority.txt"),
        "Derived with current load/capture/save/load APIs; exact DocumentConfiguration equality. Sources are rebound at immutable intrinsic dimensions. No source bytes enter Presets.\n",
    )?;
    Ok(())
}

/// Reproduces the current engine support planning for these three random-site output kinds.
///
/// This example-only projection is checked against temporary entry instrumentation in the
/// same engine calls. It delegates diameter/program scaling and all layout resolution to
/// current domain/pattern APIs and rejects other output kinds.
///
/// # Errors
/// Returns unsupported-output or existing scale/program diagnostics before timing.
fn family_request(
    document: &Document,
    definition: &PatternDefinition,
    plan: &patterns::PatternPipelinePlan,
    channel: ChannelId,
) -> Result<GridInspectRequest> {
    let effective = document.effective_channel_pattern(channel)?;
    let diameter = patterns::maximum_nominal_cell_diameter(
        &plan.family,
        document.canvas(),
        &effective.resolved_density,
    )?;
    let mut support: f64 = 0.;
    for output in &plan.ordered_outputs {
        let current = if let Some((_, program, _)) = output.connection_paths() {
            let program = patterns::resolve_connection_program_for_feature_size(
                program,
                document.canvas(),
                &effective.resolved_density,
            )?;
            diameter
                + definition.coverage.additional_margin
                + f64::from(definition.coverage.guard_steps) * program.adjacency().maximum_distance
        } else if output.regions().is_some() {
            definition.coverage.additional_margin
                + f64::from(definition.coverage.guard_steps) * diameter
                + (2.0 - 1.0) * diameter
        } else if output.marks().is_some() {
            let fill = if plan
                .family
                .random
                .as_ref()
                .is_some_and(|r| r.full_population_for_uniform_marks)
            {
                8.
            } else {
                2.
            };
            fill * diameter / 2. + definition.coverage.additional_margin
        } else {
            return Err(
                "probe admits only current random marks, connections and Voronoi outputs".into(),
            );
        };
        support = support.max(current);
    }
    Ok(GridInspectRequest {
        canvas: document.canvas().clone(),
        density: effective.resolved_density,
        rotation_degrees: effective.pattern_rotation_degrees,
        translation_x: effective.translation_x,
        translation_y: effective.translation_y,
        guard_steps: definition.coverage.guard_steps,
        support_radius: support,
        max_family_candidates: usize::MAX,
    })
}

/// Streams unambiguous full-site bytes and SHA256 outside the family clock.
/// Variant tags, lengths and counts delimit exact IEEE bits and all Random provenance.
///
/// # Errors
/// Returns create-only evidence I/O failures or an unexpected non-Random provenance variant.
fn family_digest(output: &TypedFamilyOutput, path: Option<&Path>) -> Result<String> {
    let mut writer = path
        .map(|p| fs::OpenOptions::new().write(true).create_new(true).open(p))
        .transpose()?
        .map(BufWriter::new);
    let mut digest = Sha256::new();
    let mut put = |bytes: &[u8]| -> Result<()> {
        digest.update(bytes);
        if let Some(writer) = writer.as_mut() {
            writer.write_all(bytes)?;
        }
        Ok(())
    };
    put(b"toniator-p3-full-random-family-v2\0")?;
    let fp = output.family_fingerprint().as_bytes();
    put(&(fp.len() as u64).to_le_bytes())?;
    put(fp)?;
    put(&output.site_set().product_mechanism_id().0.to_le_bytes())?;
    put(&output.planned_support_radius().to_bits().to_le_bytes())?;
    put(&output.guard_steps().to_le_bytes())?;
    put(&(output.site_set().sites().len() as u64).to_le_bytes())?;
    for site in output.site_set().sites() {
        put(&site.id.mechanism_id.0.to_le_bytes())?;
        put(&(site.id.ordinal as u64).to_le_bytes())?;
        for v in [
            site.position.x,
            site.position.y,
            site.nominal_cell_basis.axis_a.x,
            site.nominal_cell_basis.axis_a.y,
            site.nominal_cell_basis.axis_b.x,
            site.nominal_cell_basis.axis_b.y,
            site.nominal_cell_basis.diameter(),
        ] {
            put(&v.to_bits().to_le_bytes())?;
        }
        put(&[u8::from(site.scope == SiteScope::Guard)])?;
        match &site.provenance {
            FamilySiteProvenance::Random {
                candidate_ordinal,
                accepted_ordinal,
                exclusion_neighbor_ordinal,
            } => {
                put(&[3])?;
                put(&(*candidate_ordinal as u64).to_le_bytes())?;
                put(&(*accepted_ordinal as u64).to_le_bytes())?;
                match exclusion_neighbor_ordinal {
                    Some(v) => {
                        put(&[1])?;
                        put(&(*v as u64).to_le_bytes())?;
                    }
                    None => put(&[0])?,
                }
            }
            _ => return Err("non-Random site in random probe".into()),
        }
    }
    if let Some(d) = output.random_diagnostics() {
        put(&[1])?;
        for n in [
            d.offered_candidates,
            d.achieved_sites,
            d.candidates_considered,
            d.rejected_by_density,
            d.rejected_by_exclusion,
            d.rejected_outside_envelope,
            d.canvas_sites,
            d.guard_sites,
        ] {
            put(&(n as u64).to_le_bytes())?;
        }
    } else {
        put(&[0])?;
    }
    if let Some(writer) = writer.as_mut() {
        writer.flush()?;
    }
    Ok(format!("{:x}", digest.finalize()))
}

/// Builds focused controls through the current typed constructor without changing a document.
///
/// # Errors
/// Returns validation diagnostics; normal product limits and source requirements remain operative.
fn control(kind: &str, seed: u32) -> Result<PatternDefinition> {
    let character = if kind.starts_with("clustered") {
        RandomSiteCharacter::Clustered {
            cluster_density: 8.,
            cluster_spread: 24.,
            cluster_strength: 0.85,
        }
    } else if kind.starts_with("stratified") {
        RandomSiteCharacter::Stratified { jitter: 1. }
    } else {
        RandomSiteCharacter::RawUniform
    };
    let refinement = RandomSiteRefinement {
        enabled: kind.contains("lloyd"),
        density_weighted: kind.contains("weighted"),
        iterations: 2,
    };
    let definition = PatternDefinition::random_sites_with_refinement(
        PatternDefinitionId(1),
        format!("p3-{kind}"),
        PatternMechanismId(11),
        PatternMechanismId(12),
        PatternMechanismId(13),
        PatternMechanismId(14),
        PatternOutputLayerId(15),
        character,
        seed,
        SiteDensityModulation::Uniform,
        if kind.contains("exclusion") {
            SiteExclusionPolicy::MinimumCenterDistance { minimum: 5. }
        } else {
            SiteExclusionPolicy::None
        },
        16_000_000,
        16_000_000,
        refinement,
        CoveragePolicy {
            guard_steps: 1,
            additional_margin: 0.,
        },
    );
    validate_pattern_definition(&definition)?;
    Ok(definition)
}

/// Measures current family APIs with setup/decode/hash/output outside the family clock.
///
/// # Errors
/// Returns pipeline, explicit finite-limit or cancellation diagnostics without a partial product.
///
/// # Panics
/// Panics if equal seeded inputs produce different full-site digests.
fn family(
    document: &Document,
    source: &ResolvedSource,
    kind: &str,
    repeats: usize,
    directory: &Path,
) -> Result<()> {
    let channel = document
        .channel_topology()
        .ok_or("missing topology")?
        .channels()
        .first()
        .ok_or("missing channel")?
        .id;
    let seed = env::var("P3_SEED").ok().map(|s| s.parse()).transpose()?;
    let mut definition = if let Some(control_kind) = kind.strip_prefix("control:") {
        control(control_kind, seed.unwrap_or(0))?
    } else {
        document
            .pattern_definition_for(channel)
            .ok_or("missing definition")?
            .clone()
    };
    if let Some(seed) = seed {
        for mechanism in &mut definition.mechanisms {
            if let PatternMechanism::RandomSiteProcess { seed: s, .. } = mechanism {
                *s = seed;
            }
        }
    }
    validate_pattern_definition(&definition)?;
    let weighting = document
        .channel_weighting(channel)
        .ok_or("missing weighting")?;
    let plan = patterns::resolve_pattern_pipeline_for_channel(&definition, weighting)?;
    let mut request = family_request(document, &definition, &plan, channel)?;
    if let Ok(limit) = env::var("P3_LIMIT") {
        request.max_family_candidates = limit.parse()?;
    }
    let source_field = toniator_sampling::decode_source(
        source.bytes().ok_or("missing source")?,
        source.format().ok_or("missing hint")?,
    )?;
    emit(
        json!({"kind":"family_setup", "request":format!("{request:?}"), "definition":format!("{definition:?}"), "family_source_sensitive":patterns::family_requires_decoded_source(&plan.family), "site_record_bytes":std::mem::size_of::<patterns::FamilySite>()}),
    );
    let cancel_after = env::var("P3_CANCEL_MS")
        .ok()
        .map(|v| v.parse::<u64>())
        .transpose()?;
    let mut expected = None;
    for repeat in 0..repeats {
        crate::allocation::begin();
        let start = Instant::now();
        let result = patterns::evaluate_typed_family_product_with_source_progress_cancellable(
            &plan.family,
            &request,
            patterns::family_requires_decoded_source(&plan.family).then_some(&source_field),
            &|| cancel_after.is_some_and(|ms| start.elapsed().as_millis() >= u128::from(ms)),
            &|_, _| {},
        );
        let us = start.elapsed().as_micros();
        let allocation = crate::allocation::end();
        match result {
            Ok(output) => {
                let bytes_path = directory.join("full-family.bin");
                let digest = family_digest(&output, (repeat == 0).then_some(bytes_path.as_path()))?;
                if let Some(ref previous) = expected {
                    assert_eq!(previous, &digest);
                } else {
                    expected = Some(digest.clone());
                }
                emit(
                    json!({"kind":"family", "repeat":repeat,"us":us,"full_sites_sha256":digest,"fingerprint":output.family_fingerprint(),"diagnostics":output.random_diagnostics(),"support_radius":output.planned_support_radius()}),
                );
            }
            Err(error) => {
                let expected_probe = (cancel_after.is_some()
                    && error.path() == "evaluation.cancelled")
                    || (env::var_os("P3_LIMIT").is_some()
                        && error.path() == "coverage.candidate_limit");
                emit(
                    json!({"kind":"family_error","repeat":repeat,"us":us,"path":error.path(),"message":error.message(),"cancel_after_ms":cancel_after,"expected_probe":expected_probe}),
                );
                if !expected_probe {
                    return Err(error.into());
                }
            }
        }
        if let Some(allocation) = allocation {
            emit(json!({"kind":"allocation","repeat":repeat,"allocation":allocation}));
        }
    }
    Ok(())
}

/// Captures separate authoritative engine stages and native transparent output after success.
///
/// # Errors
/// Returns ordinary engine/export errors; file creation happens outside API timing.
///
/// # Panics
/// Panics if a validated topology lacks a first channel, warm RGBA changes, or the
/// probe support/guard projection disagrees with the current engine family authority.
fn engine(document: Document, source: ResolvedSource, directory: &Path) -> Result<()> {
    let mut history = DocumentHistory::new(DocumentSession::new(document)?);
    let channel = history
        .document()
        .channel_topology()
        .ok_or("missing topology")?
        .channels()[0]
        .id;
    let mut cache = EvaluationProfileCache::default();
    let mut prior = None;
    let labels = if env::var_os("P3_EDITS").is_some() {
        vec!["cold", "warm", "seed", "weight", "presentation"]
    } else {
        vec!["cold", "warm"]
    };
    for label in labels {
        if label == "seed" {
            let base = history
                .document()
                .pattern_definition_for(channel)
                .ok_or("missing definition")?
                .clone();
            let mechanism_id = base
                .mechanisms
                .iter()
                .find_map(|m| {
                    if let PatternMechanism::RandomSiteProcess { id, .. } = m {
                        Some(*id)
                    } else {
                        None
                    }
                })
                .ok_or("missing random process")?;
            history.apply(&DocumentCommand::EditSelectedChannelPatternDefinition {
                channel_id: channel,
                base_definition: base,
                edit: PatternDefinitionEdit::SetRandomSeed {
                    mechanism_id,
                    seed: 123456789,
                },
            })?;
        } else if label == "weight" {
            history.apply(&DocumentCommand::SetSourceWeightingField {
                channel_id: channel,
                edit: SourceWeightingFieldEdit::Gain(0.6),
            })?;
        } else if label == "presentation" {
            history.apply(&DocumentCommand::SetOpacity {
                channel_id: channel,
                opacity: 0.7,
            })?;
        }
        let request = EvaluationRequest::new(
            history.session().document_evaluation_snapshot(),
            source.clone(),
        );
        let start = Instant::now();
        let output = evaluate_profiled_cached_with_limits(
            request.clone(),
            EvaluationLimits::default(),
            &mut cache,
        )?;
        let api_us = start.elapsed().as_micros();
        let records=output.performance.records.iter().map(|r|json!({"stage":format!("{:?}",r.stage),"us":r.elapsed.as_micros(),"cache":format!("{:?}",r.cache),"execution":format!("{:?}",r.execution),"workloads":r.workloads.iter().map(|w|json!({"kind":format!("{:?}",w.kind),"count":w.count})).collect::<Vec<_>>()})).collect::<Vec<_>>();
        let rgba = hash(output.result.raster().pixels());
        if let Some(ref old) = prior {
            if label == "warm" {
                assert_eq!(old, &rgba);
            }
        } else {
            prior = Some(rgba.clone());
        }
        emit(
            json!({"kind":"engine","label":label,"api_us":api_us,"records":records,"family":output.result.scene().identity().family_fingerprint(),"scene":output.result.scene().identity().scene_fingerprint(),"rgba_sha256":rgba}),
        );
        if label == "cold" {
            for summary in output.result.channels() {
                let family = output
                    .result
                    .family_output(summary.channel_id())
                    .ok_or("missing engine family")?;
                let definition = history
                    .document()
                    .pattern_definition_for(summary.channel_id())
                    .ok_or("missing definition")?;
                let plan = patterns::resolve_pattern_pipeline_for_channel(
                    definition,
                    history
                        .document()
                        .channel_weighting(summary.channel_id())
                        .ok_or("missing weighting")?,
                )?;
                let projected =
                    family_request(history.document(), definition, &plan, summary.channel_id())?;
                assert_eq!(
                    projected.support_radius.to_bits(),
                    family.planned_support_radius().to_bits(),
                    "example support matches engine authority"
                );
                assert_eq!(
                    projected.guard_steps,
                    family.guard_steps(),
                    "example guard matches engine authority"
                );
                let bytes = directory.join(format!("engine-family-{}.bin", summary.channel_id().0));
                emit(
                    json!({"kind":"engine_family","channel":summary.channel_id().0,"support_radius":family.planned_support_radius(),"guard_steps":family.guard_steps(),"fingerprint":family.family_fingerprint(),"full_sites_sha256":family_digest(family,Some(&bytes))?,"diagnostics":family.random_diagnostics()}),
                );
            }
            fs::write(
                directory.join("native.png"),
                encode_png(output.result.raster())?,
            )?;
            if env::var_os("P3_SKIP_SVG").is_none() {
                match write_svg(output.result.scene()) {
                    Ok(svg) => fs::write(directory.join("native.svg"), svg)?,
                    Err(e) => fs::write(directory.join("svg-error.txt"), e.to_string())?,
                };
            } else {
                emit(
                    json!({"kind":"export_skipped","format":"svg","reason":"explicit P3_SKIP_SVG; initial native exports retained separately"}),
                );
            }
        }
    }
    Ok(())
}

/// Measures return latency after a separate thread signals the real engine cancellation flag.
/// Source setup is outside the API clock; decoding and cleanup remain inside it.
///
/// # Errors
/// Returns malformed delay, poisoned signal metadata, or unexpected evaluation errors without output publication.
///
/// # Panics
/// Panics if the scoped signal thread cannot start or unexpectedly panics; ordinary cancellation
/// uses the engine error result and never publishes a partial output.
fn engine_cancel(document: Document, source: ResolvedSource) -> Result<()> {
    use std::sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    };
    let delay: u64 = env::var("P3_CANCEL_MS")?.parse()?;
    let session = DocumentSession::new(document)?;
    let request = EvaluationRequest::new(session.document_evaluation_snapshot(), source);
    let cancelled = AtomicBool::new(false);
    let signalled = Mutex::new(None);
    let started = Instant::now();
    let (result, returned) = std::thread::scope(|scope| {
        scope.spawn(|| {
            std::thread::sleep(std::time::Duration::from_millis(delay));
            let mut time = signalled.lock().expect("signal lock");
            *time = Some(Instant::now());
            cancelled.store(true, Ordering::Release);
        });
        let result =
            evaluate_cancellable_with_limits(request, EvaluationLimits::default(), &cancelled);
        (result, Instant::now())
    });
    let signal = signalled
        .into_inner()
        .map_err(|_| "signal lock poisoned")?
        .ok_or("signal absent")?;
    match result {
        Err(EvaluationRunError::Cancelled) => emit(
            json!({"kind":"engine_cancel","delay_ms":delay,"api_us":returned.duration_since(started).as_micros(),"return_after_signal_us":returned.saturating_duration_since(signal).as_micros(),"cancelled":true,"partial_output":false}),
        ),
        Err(error) => return Err(format!("{error:?}").into()),
        Ok(_) => {
            return Err(
                "engine completed before external cancellation; not a cancelled witness".into(),
            );
        }
    }
    Ok(())
}

/// Saves the exact current fine fixture with its immutable source for parent private GTK checks.
///
/// # Errors
/// Returns current project/source I/O errors without overwriting a pre-existing destination.
fn save_project(
    document: Document,
    source: ResolvedSource,
    directory: &Path,
    source_name: &str,
) -> Result<()> {
    use toniator_io::{EmbeddedSource, EmbeddedSourceFormat, SourceBundle};
    let format = if source_name == "png" {
        EmbeddedSourceFormat::Png
    } else {
        EmbeddedSourceFormat::Svg
    };
    let sources = SourceBundle::new([EmbeddedSource::new(
        source.reference_id().clone(),
        format,
        source.bytes().ok_or("missing source")?.to_vec(),
        None,
    )?])?;
    let path = directory.join(format!("fine-even-{source_name}.toniator"));
    if path.exists() {
        return Err("refuse existing project".into());
    }
    toniator_io::save(&path, &document, &sources)?;
    let loaded = toniator_io::load(&path)?;
    if loaded.document() != &document {
        return Err("current project roundtrip mismatch".into());
    }
    emit(
        json!({"kind":"project","path":path,"sha256":hash(&fs::read(&path)?),"document_roundtrip_equal":true}),
    );
    Ok(())
}

/// Dispatches one explicit probe; the external runner owns process timeouts and serialization.
///
/// # Errors
/// Returns malformed arguments or selected probe diagnostics with no automatic fallback.
pub fn run() -> Result<()> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 6 {
        return Err("MODE SOURCE RECIPE SCALE OUTPUT REPEATS".into());
    }
    let directory = Path::new(&args[4]);
    let admitted = directory.canonicalize()?;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/validation/performance-20261016/p3")
        .canonicalize()?;
    let p3b = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/validation/performance-20261016/p3b")
        .canonicalize()?;
    if !admitted.starts_with(&root) && !admitted.starts_with(&p3b) {
        return Err("output must be inside P3/P3b evidence".into());
    }
    let repeats: usize = args[5].parse()?;
    if !(1..=10).contains(&repeats) {
        return Err("repeats must be 1..=10".into());
    }
    if args[0] != "prepare" {
        if admitted == root
            || admitted == p3b
            || fs::read_dir(&admitted)?.any(|entry| {
                entry.is_ok_and(|e| {
                    !matches!(
                        e.file_name().to_str(),
                        Some("records.jsonl" | "stderr.log" | "process.time")
                    )
                })
            })
        {
            return Err("output directory already contains evidence".into());
        }
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(admitted.join(".probe-admitted"))?;
    }
    if args[0] == "prepare" {
        return prepare(directory);
    }
    let kind = &args[2];
    let recipe = if kind.starts_with("control:") {
        "even-random-circles"
    } else {
        kind.as_str()
    };
    let start = Instant::now();
    let (document, source) = fixture(&args[1], recipe, &args[3])?;
    emit(
        json!({"kind":"setup","us":start.elapsed().as_micros(),"source":args[1],"recipe":kind,"scale":args[3],"source_sha256":hash(source.bytes().ok_or("missing bytes")?)}),
    );
    match args[0].as_str() {
        "family" => family(&document, &source, kind, repeats, directory),
        "engine" => engine(document, source, directory),
        "engine-cancel" => engine_cancel(document, source),
        "project" => save_project(document, source, directory, &args[1]),
        _ => Err("unknown mode".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs focused current private-index evidence without compiling unrelated historical lib tests.
    ///
    /// # Panics
    /// Panics on dense/sparse/oracle order, work, fallback, bounds, cancellation or fingerprint regressions.
    #[test]
    fn current_spatial_index_and_streaming_contracts() {
        patterns::random_spatial_index_evidence::traversal_and_work_match_oracle();
        patterns::random_spatial_index_evidence::fallback_limits_and_cancellation();
        patterns::random_spatial_index_evidence::fingerprint_and_order_match_previous_contract();
    }

    /// Verifies the probe's family projection matches repeatable current source-bound evaluation.
    /// Both immutable sources exercise current decode, domain resolution and weighted refinement.
    ///
    /// # Panics
    /// Panics on source/authority failures, unstable complete sites or ineffective cancellation/limits.
    #[test]
    fn current_controls_are_repeatable_source_bound_and_cancellable() {
        for source_name in ["png", "svg"] {
            let (document, source) = fixture(source_name, "even-random-circles", "1").unwrap();
            let field =
                toniator_sampling::decode_source(source.bytes().unwrap(), source.format().unwrap())
                    .unwrap();
            let definition = control("raw-lloyd-weighted", 0).unwrap();
            let channel = document.channel_topology().unwrap().channels()[0].id;
            let plan = patterns::resolve_pattern_pipeline_for_channel(
                &definition,
                document.channel_weighting(channel).unwrap(),
            )
            .unwrap();
            let canvas = CanvasSpec {
                width: 128.,
                height: 128.,
            };
            let mut request = GridInspectRequest {
                density: DensityMetric2D {
                    density: 16.,
                    aspect: 1.,
                }
                .resolve(&canvas)
                .unwrap(),
                canvas,
                rotation_degrees: 0.,
                translation_x: 0.,
                translation_y: 0.,
                guard_steps: 1,
                support_radius: 1.,
                max_family_candidates: usize::MAX,
            };
            let evaluate = |request: &GridInspectRequest, cancel: bool| {
                patterns::evaluate_typed_family_product_with_source_progress_cancellable(
                    &plan.family,
                    request,
                    Some(&field),
                    &|| cancel,
                    &|_, _| {},
                )
            };
            let first = evaluate(&request, false).unwrap();
            assert_eq!(first, evaluate(&request, false).unwrap());
            assert_eq!(
                evaluate(&request, true).unwrap_err().path(),
                "evaluation.cancelled"
            );
            request.max_family_candidates = 1;
            assert_eq!(
                evaluate(&request, false).unwrap_err().path(),
                "coverage.candidate_limit"
            );
        }
    }

    /// Verifies independently retained canonical bytes hash to the reported full-family digest.
    /// A unique P3 evidence path keeps repeated runs create-only and preserves older evidence.
    ///
    /// # Panics
    /// Panics on source/evaluator/I/O failures or disagreement between file bytes and streamed SHA256.
    #[test]
    fn full_family_file_is_an_independent_digest_witness() {
        let (document, source) = fixture("png", "source-weighted-dispersion-voronoi", "1").unwrap();
        let channel = document.channel_topology().unwrap().channels()[0].id;
        let definition = document.pattern_definition_for(channel).unwrap();
        let plan = patterns::resolve_pattern_pipeline_for_channel(
            definition,
            document.channel_weighting(channel).unwrap(),
        )
        .unwrap();
        let request = family_request(&document, definition, &plan, channel).unwrap();
        let field =
            toniator_sampling::decode_source(source.bytes().unwrap(), source.format().unwrap())
                .unwrap();
        let output = patterns::evaluate_typed_family_product_with_source_progress_cancellable(
            &plan.family,
            &request,
            Some(&field),
            &|| false,
            &|_, _| {},
        )
        .unwrap();
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/validation/performance-20261016/p3/test-family-{nonce}.bin"
        ));
        let digest = family_digest(&output, Some(&path)).unwrap();
        assert_eq!(digest, hash(&fs::read(&path).unwrap()));
        assert_eq!(digest, family_digest(&output, None).unwrap());
    }
}
