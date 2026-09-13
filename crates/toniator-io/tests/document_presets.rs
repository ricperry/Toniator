use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::Value;
use toniator_domain::{
    ArtworkWeightResponse, AuthoredCurveSegment, AuthoredPoint2, AuthoredStructureDraft,
    AuthoredStructureKind, CanvasSpec, ChannelPatternLayoutDelta, ChannelTopology,
    ChannelTopologyTemplate, ConnectedGeometryResponse, DensityMetric2D, DensityMetricDelta2D,
    Document, DocumentCommand, DocumentConfiguration, DocumentId, GeneralizedSiteProductDraft,
    GuideDimensionDraft, HalftoneChannelModel, MarkOrientationDraft, PathStrokeStyle,
    PatternDefinition, PatternDefinitionBundle, PatternDefinitionId, PatternDefinitionRecipe,
    PatternMechanismId, PatternOutputLayerId, PatternOutputSettings, PatternStructureRecipe,
    SiteDensityModulation, SourceMapping, SourceMappingComponent, SourceReference,
    SourceReferenceId, TranslationEditedAxis,
};
use toniator_io::{
    DocumentPresetError, EmbeddedSource, EmbeddedSourceFormat, SourceBundle,
    capture_document_preset_destination, load_document_preset, save, save_document_preset,
};
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

/// Allocates one collision-resistant disposable test directory.
fn temporary_directory(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "toniator-document-presets-{name}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock follows epoch")
            .as_nanos()
    ))
}

/// Builds one referenced Curve Motif recipe with an authored open-path resource.
fn curve_motif_recipe() -> PatternDefinitionRecipe {
    let motif = AuthoredStructureDraft::new(
        AuthoredStructureKind::OpenPath,
        vec![
            AuthoredCurveSegment::Line {
                start: AuthoredPoint2 { x: 0.0, y: 0.0 },
                end: AuthoredPoint2 { x: 0.45, y: 0.2 },
            },
            AuthoredCurveSegment::Line {
                start: AuthoredPoint2 { x: 0.45, y: 0.2 },
                end: AuthoredPoint2 { x: 1.0, y: 0.0 },
            },
        ],
    )
    .expect("Curve Motif resource validates");
    PatternDefinitionRecipe::connected(PatternStructureRecipe::AuthoredResources {
        resources: vec![motif],
        definition: Box::new(PatternStructureRecipe::CurveMotifPaths {
            definition: Box::new(PatternStructureRecipe::GeneralizedStraightGuides {
                name: "Preset Curve Motif".into(),
                coverage: toniator_domain::CoveragePolicy {
                    guard_steps: 2,
                    additional_margin: 0.5,
                },
                dimensions: vec![GuideDimensionDraft {
                    baseline_angle_degrees: 0.0,
                    phase: 0.1,
                    spacing_multiplier: 1.0,
                }],
                product: GeneralizedSiteProductDraft::AlongGuides {
                    dimension_indices: vec![0],
                    interval_multiplier: 1.0,
                    phase: 0.2,
                },
                orientation: MarkOrientationDraft::GuideTangent { dimension_index: 0 },
            }),
            resource_index: 0,
            style: PathStrokeStyle::default(),
            mirror_alternate_rows: true,
            alternate_row_phase: Some(0.5),
        }),
    })
}

