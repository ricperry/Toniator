use std::{fs, path::PathBuf};

use serde_json::Value;
use toniator_domain::{
    CanvasSpec, Document, PatternDefinition, PatternMechanismId, PatternOutputLayerId,
    PresetMetadata, PresetRecord, RandomSiteCharacter, SiteDensityModulation, SiteExclusionPolicy,
    SourceReference, SourceReferenceId,
};
use toniator_io::{
    EmbeddedSource, EmbeddedSourceFormat, SourceBundle, load, load_preset, save, save_preset,
};

/// Builds a current project with authored scatter spacing and one embedded source.
///
/// # Panics
/// Panics if current domain source, topology, or random definition invariants fail.
fn fixture() -> (Document, SourceBundle) {
    let source_id = SourceReferenceId::new("spatial-density-source").unwrap();
    let base = Document::new_default_document(
        CanvasSpec {
            width: 320.0,
            height: 180.0,
        },
        SourceReference::Assigned(source_id.clone()),
    )
    .unwrap();
    let mut definition = PatternDefinition::random_sites(
        base.pattern_settings().definition_id,
        "Authored scatter",
        PatternMechanismId(1),
        PatternMechanismId(2),
        PatternMechanismId(3),
        PatternMechanismId(4),
        PatternOutputLayerId(1),
        RandomSiteCharacter::Stratified { jitter: 0.4 },
        91,
        SiteDensityModulation::Uniform,
        SiteExclusionPolicy::None,
        16_000_000,
        16_000_000,
        base.pattern_definition_bundles()[0]
            .definition
            .coverage
            .clone(),
    );
    let toniator_domain::PatternMechanism::RandomSiteProcess {
        nominal_spacing, ..
    } = &mut definition.mechanisms[0]
    else {
        panic!("random process must be first");
    };
    *nominal_spacing = Some(13.25);
    let mut bundle = base.pattern_definition_bundles()[0].clone();
    bundle.definition = definition;
    let document = Document::with_source_and_topology(
        base.id(),
        base.canvas().clone(),
        base.source().clone(),
        vec![bundle],
        base.pattern_settings().clone(),
        base.channel_model().unwrap(),
        base.channel_topology().unwrap().clone(),
    )
    .unwrap();
    let source = EmbeddedSource::new(
        source_id,
        EmbeddedSourceFormat::Svg,
        b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"320\" height=\"180\"/>".to_vec(),
        Some("spatial.svg".into()),
    )
    .unwrap();
    (document, SourceBundle::new([source]).unwrap())
}

/// Keeps stage-owned persistence artifacts away from immutable sources and user files.
///
/// # Panics
/// Panics if the validation directory cannot be created.
fn validation_directory() -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/validation/ton018-spatial-density-20261009/io");
    fs::create_dir_all(&path).unwrap();
    path
}

/// Proves current project and standalone Pattern persistence keep authored spacing and
/// dimensionless Feature-size authority; an older Pattern version is rejected.
///
/// # Panics
/// Panics if current serialization, reconstruction, or version rejection fails.
#[test]
fn authored_scatter_spacing_round_trips_and_old_pattern_rejects() {
    let directory = validation_directory();
    let (document, sources) = fixture();
    assert_eq!(document.pattern_settings().density.density, 100.0);
    let project = directory.join("scatter.toniator");
    save(&project, &document, &sources).unwrap();
    let loaded = load(&project).unwrap();
    assert_eq!(loaded.document(), &document);
    assert_eq!(
        loaded.versions().document(),
        toniator_io::DOCUMENT_SCHEMA_VERSION
    );

    let preset = PresetRecord {
        saved_calibration: None,
        metadata: PresetMetadata {
            id: "spatial-density".into(),
            name: "Spatial density".into(),
            category: "Test".into(),
            description: "Authored spatial spacing".into(),
            thumbnail: None,
        },
        recipe: document
            .reconstruct_pattern_definition_recipe(document.pattern_settings().definition_id)
            .unwrap(),
    };
    let preset_path = directory.join("scatter.toniator-pattern");
    save_preset(&preset_path, &preset).unwrap();
    assert_eq!(load_preset(&preset_path).unwrap(), preset);
    let mut json: Value = serde_json::from_slice(&fs::read(&preset_path).unwrap()).unwrap();
    assert!(json.to_string().contains("13.25"));
    json["preset_format_version"] = Value::from(6);
    let obsolete = directory.join("obsolete-pattern-v6.toniator-pattern");
    fs::write(&obsolete, serde_json::to_vec(&json).unwrap()).unwrap();
    assert!(load_preset(&obsolete).is_err());
}