/// Builds varied current configuration with independent fill/weighting and inherited recipe intent.
fn modeled_document(
    model: HalftoneChannelModel,
    id: u64,
    canvas: CanvasSpec,
    source_name: &str,
) -> Document {
    let source = SourceReference::Assigned(
        SourceReferenceId::new(source_name.to_owned()).expect("source identity validates"),
    );
    let base = Document::new_default_document(canvas.clone(), source.clone())
        .expect("base document validates");
    let template = ChannelTopologyTemplate {
        pattern_instance: base
            .channel_topology()
            .expect("base modeled topology")
            .channels()[0]
            .pattern_instance
            .clone(),
    };
    let mut channels = ChannelTopology::canonical(model, template)
        .expect("canonical topology validates")
        .channels()
        .to_vec();
    channels[0].mapping.gain = 0.75;
    channels[0].mapping.bias = 0.1;
    channels[0].mapping.tone = toniator_domain::SourceTone {
        black_point: 0.15,
        white_point: 0.85,
        gamma: 1.7,
        contrast: 1.2,
        cutoff: 0.25,
    };
    channels[0].visible = false;
    channels[0].opacity = 0.6;

    let mut bundles = base.pattern_definition_bundles().to_vec();
    if channels.len() > 1 {
        let random_id = PatternDefinitionId(20);
        let random_output = PatternOutputLayerId(24);
        let random = PatternDefinition::random_sites(
            random_id,
            "Preset random channel",
            PatternMechanismId(21),
            PatternMechanismId(22),
            PatternMechanismId(23),
            PatternMechanismId(25),
            random_output,
            toniator_domain::RandomSiteCharacter::Even {
                minimum_center_distance: 0.25,
            },
            17,
            SiteDensityModulation::ArtworkWeighted,
            toniator_domain::SiteExclusionPolicy::None,
            20_000,
            20_000,
            toniator_domain::CoveragePolicy {
                guard_steps: 3,
                additional_margin: 1.5,
            },
        );
        bundles.push(PatternDefinitionBundle {
            definition: random,
            output_settings: vec![PatternOutputSettings {
                output_layer_id: random_output,
                response: base.pattern_definition_bundles()[0].output_settings[0]
                    .response
                    .clone(),
            }],
        });
        let random_channel = channels.len() - 1;
        channels[random_channel].weighting = toniator_domain::SourceWeighting {
            mapping: SourceMapping {
                component: SourceMappingComponent::Luminance,
                placement: toniator_domain::SourcePlacement::StretchToCanvas,
                inverted: true,
                gain: 0.8,
                bias: 0.1,
                tone: toniator_domain::SourceTone::identity(),
            },
            strength: 0.7,
            response: ArtworkWeightResponse::Smoothstep,
        };
        channels[random_channel]
            .pattern_instance
            .definition_override = Some(random_id);
        channels[random_channel].pattern_instance.layout_delta = ChannelPatternLayoutDelta {
            density: Some(DensityMetricDelta2D {
                density_delta: 3.0,
                aspect_delta: 0.25,
            }),
            rotation_degrees: None,
            translation_x: 4.5,
            translation_y: -2.0,
        };
    }
    let mut document = Document::with_source_topology_and_authored_structures(
        DocumentId(id),
        canvas,
        source,
        bundles,
        base.pattern_settings().clone(),
        model,
        ChannelTopology::new(channels),
        Vec::new(),
    )
    .expect("varied modeled document validates");
    if document
        .channel_topology()
        .expect("modeled topology")
        .channels()
        .len()
        > 1
    {
        let channel_id = document
            .channel_topology()
            .expect("modeled topology")
            .channels()[1]
            .id;
        let base_definition = document.pattern_definition_bundles()[0].definition.clone();
        (document, _) = document
            .apply_command(
                &DocumentCommand::ReplaceChannelPatternDefinitionOverrideRecipe {
                    base: document.pattern_settings().clone(),
                    channel_id,
                    base_definition,
                    recipe: curve_motif_recipe(),
                },
            )
            .expect("Curve Motif channel materializes");
        let density = DensityMetric2D {
            density: document.pattern_settings().density.density + 2.0,
            aspect: document.pattern_settings().density.aspect + 0.2,
        };
        let command = document
            .set_channel_density_for_effective(channel_id, density)
            .expect("Curve Motif density command builds");
        (document, _) = document
            .apply_command(&command)
            .expect("Curve Motif density applies");
        (document, _) = document
            .apply_command(&DocumentCommand::SetTranslationAxis {
                channel_id,
                edited_axis: TranslationEditedAxis::X,
                value: 3.25,
            })
            .expect("Curve Motif translation applies");
        let output_id = document
            .effective_channel_pattern(channel_id)
            .expect("Curve Motif resolves")
            .output_settings[0]
            .output_layer_id;
        let response = document
            .set_channel_output_response_for_effective(
                channel_id,
                output_id,
                toniator_domain::PatternGeometryResponse::Connected(ConnectedGeometryResponse {
                    minimum_thickness: 0.05,
                    maximum_thickness: 0.85,
                    bias: -0.02,
                }),
            )
            .expect("Curve Motif output response command builds");
        (document, _) = document
            .apply_command(&response)
            .expect("Curve Motif output response applies");
    }
    document
}

/// Builds the exact source bundle for one source-backed document witness.
fn source_bundle(
    document: &Document,
    format: EmbeddedSourceFormat,
    bytes: &[u8],
    display_name: &str,
) -> SourceBundle {
    let SourceReference::Assigned(id) = document.source() else {
        panic!("test document has assigned source")
    };
    SourceBundle::new([EmbeddedSource::new(
        id.clone(),
        format,
        bytes.to_vec(),
        Some(display_name.to_owned()),
    )
    .expect("embedded source validates")])
    .expect("source bundle validates")
}

/// Reads one archive member without extracting filesystem content.
fn archive_entry(path: &Path, name: &str) -> Vec<u8> {
    let mut archive =
        ZipArchive::new(File::open(path).expect("archive opens")).expect("archive is ZIP");
    let mut bytes = Vec::new();
    archive
        .by_name(name)
        .expect("named member exists")
        .read_to_end(&mut bytes)
        .expect("member reads");
    bytes
}

/// Writes deterministic test-only archive entries with explicit compression.
fn write_archive(path: &Path, entries: &[(&str, &[u8], CompressionMethod)]) {
    let options = |method| {
        SimpleFileOptions::default()
            .compression_method(method)
            .last_modified_time(zip::DateTime::default())
            .unix_permissions(0o100600)
    };
    let mut writer = ZipWriter::new(File::create(path).expect("archive creates"));
    for (name, bytes, method) in entries {
        writer
            .start_file(*name, options(*method))
            .expect("member starts");
        writer.write_all(bytes).expect("member writes");
    }
    writer.finish().expect("archive finishes");
}

/// Rewrites a project while corrupting only its embedded source bytes.
fn corrupt_project_source(path: &Path) {
    let file = File::open(path).expect("project opens");
    let mut archive = ZipArchive::new(file).expect("project is ZIP");
    let mut entries = Vec::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).expect("project member opens");
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).expect("project member reads");
        if entry.name().starts_with("sources/") {
            bytes[0] ^= 0xff;
        }
        entries.push((entry.name().to_owned(), bytes));
    }
    drop(archive);
    let borrowed = entries
        .iter()
        .map(|(name, bytes)| (name.as_str(), bytes.as_slice(), CompressionMethod::Stored))
        .collect::<Vec<_>>();
    write_archive(path, &borrowed);
}

/// Proves all current channel models and exact authored intent survive source-free round trips.
///
/// # Panics
///
/// Panics when deterministic archive shape changes, excluded project/source
/// fields leak, a model fails to round trip, or destination authority changes.
#[test]
fn source_free_presets_round_trip_all_models_and_exclude_project_authority() {
    let directory = temporary_directory("round-trip");
    fs::create_dir(&directory).expect("test directory creates");
    for (index, model) in [
        HalftoneChannelModel::Rgb,
        HalftoneChannelModel::Cmyk,
        HalftoneChannelModel::SourceColorAlpha,
    ]
    .into_iter()
    .enumerate()
    {
        let source = modeled_document(
            model,
            100 + index as u64,
            CanvasSpec {
                width: 900.0,
                height: 620.0,
            },
            &format!("preset-source-{index}"),
        );
        let destination = modeled_document(
            HalftoneChannelModel::Rgb,
            900 + index as u64,
            CanvasSpec {
                width: 1024.0,
                height: 512.0,
            },
            &format!("destination-source-{index}"),
        );
        let configuration = DocumentConfiguration::capture(&source);
        let path = directory.join(format!("model-{index}.toniator-preset"));
        let guard = capture_document_preset_destination(&path).expect("missing guard captures");
        assert!(!guard.is_existing());
        save_document_preset(&path, &configuration, &guard).expect("Preset saves");

        let mut archive =
            ZipArchive::new(File::open(&path).expect("Preset opens")).expect("Preset is ZIP");
        assert_eq!(archive.len(), 1);
        assert_eq!(
            archive.by_index(0).expect("member opens").name(),
            "preset.json"
        );
        drop(archive);
        let root: Value = serde_json::from_slice(&archive_entry(&path, "preset.json"))
            .expect("Preset JSON parses");
        assert_eq!(root["kind"], "document_preset");
        assert_eq!(root["document_preset_format_version"], 3);
        assert_eq!(
            root["document_schema_version"],
            toniator_io::DOCUMENT_SCHEMA_VERSION
        );
        let configuration_json = root["configuration"]
            .as_object()
            .expect("configuration object");
        assert_eq!(
            configuration_json.len(),
            if source.authored_structures().is_empty() {
                3
            } else {
                4
            }
        );
        assert_eq!(
            configuration_json.contains_key("authored_structures"),
            !source.authored_structures().is_empty()
        );
        for excluded in ["id", "canvas", "source_reference_id", "source", "revision"] {
            assert!(root.get(excluded).is_none());
            assert!(configuration_json.get(excluded).is_none());
        }

        let loaded = load_document_preset(&path, &destination).expect("Preset loads");
        assert_eq!(loaded, configuration);
        let bound = loaded.bind(&destination).expect("loaded Preset binds");
        assert_eq!(bound.id(), destination.id());
        assert_eq!(bound.canvas(), destination.canvas());
        assert_eq!(bound.source(), destination.source());
    }
    fs::remove_dir_all(directory).expect("test directory removes");
}

/// Proves renamed projects use ordinary complete source-integrity validation before capture.
///
/// Both immutable PNG and SVG inputs are exercised as project-as-Preset witnesses.
///
/// # Panics
///
/// Panics when a valid renamed project differs from direct configuration
/// capture, or when corrupted source bytes are accepted.
#[test]
fn project_as_preset_validates_immutable_png_and_svg_sources() {
    let directory = temporary_directory("project-input");
    fs::create_dir(&directory).expect("test directory creates");
    let destination = modeled_document(
        HalftoneChannelModel::Rgb,
        400,
        CanvasSpec {
            width: 333.0,
            height: 777.0,
        },
        "destination-source",
    );
    for (index, format, bytes, display_name) in [
        (
            0,
            EmbeddedSourceFormat::Png,
            &include_bytes!("../../../assets/raster-sample.png")[..],
            "raster-sample.png",
        ),
        (
            1,
            EmbeddedSourceFormat::Svg,
            &include_bytes!("../../../assets/vector-sample.svg")[..],
            "vector-sample.svg",
        ),
    ] {
        let project = modeled_document(
            HalftoneChannelModel::Cmyk,
            500 + index,
            CanvasSpec {
                width: 900.0,
                height: 620.0,
            },
            &format!("project-source-{index}"),
        );
        let sources = source_bundle(&project, format, bytes, display_name);
        let path = directory.join(format!("renamed-{index}.toniator-preset"));
        save(&path, &project, &sources).expect("project saves under renamed extension");
        let loaded = load_document_preset(&path, &destination).expect("renamed project loads");
        assert_eq!(loaded, DocumentConfiguration::capture(&project));

        corrupt_project_source(&path);
        let error = load_document_preset(&path, &destination)
            .expect_err("source digest corruption rejects");
        assert!(matches!(error, DocumentPresetError::Project { .. }));
        assert!(error.to_string().contains("SHA-256"));
    }
    fs::remove_dir_all(directory).expect("test directory removes");
}

/// Proves unknown fields are rejected recursively without changing ordinary project parsing.
///
/// # Panics
///
/// Panics when a nested source-like field or an envelope source field is
/// ignored by the new reader rather than reported with its path.
#[test]
fn preset_reader_rejects_unknown_fields_at_root_and_nested_paths() {
    let directory = temporary_directory("unknown-fields");
    fs::create_dir(&directory).expect("test directory creates");
    let document = modeled_document(
        HalftoneChannelModel::Cmyk,
        1,
        CanvasSpec {
            width: 640.0,
            height: 480.0,
        },
        "source",
    );
    let clean = directory.join("clean.toniator-preset");
    let guard = capture_document_preset_destination(&clean).expect("guard captures");
    save_document_preset(&clean, &DocumentConfiguration::capture(&document), &guard)
        .expect("clean Preset saves");
    let original = archive_entry(&clean, "preset.json");

    let mut nested: Value = serde_json::from_slice(&original).expect("JSON parses");
    nested["configuration"]["channel_configuration"]["channels"][0]
        .as_object_mut()
        .expect("channel object")
        .insert(
            "source_reference_id".into(),
            Value::String("forbidden".into()),
        );
    let nested_bytes = serde_json::to_vec(&nested).expect("nested JSON serializes");
    let nested_path = directory.join("nested.toniator-preset");
    write_archive(
        &nested_path,
        &[("preset.json", &nested_bytes, CompressionMethod::Stored)],
    );
    let nested_error =
        load_document_preset(&nested_path, &document).expect_err("nested unknown field rejects");
    assert!(nested_error.to_string().contains("source_reference_id"));

    let mut root: Value = serde_json::from_slice(&original).expect("JSON parses");
    root.as_object_mut()
        .expect("root object")
        .insert("source".into(), Value::String("forbidden".into()));
    let root_bytes = serde_json::to_vec(&root).expect("root JSON serializes");
    let root_path = directory.join("root.toniator-preset");
    write_archive(
        &root_path,
        &[("preset.json", &root_bytes, CompressionMethod::Stored)],
    );
    let root_error =
        load_document_preset(&root_path, &document).expect_err("root unknown field rejects");
    assert!(root_error.to_string().contains("source"));
    fs::remove_dir_all(directory).expect("test directory removes");
}

/// Proves archive shape, format versions, and expanded JSON bounds reject transactionally.
///
/// # Panics
///
/// Panics when an extra member, unknown version, or compressed oversized JSON
/// reaches domain binding.
#[test]
fn preset_reader_rejects_extra_entries_versions_and_expanded_size() {
    let directory = temporary_directory("format-errors");
    fs::create_dir(&directory).expect("test directory creates");
    let document = modeled_document(
        HalftoneChannelModel::Rgb,
        1,
        CanvasSpec {
            width: 100.0,
            height: 100.0,
        },
        "source",
    );
    let clean = directory.join("clean.toniator-preset");
    let guard = capture_document_preset_destination(&clean).expect("guard captures");
    save_document_preset(&clean, &DocumentConfiguration::capture(&document), &guard)
        .expect("clean Preset saves");
    let original = archive_entry(&clean, "preset.json");

    let extra = directory.join("extra.toniator-preset");
    write_archive(
        &extra,
        &[
            ("preset.json", &original, CompressionMethod::Stored),
            ("source.png", b"forbidden", CompressionMethod::Stored),
        ],
    );
    assert!(matches!(
        load_document_preset(&extra, &document),
        Err(DocumentPresetError::EntryTopology { .. })
    ));

    let wrong_kind_bytes = br#"{
        "kind":"structural_pattern",
        "document_preset_format_version":1,
        "document_schema_version":7,
        "configuration":"must not decode"
    }"#;
    let wrong_kind_path = directory.join("wrong-kind.toniator-preset");
    write_archive(
        &wrong_kind_path,
        &[("preset.json", wrong_kind_bytes, CompressionMethod::Stored)],
    );
    assert!(matches!(
        load_document_preset(&wrong_kind_path, &document),
        Err(DocumentPresetError::Kind { .. })
    ));

    let wrong_version_bytes = br#"{
        "kind":"document_preset",
        "document_preset_format_version":2,
        "document_schema_version":7,
        "configuration":"must not decode"
    }"#;
    let wrong_version_path = directory.join("dispatch-version.toniator-preset");
    write_archive(
        &wrong_version_path,
        &[(
            "preset.json",
            wrong_version_bytes,
            CompressionMethod::Stored,
        )],
    );
    assert!(matches!(
        load_document_preset(&wrong_version_path, &document),
        Err(DocumentPresetError::Version { .. })
    ));

    let mut version: Value = serde_json::from_slice(&original).expect("JSON parses");
    version["document_preset_format_version"] = Value::from(2);
    let version_bytes = serde_json::to_vec(&version).expect("version JSON serializes");
    let version_path = directory.join("version.toniator-preset");
    write_archive(
        &version_path,
        &[("preset.json", &version_bytes, CompressionMethod::Stored)],
    );
    assert!(matches!(
        load_document_preset(&version_path, &document),
        Err(DocumentPresetError::Version { .. })
    ));

    let oversized = vec![b' '; 4 * 1024 * 1024 + 1];
    let oversized_path = directory.join("oversized.toniator-preset");
    write_archive(
        &oversized_path,
        &[("preset.json", &oversized, CompressionMethod::Deflated)],
    );
    assert!(matches!(
        load_document_preset(&oversized_path, &document),
        Err(DocumentPresetError::Limits { .. })
    ));
    fs::remove_dir_all(directory).expect("test directory removes");
}

/// Proves create-only and stale-overwrite guards preserve another writer's bytes.
///
/// # Panics
///
/// Panics when late creation or external overwrite is replaced, when a fresh
/// overwrite fails, or when repeated serialization is nondeterministic.
#[test]
fn guarded_publication_is_create_only_stale_safe_and_deterministic() {
    let directory = temporary_directory("guarded-save");
    fs::create_dir(&directory).expect("test directory creates");
    let document = modeled_document(
        HalftoneChannelModel::SourceColorAlpha,
        1,
        CanvasSpec {
            width: 200.0,
            height: 300.0,
        },
        "source",
    );
    let configuration = DocumentConfiguration::capture(&document);
    let path = directory.join("guarded.toniator-preset");
    let missing = capture_document_preset_destination(&path).expect("missing guard captures");
    fs::write(&path, b"late writer").expect("late writer creates destination");
    assert!(matches!(
        save_document_preset(&path, &configuration, &missing),
        Err(DocumentPresetError::StaleDestination { .. })
    ));
    assert_eq!(fs::read(&path).expect("late bytes read"), b"late writer");

    fs::remove_file(&path).expect("late file removes");
    let missing = capture_document_preset_destination(&path).expect("missing guard captures");
    save_document_preset(&path, &configuration, &missing).expect("create-only save succeeds");
    let deterministic = fs::read(&path).expect("Preset reads");
    let existing = capture_document_preset_destination(&path).expect("existing guard captures");
    assert!(existing.is_existing());
    fs::write(&path, b"external replacement").expect("external writer replaces");
    assert!(matches!(
        save_document_preset(&path, &configuration, &existing),
        Err(DocumentPresetError::StaleDestination { .. })
    ));
    assert_eq!(
        fs::read(&path).expect("external bytes read"),
        b"external replacement"
    );

    fs::write(&path, &deterministic).expect("known Preset restores");
    let fresh = capture_document_preset_destination(&path).expect("fresh guard captures");
    save_document_preset(&path, &configuration, &fresh).expect("fresh overwrite succeeds");
    assert_eq!(fs::read(&path).expect("Preset rereads"), deterministic);
    fs::remove_dir_all(directory).expect("test directory removes");
}
