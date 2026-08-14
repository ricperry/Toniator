use sha2::{Digest, Sha256};
use std::process::Command;
use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use toniator_domain::{
    ArtworkWeightResponse, CanvasSpec, ChannelAppearance, ChannelId, ChannelPatternLayout,
    ChannelSourceMapping, ChannelState, ChannelTopologyTemplate, ColorComponent, ColorValue,
    CoveragePolicy, DensityEditedAxis, DensityMetric2D, Document, DocumentCommand,
    DocumentCommandFieldClassification, DocumentHistory, DocumentId, DocumentSession,
    GeneralizedSiteProduct, GuideDimensionId, HalftoneChannelModel, LegacyMappingFieldEdit,
    MarkGeometryResponse, MarkOrientation, MarkPrototype, ModeledMappingFieldEdit,
    PatternDefinition, PatternDefinitionEdit, PatternDefinitionId, PatternMechanismId,
    PatternOutputLayerId, PropertyDependency, PropertyDescriptor, PropertyFieldId,
    PropertyFieldValue, PropertyTarget, PropertyValueKind, RandomSiteCharacter,
    SiteDensityModulation, SiteExclusionPolicy, SourceComponent, SourceMapping,
    SourceMappingComponent, SourcePlacement, SourceReference, SourceReferenceId,
    StraightGuideDimension, StraightGuideRepetition, VisibleMarkSizingPolicy,
    property_field_contract,
};
use toniator_engine::{
    CanonicalMark, ChannelDiagnosticRequest, ChannelDiagnosticScheduler, EvaluationLimits,
    EvaluationRequest, EvaluationScheduler, GeometryOutput, RenderScene, ResolvedSource, SiteScope,
    SourceFormatHint, encode_png, evaluate_channel_diagnostic, evaluate_with_limits, write_svg,
};
use toniator_io::{EmbeddedSource, EmbeddedSourceFormat, SourceBundle, load, save};

#[test]
fn valid_document_exits_zero_and_reports_success() {
    let output = Command::new(env!("CARGO_BIN_EXE_toniator"))
        .args([
            "validate",
            "--canvas",
            "900x600",
            "--density-x",
            "90.0",
            "--density-y",
            "60.0",
            "--opacity",
            "0.75",
        ])
        .output()
        .expect("CLI runs");

    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stderr).expect("utf8 stderr"), "");
    assert!(
        String::from_utf8(output.stdout)
            .expect("utf8 stdout")
            .contains("valid document")
    );
}

#[test]
fn capabilities_uses_the_shared_schema_derived_headless_surface() {
    let output = Command::new(env!("CARGO_BIN_EXE_toniator"))
        .args(["capabilities", "--canvas", "900x600"])
        .output()
        .expect("CLI runs");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    assert!(stdout.starts_with("capabilities-v1\tcount="));
    let lines: Vec<_> = stdout.lines().collect();
    assert!(lines.len() > 10);
    assert!(lines.iter().skip(1).all(|line| {
        line.contains("field=")
            && line.contains("target=")
            && line.contains("command=")
            && line.contains("kind=")
            && line.contains("choices=")
            && line.contains("bounds=")
            && line.contains("reference=")
            && line.contains("invalidation=")
    }));
    assert!(lines.iter().any(|line| line.contains("ColorRed")));
    assert!(lines.iter().any(|line| line.contains("ModeledMappingGain")));
    let repeated = Command::new(env!("CARGO_BIN_EXE_toniator"))
        .args(["capabilities", "--canvas", "900x600"])
        .output()
        .expect("CLI repeats");
    assert_eq!(stdout.as_bytes(), repeated.stdout.as_slice());
}

#[test]
fn capabilities_loads_an_authoritative_container_without_variant_grammar() {
    let (document, sources) = parity_document(
        fs::read(baseline_path("raster-sample.png")).unwrap(),
        EmbeddedSourceFormat::Png,
        900.0,
        600.0,
        HalftoneChannelModel::Cmyk,
    );
    let directory = temporary_directory("capabilities-input");
    let path = directory.join("document.toniator");
    save(&path, &document, &sources).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_toniator"))
        .args(["capabilities", "--input", path.to_str().unwrap()])
        .output()
        .expect("CLI runs");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("ModeledMappingComponent"));
    assert!(stdout.contains("field=Paint"));
    assert!(!stdout.contains("LegacyMappingComponent"));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn capabilities_loaded_source_color_alpha_uses_sampled_paint_disclosure() {
    let (document, sources) = parity_document(
        fs::read(baseline_path("raster-sample.png")).unwrap(),
        EmbeddedSourceFormat::Png,
        900.0,
        600.0,
        HalftoneChannelModel::SourceColorAlpha,
    );
    let directory = temporary_directory("capabilities-source-color");
    let path = directory.join("document.toniator");
    save(&path, &document, &sources).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_toniator"))
        .args(["capabilities", "--input", path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Paint(SampledSource)"));
    assert!(stdout.contains("ModeledMappingComponent"));
    assert!(!stdout.contains("field=ColorRed"));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn capabilities_loaded_random_and_guide_documents_disclose_only_compatible_fields() {
    let (base, sources) = parity_document(
        fs::read(baseline_path("raster-sample.png")).unwrap(),
        EmbeddedSourceFormat::Png,
        900.0,
        600.0,
        HalftoneChannelModel::Rgb,
    );
    let random = PatternDefinition::random_sites(
        PatternDefinitionId(1),
        "typed",
        PatternMechanismId(10),
        PatternMechanismId(11),
        PatternMechanismId(12),
        PatternMechanismId(13),
        PatternOutputLayerId(1),
        RandomSiteCharacter::Clustered {
            cluster_density: 0.2,
            cluster_spread: 2.0,
            cluster_strength: 0.7,
        },
        7,
        SiteDensityModulation::ArtworkWeighted {
            mapping: SourceMapping {
                component: SourceMappingComponent::Luminance,
                placement: SourcePlacement::StretchToCanvas,
                inverted: false,
                gain: 1.0,
                bias: 0.0,
            },
            strength: 0.8,
            response: ArtworkWeightResponse::Smoothstep,
        },
        SiteExclusionPolicy::VisibleMarkMargin {
            margin: 0.5,
            sizing: VisibleMarkSizingPolicy::MaximumSupportRadius,
        },
        64,
        128,
        CoveragePolicy {
            guard_steps: 2,
            additional_margin: 4.5,
        },
    );
    let random_document = Document::with_source_and_topology(
        base.id(),
        base.canvas().clone(),
        base.source().clone(),
        vec![random],
        HalftoneChannelModel::Rgb,
        base.channel_topology().unwrap().clone(),
    )
    .unwrap();
    let guide = PatternDefinition::generalized_straight_guides(
        PatternDefinitionId(1),
        "typed",
        PatternMechanismId(20),
        PatternMechanismId(21),
        PatternOutputLayerId(1),
        vec![
            StraightGuideDimension {
                id: GuideDimensionId(30),
                baseline_angle_degrees: 0.0,
                phase: 0.0,
                repetition: StraightGuideRepetition {
                    spacing_multiplier: 1.0,
                },
            },
            StraightGuideDimension {
                id: GuideDimensionId(31),
                baseline_angle_degrees: 90.0,
                phase: 0.0,
                repetition: StraightGuideRepetition {
                    spacing_multiplier: 1.0,
                },
            },
        ],
        GeneralizedSiteProduct::Intersections {
            dimensions: vec![GuideDimensionId(30), GuideDimensionId(31)],
            merge_epsilon: 0.0,
        },
        MarkOrientation::GuideTangent {
            dimension_id: GuideDimensionId(30),
        },
        CoveragePolicy {
            guard_steps: 2,
            additional_margin: 4.5,
        },
    );
    let guide_document = Document::with_source_and_topology(
        base.id(),
        base.canvas().clone(),
        base.source().clone(),
        vec![guide],
        HalftoneChannelModel::Rgb,
        base.channel_topology().unwrap().clone(),
    )
    .unwrap();
    let directory = temporary_directory("capabilities-progressive");
    for (name, document, required, absent) in [
        (
            "random",
            random_document,
            vec![
                "RandomClusterDensity",
                "ArtworkWeightMappingGain",
                "VisibleMarkMargin",
            ],
            vec![
                "RandomEvenMinimumCenterDistance",
                "ExclusionMinimumCenterDistance",
                "OutputOrientationDimension",
            ],
        ),
        (
            "guide",
            guide_document,
            vec!["OutputOrientationDimension"],
            vec![
                "RandomClusterDensity",
                "ArtworkWeightMappingGain",
                "VisibleMarkMargin",
            ],
        ),
    ] {
        let path = directory.join(format!("{name}.toniator"));
        save(&path, &document, &sources).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_toniator"))
            .args(["capabilities", "--input", path.to_str().unwrap()])
            .output()
            .unwrap();
        assert!(output.status.success());
        let stdout = String::from_utf8(output.stdout).unwrap();
        for field in required {
            assert!(stdout.contains(field), "missing {field}");
        }
        for field in absent {
            assert!(!stdout.contains(field), "inactive {field}");
        }
        let repeat = Command::new(env!("CARGO_BIN_EXE_toniator"))
            .args(["capabilities", "--input", path.to_str().unwrap()])
            .output()
            .unwrap();
        assert_eq!(stdout.as_bytes(), repeat.stdout.as_slice());
    }
    fs::remove_dir_all(directory).unwrap();
}

fn descriptor_for(
    document: &Document,
    field: PropertyFieldId,
    target: PropertyTarget,
) -> PropertyDescriptor {
    document
        .property_descriptors()
        .into_iter()
        .find(|descriptor| descriptor.field == field && descriptor.target == target)
        .unwrap_or_else(|| panic!("missing active descriptor {field:?} at {target:?}"))
}

fn descriptor_scope(target: PropertyTarget) -> &'static str {
    match target {
        PropertyTarget::Document => "document",
        PropertyTarget::Channel(_) => "channel",
        PropertyTarget::Definition(_) => "definition",
        PropertyTarget::Mechanism(_, _) => "mechanism",
        PropertyTarget::OutputLayer(_, _) => "output-layer",
        PropertyTarget::GuideDimension(_, _, _) => "guide-dimension",
    }
}

fn assert_cli_descriptor_command_parity(
    history: &mut DocumentHistory,
    descriptor: &PropertyDescriptor,
    command: DocumentCommand,
    affected_channels: Vec<ChannelId>,
) {
    let classification = command.field_classification();
    let DocumentCommandFieldClassification::DescriptorBacked(projections) = classification else {
        panic!("CLI descriptor field must use a descriptor-backed command");
    };
    assert_eq!(projections.len(), 1);
    let projection = projections[0];
    assert_eq!(projection.field, descriptor.field);
    let value_kind_matches = matches!(
        (descriptor.value_kind, projection.value),
        (
            PropertyValueKind::FiniteF64,
            PropertyFieldValue::FiniteF64(_)
        ) | (PropertyValueKind::U32, PropertyFieldValue::U32(_))
            | (PropertyValueKind::Boolean, PropertyFieldValue::Boolean(_))
            | (
                PropertyValueKind::EnumChoice,
                PropertyFieldValue::EnumChoice(_)
            )
            | (
                PropertyValueKind::StableIdReference,
                PropertyFieldValue::StableIdReference
                    | PropertyFieldValue::StableIdReferenceCollection(_)
            )
    );
    assert!(value_kind_matches, "{descriptor:?}");
    let contract = property_field_contract(descriptor.field);
    assert_eq!(descriptor.command_kind(), contract.command_kind);
    assert_eq!(descriptor.invalidation, contract.invalidation);
    let result = history.apply(&command).unwrap();
    assert_eq!(result.affected_channels, affected_channels);
    assert_eq!(result.invalidation, descriptor.invalidation);
}

fn legacy_cli_parity_document() -> Document {
    let definition = PatternDefinition::supported_straight_grid(
        PatternDefinitionId(1),
        "legacy",
        PatternMechanismId(1),
        PatternMechanismId(2),
        PatternOutputLayerId(1),
        CoveragePolicy {
            guard_steps: 2,
            additional_margin: 4.5,
        },
    );
    Document::with_source(
        DocumentId(71),
        CanvasSpec {
            width: 90.0,
            height: 60.0,
        },
        SourceReference::Assigned(SourceReferenceId::new("legacy-cli-source").unwrap()),
        vec![definition],
        vec![ChannelState {
            id: ChannelId(1),
            pattern_definition_id: PatternDefinitionId(1),
            layout: ChannelPatternLayout {
                density: DensityMetric2D {
                    across_x: 9.0,
                    across_y: 6.0,
                    aspect_locked: true,
                },
                rotation_degrees: 0.0,
                translation_x: 0.0,
                translation_y: 0.0,
            },
            appearance: ChannelAppearance {
                visible: true,
                color: ColorValue {
                    red: 0.1,
                    green: 0.2,
                    blue: 0.3,
                    alpha: 1.0,
                },
                opacity: 1.0,
            },
            mark_geometry_response: MarkGeometryResponse {
                minimum_fill: 2.0,
                maximum_fill: 4.5,
                rotation_offset_degrees: 0.0,
            },
            source_mapping: ChannelSourceMapping {
                component: SourceComponent::Luminance,
                placement: SourcePlacement::StretchToCanvas,
            },
        }],
    )
    .unwrap()
}

fn modeled_cli_parity_document(model: HalftoneChannelModel) -> Document {
    parity_document(vec![0], EmbeddedSourceFormat::Png, 90.0, 60.0, model).0
}

fn structural_cli_parity_document(definition: PatternDefinition) -> Document {
    let base = modeled_cli_parity_document(HalftoneChannelModel::Rgb);
    Document::with_source_and_topology(
        base.id(),
        base.canvas().clone(),
        base.source().clone(),
        vec![definition],
        HalftoneChannelModel::Rgb,
        base.channel_topology().unwrap().clone(),
    )
    .unwrap()
}

#[test]
fn cli_headless_descriptor_commands_share_typed_contracts_and_fail_atomically() {
    let mut scopes = std::collections::BTreeSet::new();
    let mut kinds = std::collections::BTreeSet::new();
    let mut dependencies = std::collections::BTreeSet::new();
    let observe = |descriptor: &PropertyDescriptor,
                   scopes: &mut std::collections::BTreeSet<&'static str>,
                   kinds: &mut std::collections::BTreeSet<String>,
                   dependencies: &mut std::collections::BTreeSet<String>| {
        scopes.insert(descriptor_scope(descriptor.target));
        kinds.insert(format!("{:?}", descriptor.value_kind));
        dependencies.insert(format!("{:?}", descriptor.dependency));
    };

    let mut legacy =
        DocumentHistory::new(DocumentSession::new(legacy_cli_parity_document()).unwrap());
    let source = descriptor_for(
        legacy.document(),
        PropertyFieldId::SourceReference,
        PropertyTarget::Document,
    );
    observe(&source, &mut scopes, &mut kinds, &mut dependencies);
    assert_cli_descriptor_command_parity(
        &mut legacy,
        &source,
        DocumentCommand::SetSourceReference {
            source: SourceReference::Assigned(SourceReferenceId::new("legacy-cli-next").unwrap()),
        },
        vec![ChannelId(1)],
    );
    let coverage = descriptor_for(
        legacy.document(),
        PropertyFieldId::CoverageGuardSteps,
        PropertyTarget::Definition(PatternDefinitionId(1)),
    );
    observe(&coverage, &mut scopes, &mut kinds, &mut dependencies);
    let legacy_base = legacy.document().pattern_definitions()[0].clone();
    assert_cli_descriptor_command_parity(
        &mut legacy,
        &coverage,
        DocumentCommand::EditSharedPatternDefinition {
            definition_id: PatternDefinitionId(1),
            base_definition: legacy_base,
            edit: PatternDefinitionEdit::SetCoverageGuardSteps { guard_steps: 3 },
        },
        vec![ChannelId(1)],
    );
    let legacy_mapping = descriptor_for(
        legacy.document(),
        PropertyFieldId::LegacyMappingComponent,
        PropertyTarget::Channel(ChannelId(1)),
    );
    observe(&legacy_mapping, &mut scopes, &mut kinds, &mut dependencies);
    assert_cli_descriptor_command_parity(
        &mut legacy,
        &legacy_mapping,
        DocumentCommand::SetLegacyMappingField {
            channel_id: ChannelId(1),
            edit: LegacyMappingFieldEdit::Component(SourceComponent::Alpha),
        },
        vec![ChannelId(1)],
    );

    let mut rgb = DocumentHistory::new(
        DocumentSession::new(modeled_cli_parity_document(HalftoneChannelModel::Rgb)).unwrap(),
    );
    let color = descriptor_for(
        rgb.document(),
        PropertyFieldId::ColorBlue,
        PropertyTarget::Channel(ChannelId(1)),
    );
    observe(&color, &mut scopes, &mut kinds, &mut dependencies);
    assert_cli_descriptor_command_parity(
        &mut rgb,
        &color,
        DocumentCommand::SetColorComponent {
            channel_id: ChannelId(1),
            component: ColorComponent::Blue,
            value: 0.6,
        },
        vec![ChannelId(1)],
    );
    let modeled_mapping = descriptor_for(
        rgb.document(),
        PropertyFieldId::ModeledMappingGain,
        PropertyTarget::Channel(ChannelId(1)),
    );
    observe(&modeled_mapping, &mut scopes, &mut kinds, &mut dependencies);
    assert_cli_descriptor_command_parity(
        &mut rgb,
        &modeled_mapping,
        DocumentCommand::SetModeledMappingField {
            channel_id: ChannelId(1),
            edit: ModeledMappingFieldEdit::Gain(0.5),
        },
        vec![ChannelId(1)],
    );
    let opacity = descriptor_for(
        rgb.document(),
        PropertyFieldId::Opacity,
        PropertyTarget::Channel(ChannelId(1)),
    );
    observe(&opacity, &mut scopes, &mut kinds, &mut dependencies);
    assert_cli_descriptor_command_parity(
        &mut rgb,
        &opacity,
        DocumentCommand::SetOpacity {
            channel_id: ChannelId(1),
            opacity: 0.5,
        },
        vec![ChannelId(1)],
    );

    let mut cmyk = DocumentHistory::new(
        DocumentSession::new(modeled_cli_parity_document(HalftoneChannelModel::Cmyk)).unwrap(),
    );
    let visibility = descriptor_for(
        cmyk.document(),
        PropertyFieldId::Visibility,
        PropertyTarget::Channel(ChannelId(4)),
    );
    observe(&visibility, &mut scopes, &mut kinds, &mut dependencies);
    assert_cli_descriptor_command_parity(
        &mut cmyk,
        &visibility,
        DocumentCommand::SetVisibility {
            channel_id: ChannelId(4),
            visible: false,
        },
        vec![ChannelId(4)],
    );

    let mut source_color = DocumentHistory::new(
        DocumentSession::new(modeled_cli_parity_document(
            HalftoneChannelModel::SourceColorAlpha,
        ))
        .unwrap(),
    );
    let paint = descriptor_for(
        source_color.document(),
        PropertyFieldId::Paint,
        PropertyTarget::Channel(ChannelId(8)),
    );
    observe(&paint, &mut scopes, &mut kinds, &mut dependencies);
    assert_eq!(paint.choices.len(), 1);
    let source_color_before = source_color.document().clone();
    let source_color_revision = source_color.revision();
    assert!(
        source_color
            .apply(&DocumentCommand::SetChannelPaint {
                channel_id: ChannelId(8),
                paint: toniator_domain::ChannelPaint::Solid(ColorValue {
                    red: 0.0,
                    green: 0.0,
                    blue: 0.0,
                    alpha: 1.0
                })
            })
            .is_err()
    );
    assert_eq!(source_color.document(), &source_color_before);
    assert_eq!(source_color.revision(), source_color_revision);
    let source_mapping = descriptor_for(
        source_color.document(),
        PropertyFieldId::ModeledMappingInverted,
        PropertyTarget::Channel(ChannelId(8)),
    );
    observe(&source_mapping, &mut scopes, &mut kinds, &mut dependencies);
    assert_cli_descriptor_command_parity(
        &mut source_color,
        &source_mapping,
        DocumentCommand::SetModeledMappingField {
            channel_id: ChannelId(8),
            edit: ModeledMappingFieldEdit::Inverted(true),
        },
        vec![ChannelId(8)],
    );

    let guide = PatternDefinition::generalized_straight_guides(
        PatternDefinitionId(1),
        "guide",
        PatternMechanismId(20),
        PatternMechanismId(21),
        PatternOutputLayerId(1),
        vec![
            StraightGuideDimension {
                id: GuideDimensionId(30),
                baseline_angle_degrees: 0.0,
                phase: 0.0,
                repetition: StraightGuideRepetition {
                    spacing_multiplier: 1.0,
                },
            },
            StraightGuideDimension {
                id: GuideDimensionId(31),
                baseline_angle_degrees: 90.0,
                phase: 0.0,
                repetition: StraightGuideRepetition {
                    spacing_multiplier: 1.0,
                },
            },
            StraightGuideDimension {
                id: GuideDimensionId(32),
                baseline_angle_degrees: 45.0,
                phase: 0.0,
                repetition: StraightGuideRepetition {
                    spacing_multiplier: 1.0,
                },
            },
        ],
        GeneralizedSiteProduct::Intersections {
            dimensions: vec![
                GuideDimensionId(30),
                GuideDimensionId(31),
                GuideDimensionId(32),
            ],
            merge_epsilon: 0.0,
        },
        MarkOrientation::GuideTangent {
            dimension_id: GuideDimensionId(30),
        },
        CoveragePolicy {
            guard_steps: 2,
            additional_margin: 4.5,
        },
    );
    let mut guide =
        DocumentHistory::new(DocumentSession::new(structural_cli_parity_document(guide)).unwrap());
    let guide_phase = descriptor_for(
        guide.document(),
        PropertyFieldId::GuidePhase,
        PropertyTarget::GuideDimension(
            PatternDefinitionId(1),
            PatternMechanismId(20),
            GuideDimensionId(30),
        ),
    );
    observe(&guide_phase, &mut scopes, &mut kinds, &mut dependencies);
    let guide_base = guide.document().pattern_definitions()[0].clone();
    assert_cli_descriptor_command_parity(
        &mut guide,
        &guide_phase,
        DocumentCommand::EditSharedPatternDefinition {
            definition_id: PatternDefinitionId(1),
            base_definition: guide_base,
            edit: PatternDefinitionEdit::SetGuidePhase {
                mechanism_id: PatternMechanismId(20),
                dimension_id: GuideDimensionId(30),
                phase: 0.25,
            },
        },
        vec![ChannelId(1), ChannelId(2), ChannelId(3)],
    );
    let intersections = descriptor_for(
        guide.document(),
        PropertyFieldId::IntersectionDimensions,
        PropertyTarget::Mechanism(PatternDefinitionId(1), PatternMechanismId(21)),
    );
    observe(&intersections, &mut scopes, &mut kinds, &mut dependencies);
    let guide_base = guide.document().pattern_definitions()[0].clone();
    assert_cli_descriptor_command_parity(
        &mut guide,
        &intersections,
        DocumentCommand::EditSharedPatternDefinition {
            definition_id: PatternDefinitionId(1),
            base_definition: guide_base,
            edit: PatternDefinitionEdit::SetIntersectionDimensions {
                mechanism_id: PatternMechanismId(21),
                dimensions: vec![GuideDimensionId(30), GuideDimensionId(31)],
            },
        },
        vec![ChannelId(1), ChannelId(2), ChannelId(3)],
    );
    let orientation_dimension = descriptor_for(
        guide.document(),
        PropertyFieldId::OutputOrientationDimension,
        PropertyTarget::OutputLayer(PatternDefinitionId(1), PatternOutputLayerId(1)),
    );
    observe(
        &orientation_dimension,
        &mut scopes,
        &mut kinds,
        &mut dependencies,
    );
    let guide_base = guide.document().pattern_definitions()[0].clone();
    assert_cli_descriptor_command_parity(
        &mut guide,
        &orientation_dimension,
        DocumentCommand::EditSharedPatternDefinition {
            definition_id: PatternDefinitionId(1),
            base_definition: guide_base,
            edit: PatternDefinitionEdit::SetOutputOrientationDimension {
                output_layer_id: PatternOutputLayerId(1),
                dimension_id: GuideDimensionId(31),
            },
        },
        vec![ChannelId(1), ChannelId(2), ChannelId(3)],
    );
    let orientation = descriptor_for(
        guide.document(),
        PropertyFieldId::OutputOrientation,
        PropertyTarget::OutputLayer(PatternDefinitionId(1), PatternOutputLayerId(1)),
    );
    observe(&orientation, &mut scopes, &mut kinds, &mut dependencies);
    let guide_base = guide.document().pattern_definitions()[0].clone();
    assert_cli_descriptor_command_parity(
        &mut guide,
        &orientation,
        DocumentCommand::EditSharedPatternDefinition {
            definition_id: PatternDefinitionId(1),
            base_definition: guide_base,
            edit: PatternDefinitionEdit::SetOutputOrientation {
                output_layer_id: PatternOutputLayerId(1),
                orientation: MarkOrientation::GuideNormal {
                    dimension_id: GuideDimensionId(31),
                },
            },
        },
        vec![ChannelId(1), ChannelId(2), ChannelId(3)],
    );

    let along = PatternDefinition::generalized_straight_guides(
        PatternDefinitionId(1),
        "along",
        PatternMechanismId(20),
        PatternMechanismId(21),
        PatternOutputLayerId(1),
        vec![
            StraightGuideDimension {
                id: GuideDimensionId(30),
                baseline_angle_degrees: 0.0,
                phase: 0.0,
                repetition: StraightGuideRepetition {
                    spacing_multiplier: 1.0,
                },
            },
            StraightGuideDimension {
                id: GuideDimensionId(31),
                baseline_angle_degrees: 90.0,
                phase: 0.0,
                repetition: StraightGuideRepetition {
                    spacing_multiplier: 1.0,
                },
            },
        ],
        GeneralizedSiteProduct::AlongGuides {
            dimensions: vec![GuideDimensionId(30)],
            interval_multiplier: 1.0,
            phase: 0.0,
        },
        MarkOrientation::Fixed,
        CoveragePolicy {
            guard_steps: 2,
            additional_margin: 4.5,
        },
    );
    let mut along =
        DocumentHistory::new(DocumentSession::new(structural_cli_parity_document(along)).unwrap());
    let along_phase = descriptor_for(
        along.document(),
        PropertyFieldId::AlongGuidePhase,
        PropertyTarget::Mechanism(PatternDefinitionId(1), PatternMechanismId(21)),
    );
    observe(&along_phase, &mut scopes, &mut kinds, &mut dependencies);
    let along_base = along.document().pattern_definitions()[0].clone();
    assert_cli_descriptor_command_parity(
        &mut along,
        &along_phase,
        DocumentCommand::EditSharedPatternDefinition {
            definition_id: PatternDefinitionId(1),
            base_definition: along_base,
            edit: PatternDefinitionEdit::SetAlongGuidePhase {
                mechanism_id: PatternMechanismId(21),
                phase: 0.25,
            },
        },
        vec![ChannelId(1), ChannelId(2), ChannelId(3)],
    );

    let clustered_weighted_visible = PatternDefinition::random_sites(
        PatternDefinitionId(1),
        "random",
        PatternMechanismId(10),
        PatternMechanismId(11),
        PatternMechanismId(12),
        PatternMechanismId(13),
        PatternOutputLayerId(1),
        RandomSiteCharacter::Clustered {
            cluster_density: 0.1,
            cluster_spread: 2.0,
            cluster_strength: 0.5,
        },
        7,
        SiteDensityModulation::ArtworkWeighted {
            mapping: SourceMapping::canonical(SourceMappingComponent::Luminance),
            strength: 0.5,
            response: ArtworkWeightResponse::Smoothstep,
        },
        SiteExclusionPolicy::VisibleMarkMargin {
            margin: 0.5,
            sizing: VisibleMarkSizingPolicy::MaximumSupportRadius,
        },
        64,
        128,
        CoveragePolicy {
            guard_steps: 2,
            additional_margin: 4.5,
        },
    );
    let mut random = DocumentHistory::new(
        DocumentSession::new(structural_cli_parity_document(clustered_weighted_visible)).unwrap(),
    );
    let random_seed = descriptor_for(
        random.document(),
        PropertyFieldId::RandomSeed,
        PropertyTarget::Mechanism(PatternDefinitionId(1), PatternMechanismId(10)),
    );
    observe(&random_seed, &mut scopes, &mut kinds, &mut dependencies);
    let random_base = random.document().pattern_definitions()[0].clone();
    assert_cli_descriptor_command_parity(
        &mut random,
        &random_seed,
        DocumentCommand::EditSharedPatternDefinition {
            definition_id: PatternDefinitionId(1),
            base_definition: random_base,
            edit: PatternDefinitionEdit::SetRandomSeed {
                mechanism_id: PatternMechanismId(10),
                seed: 8,
            },
        },
        vec![ChannelId(1), ChannelId(2), ChannelId(3)],
    );
    for (field, edit) in [
        (
            PropertyFieldId::RandomClusterStrength,
            PatternDefinitionEdit::SetRandomClusterStrength {
                mechanism_id: PatternMechanismId(10),
                cluster_strength: 0.6,
            },
        ),
        (
            PropertyFieldId::ArtworkWeightMappingGain,
            PatternDefinitionEdit::SetArtworkWeightMappingGain {
                mechanism_id: PatternMechanismId(11),
                gain: 0.5,
            },
        ),
        (
            PropertyFieldId::VisibleMarkMargin,
            PatternDefinitionEdit::SetVisibleMarkMargin {
                mechanism_id: PatternMechanismId(12),
                margin: 0.75,
            },
        ),
        (
            PropertyFieldId::RandomMaximumAttempts,
            PatternDefinitionEdit::SetRandomMaximumAttempts {
                mechanism_id: PatternMechanismId(13),
                maximum_attempts: 65,
            },
        ),
    ] {
        let descriptor = descriptor_for(
            random.document(),
            field,
            PropertyTarget::Mechanism(
                PatternDefinitionId(1),
                match field {
                    PropertyFieldId::RandomMaximumAttempts => PatternMechanismId(13),
                    PropertyFieldId::ArtworkWeightMappingGain => PatternMechanismId(11),
                    PropertyFieldId::VisibleMarkMargin => PatternMechanismId(12),
                    PropertyFieldId::RandomClusterStrength => PatternMechanismId(10),
                    _ => unreachable!(),
                },
            ),
        );
        observe(&descriptor, &mut scopes, &mut kinds, &mut dependencies);
        let base = random.document().pattern_definitions()[0].clone();
        assert_cli_descriptor_command_parity(
            &mut random,
            &descriptor,
            DocumentCommand::EditSharedPatternDefinition {
                definition_id: PatternDefinitionId(1),
                base_definition: base,
                edit,
            },
            vec![ChannelId(1), ChannelId(2), ChannelId(3)],
        );
    }
    let random_before = random.document().clone();
    let random_revision = random.revision();
    let base = random.document().pattern_definitions()[0].clone();
    assert!(
        random
            .apply(&DocumentCommand::EditSharedPatternDefinition {
                definition_id: PatternDefinitionId(1),
                base_definition: base,
                edit: PatternDefinitionEdit::SetArtworkWeightStrength {
                    mechanism_id: PatternMechanismId(11),
                    strength: f64::NAN
                }
            })
            .is_err()
    );
    assert_eq!(random.document(), &random_before);
    assert_eq!(random.revision(), random_revision);

    let even_minimum = PatternDefinition::random_sites(
        PatternDefinitionId(1),
        "even",
        PatternMechanismId(10),
        PatternMechanismId(11),
        PatternMechanismId(12),
        PatternMechanismId(13),
        PatternOutputLayerId(1),
        RandomSiteCharacter::Even {
            minimum_center_distance: 3.0,
        },
        8,
        SiteDensityModulation::Uniform,
        SiteExclusionPolicy::MinimumCenterDistance { minimum: 2.0 },
        64,
        128,
        CoveragePolicy {
            guard_steps: 2,
            additional_margin: 4.5,
        },
    );
    let mut even = DocumentHistory::new(
        DocumentSession::new(structural_cli_parity_document(even_minimum)).unwrap(),
    );
    for (field, mechanism_id, edit) in [
        (
            PropertyFieldId::RandomEvenMinimumCenterDistance,
            PatternMechanismId(10),
            PatternDefinitionEdit::SetRandomEvenMinimumCenterDistance {
                mechanism_id: PatternMechanismId(10),
                minimum_center_distance: 4.0,
            },
        ),
        (
            PropertyFieldId::ExclusionMinimumCenterDistance,
            PatternMechanismId(12),
            PatternDefinitionEdit::SetExclusionMinimumCenterDistance {
                mechanism_id: PatternMechanismId(12),
                minimum_center_distance: 3.0,
            },
        ),
    ] {
        let descriptor = descriptor_for(
            even.document(),
            field,
            PropertyTarget::Mechanism(PatternDefinitionId(1), mechanism_id),
        );
        observe(&descriptor, &mut scopes, &mut kinds, &mut dependencies);
        let base = even.document().pattern_definitions()[0].clone();
        assert_cli_descriptor_command_parity(
            &mut even,
            &descriptor,
            DocumentCommand::EditSharedPatternDefinition {
                definition_id: PatternDefinitionId(1),
                base_definition: base,
                edit,
            },
            vec![ChannelId(1), ChannelId(2), ChannelId(3)],
        );
    }

    assert_eq!(
        scopes,
        [
            "document",
            "channel",
            "definition",
            "mechanism",
            "output-layer",
            "guide-dimension"
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(
        kinds,
        [
            "FiniteF64",
            "U32",
            "Boolean",
            "StableIdReference",
            "EnumChoice"
        ]
        .into_iter()
        .map(String::from)
        .collect()
    );
    for dependency in [
        PropertyDependency::Always,
        PropertyDependency::ModeledChannel,
        PropertyDependency::SolidPaint,
        PropertyDependency::SampledPaint,
        PropertyDependency::StraightGuideDimension,
        PropertyDependency::IntersectionProduct,
        PropertyDependency::AlongGuideProduct,
        PropertyDependency::RandomProcess,
        PropertyDependency::ClusteredRandomProcess,
        PropertyDependency::ArtworkWeightedDensity,
        PropertyDependency::EvenRandomProcess,
        PropertyDependency::MinimumCenterExclusion,
        PropertyDependency::VisibleMarkExclusion,
        PropertyDependency::MarkPrototypeOutput,
        PropertyDependency::GuidedOutputOrientation,
    ] {
        assert!(
            dependencies.contains(&format!("{dependency:?}")),
            "missing {dependency:?}"
        );
    }
}

#[test]
#[ignore = "explicit Stage 17 native evidence generation"]
fn generate_stage17_low_resolution_evidence() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/validation/stage-17/generated/low-resolution");
    if root.exists() {
        fs::remove_dir_all(&root).unwrap();
    }
    fs::create_dir_all(&root).unwrap();
    let source_id = SourceReferenceId::new("stage17-evidence-source").unwrap();
    let bytes = fs::read(baseline_path("raster-sample.png")).unwrap();
    let sources = SourceBundle::new([EmbeddedSource::new(
        source_id.clone(),
        EmbeddedSourceFormat::Png,
        bytes,
        Some("raster-sample.png".into()),
    )
    .unwrap()])
    .unwrap();
    let document = Document::new_default_document(
        CanvasSpec {
            width: 96.0,
            height: 64.0,
        },
        SourceReference::Assigned(source_id),
    )
    .unwrap();
    let mut history =
        toniator_domain::DocumentHistory::new(DocumentSession::new(document).unwrap());
    let before = history.document().clone();
    history
        .apply(&toniator_domain::DocumentCommand::SetOpacity {
            channel_id: ChannelId(1),
            opacity: 0.5,
        })
        .unwrap();
    let applied = history.document().clone();
    history.undo().unwrap();
    let undone = history.document().clone();
    history.redo().unwrap();
    let redone = history.document().clone();
    assert_eq!(before, undone);
    assert_eq!(applied, redone);
    for (name, document) in [
        ("before", before),
        ("apply", applied),
        ("undo", undone),
        ("redo", redone),
    ] {
        let path = root.join(format!("{name}.toniator"));
        save(&path, &document, &sources).unwrap();
        let descriptors = document.property_descriptors();
        fs::write(
            root.join(format!("{name}.capabilities.txt")),
            descriptors
                .iter()
                .map(|descriptor| {
                    format!(
                        "{:?}\t{:?}\t{:?}\t{:?}\n",
                        descriptor.field,
                        descriptor.target,
                        descriptor.command_kind(),
                        descriptor.invalidation
                    )
                })
                .collect::<String>(),
        )
        .unwrap();
    }
    fs::write(
        root.join("manifest.txt"),
        "stage17 low-resolution typed-history evidence\ncontainers=before,apply,undo,redo\n",
    )
    .unwrap();

    let guide_base = history.document().clone();
    write_stage17_case(
        &root,
        &sources,
        &guide_base,
        "guide-intersection",
        PatternDefinition::generalized_straight_guides(
            PatternDefinitionId(100),
            "Stage 17 evidence guide intersections",
            PatternMechanismId(1000),
            PatternMechanismId(1001),
            PatternOutputLayerId(2000),
            vec![
                StraightGuideDimension {
                    id: GuideDimensionId(3000),
                    baseline_angle_degrees: 0.0,
                    phase: 0.0,
                    repetition: StraightGuideRepetition {
                        spacing_multiplier: 1.0,
                    },
                },
                StraightGuideDimension {
                    id: GuideDimensionId(3001),
                    baseline_angle_degrees: 90.0,
                    phase: 0.0,
                    repetition: StraightGuideRepetition {
                        spacing_multiplier: 1.0,
                    },
                },
            ],
            GeneralizedSiteProduct::Intersections {
                dimensions: vec![GuideDimensionId(3000), GuideDimensionId(3001)],
                merge_epsilon: 0.125,
            },
            MarkOrientation::Fixed,
            CoveragePolicy {
                guard_steps: 2,
                additional_margin: 4.5,
            },
        ),
        &[],
        &[],
        PatternDefinitionEdit::SetGuidePhase {
            mechanism_id: PatternMechanismId(1000),
            dimension_id: GuideDimensionId(3000),
            phase: 0.25,
        },
    );
    write_stage17_case(
        &root,
        &sources,
        &guide_base,
        "along-guide",
        PatternDefinition::generalized_straight_guides(
            PatternDefinitionId(101),
            "Stage 17 evidence along guide",
            PatternMechanismId(1010),
            PatternMechanismId(1011),
            PatternOutputLayerId(2001),
            vec![
                StraightGuideDimension {
                    id: GuideDimensionId(3010),
                    baseline_angle_degrees: 30.0,
                    phase: 0.0,
                    repetition: StraightGuideRepetition {
                        spacing_multiplier: 1.0,
                    },
                },
                StraightGuideDimension {
                    id: GuideDimensionId(3011),
                    baseline_angle_degrees: 120.0,
                    phase: 0.0,
                    repetition: StraightGuideRepetition {
                        spacing_multiplier: 1.5,
                    },
                },
            ],
            GeneralizedSiteProduct::AlongGuides {
                dimensions: vec![GuideDimensionId(3010)],
                interval_multiplier: 1.25,
                phase: 0.125,
            },
            MarkOrientation::GuideTangent {
                dimension_id: GuideDimensionId(3010),
            },
            CoveragePolicy {
                guard_steps: 2,
                additional_margin: 4.5,
            },
        ),
        &[],
        &[],
        PatternDefinitionEdit::SetAlongGuidePhase {
            mechanism_id: PatternMechanismId(1011),
            phase: 0.375,
        },
    );
    write_stage17_case(
        &root,
        &sources,
        &guide_base,
        "raw-random",
        PatternDefinition::random_sites(
            PatternDefinitionId(110),
            "Stage 17 evidence raw random",
            PatternMechanismId(1100),
            PatternMechanismId(1101),
            PatternMechanismId(1102),
            PatternMechanismId(1103),
            PatternOutputLayerId(2100),
            RandomSiteCharacter::RawUniform,
            0,
            SiteDensityModulation::Uniform,
            SiteExclusionPolicy::None,
            16,
            32,
            CoveragePolicy {
                guard_steps: 2,
                additional_margin: 4.5,
            },
        ),
        &[
            PatternDefinitionEdit::SetRandomSeed {
                mechanism_id: PatternMechanismId(1100),
                seed: 42,
            },
            PatternDefinitionEdit::SetRandomMaximumAttempts {
                mechanism_id: PatternMechanismId(1103),
                maximum_attempts: 64,
            },
        ],
        &[],
        PatternDefinitionEdit::SetRandomMaximumNeighborChecks {
            mechanism_id: PatternMechanismId(1103),
            maximum_neighbor_checks: 96,
        },
    );
    write_stage17_case(
        &root,
        &sources,
        &guide_base,
        "even-random",
        PatternDefinition::random_sites(
            PatternDefinitionId(111),
            "Stage 17 evidence even random",
            PatternMechanismId(1110),
            PatternMechanismId(1111),
            PatternMechanismId(1112),
            PatternMechanismId(1113),
            PatternOutputLayerId(2101),
            RandomSiteCharacter::Even {
                minimum_center_distance: 1.0,
            },
            7,
            SiteDensityModulation::Uniform,
            SiteExclusionPolicy::None,
            64,
            96,
            CoveragePolicy {
                guard_steps: 2,
                additional_margin: 4.5,
            },
        ),
        &[PatternDefinitionEdit::SetRandomSeed {
            mechanism_id: PatternMechanismId(1110),
            seed: 9,
        }],
        &[],
        PatternDefinitionEdit::SetRandomEvenMinimumCenterDistance {
            mechanism_id: PatternMechanismId(1110),
            minimum_center_distance: 2.0,
        },
    );
    write_stage17_case(
        &root,
        &sources,
        &guide_base,
        "clustered-random",
        PatternDefinition::random_sites(
            PatternDefinitionId(112),
            "Stage 17 evidence clustered random",
            PatternMechanismId(1120),
            PatternMechanismId(1121),
            PatternMechanismId(1122),
            PatternMechanismId(1123),
            PatternOutputLayerId(2102),
            RandomSiteCharacter::Clustered {
                cluster_density: 0.1,
                cluster_spread: 0.5,
                cluster_strength: 0.2,
            },
            1,
            SiteDensityModulation::Uniform,
            SiteExclusionPolicy::None,
            16,
            32,
            CoveragePolicy {
                guard_steps: 2,
                additional_margin: 4.5,
            },
        ),
        &[
            PatternDefinitionEdit::SetRandomSeed {
                mechanism_id: PatternMechanismId(1120),
                seed: 29,
            },
            PatternDefinitionEdit::SetRandomClusterDensity {
                mechanism_id: PatternMechanismId(1120),
                cluster_density: 0.25,
            },
            PatternDefinitionEdit::SetRandomClusterSpread {
                mechanism_id: PatternMechanismId(1120),
                cluster_spread: 1.25,
            },
            PatternDefinitionEdit::SetRandomClusterStrength {
                mechanism_id: PatternMechanismId(1120),
                cluster_strength: 0.8,
            },
            PatternDefinitionEdit::SetRandomMaximumAttempts {
                mechanism_id: PatternMechanismId(1123),
                maximum_attempts: 64,
            },
        ],
        &[],
        PatternDefinitionEdit::SetRandomMaximumNeighborChecks {
            mechanism_id: PatternMechanismId(1123),
            maximum_neighbor_checks: 96,
        },
    );
    write_stage17_case(
        &root,
        &sources,
        &guide_base,
        "artwork-weighted-random",
        PatternDefinition::random_sites(
            PatternDefinitionId(113),
            "Stage 17 evidence artwork weighted random",
            PatternMechanismId(1130),
            PatternMechanismId(1131),
            PatternMechanismId(1132),
            PatternMechanismId(1133),
            PatternOutputLayerId(2103),
            RandomSiteCharacter::RawUniform,
            2,
            SiteDensityModulation::ArtworkWeighted {
                mapping: SourceMapping::canonical(SourceMappingComponent::Luminance),
                strength: 0.2,
                response: ArtworkWeightResponse::Linear,
            },
            SiteExclusionPolicy::None,
            16,
            32,
            CoveragePolicy {
                guard_steps: 2,
                additional_margin: 4.5,
            },
        ),
        &[
            PatternDefinitionEdit::SetRandomSeed {
                mechanism_id: PatternMechanismId(1130),
                seed: 31,
            },
            PatternDefinitionEdit::SetArtworkWeightMappingComponent {
                mechanism_id: PatternMechanismId(1131),
                component: SourceMappingComponent::Blue,
            },
            PatternDefinitionEdit::SetArtworkWeightMappingInverted {
                mechanism_id: PatternMechanismId(1131),
                inverted: true,
            },
            PatternDefinitionEdit::SetArtworkWeightMappingGain {
                mechanism_id: PatternMechanismId(1131),
                gain: 1.25,
            },
            PatternDefinitionEdit::SetArtworkWeightMappingBias {
                mechanism_id: PatternMechanismId(1131),
                bias: -0.1,
            },
            PatternDefinitionEdit::SetArtworkWeightStrength {
                mechanism_id: PatternMechanismId(1131),
                strength: 0.8,
            },
            PatternDefinitionEdit::SetArtworkWeightResponse {
                mechanism_id: PatternMechanismId(1131),
                response: ArtworkWeightResponse::Smoothstep,
            },
            PatternDefinitionEdit::SetRandomMaximumAttempts {
                mechanism_id: PatternMechanismId(1133),
                maximum_attempts: 64,
            },
        ],
        &[PatternDefinitionEdit::SetArtworkWeightMappingPlacement {
            mechanism_id: PatternMechanismId(1131),
            placement: SourcePlacement::StretchToCanvas,
        }],
        PatternDefinitionEdit::SetRandomMaximumNeighborChecks {
            mechanism_id: PatternMechanismId(1133),
            maximum_neighbor_checks: 96,
        },
    );
    write_stage17_case(
        &root,
        &sources,
        &guide_base,
        "center-excluded-random",
        PatternDefinition::random_sites(
            PatternDefinitionId(114),
            "Stage 17 evidence center excluded random",
            PatternMechanismId(1140),
            PatternMechanismId(1141),
            PatternMechanismId(1142),
            PatternMechanismId(1143),
            PatternOutputLayerId(2104),
            RandomSiteCharacter::RawUniform,
            3,
            SiteDensityModulation::Uniform,
            SiteExclusionPolicy::None,
            16,
            32,
            CoveragePolicy {
                guard_steps: 2,
                additional_margin: 4.5,
            },
        ),
        &[
            PatternDefinitionEdit::SetRandomSeed {
                mechanism_id: PatternMechanismId(1140),
                seed: 37,
            },
            PatternDefinitionEdit::SetExclusionVariant {
                mechanism_id: PatternMechanismId(1142),
                policy: SiteExclusionPolicy::MinimumCenterDistance { minimum: 0.5 },
            },
            PatternDefinitionEdit::SetRandomMaximumAttempts {
                mechanism_id: PatternMechanismId(1143),
                maximum_attempts: 64,
            },
            PatternDefinitionEdit::SetRandomMaximumNeighborChecks {
                mechanism_id: PatternMechanismId(1143),
                maximum_neighbor_checks: 96,
            },
        ],
        &[
            PatternDefinitionEdit::SetOutputSiteProduct {
                output_layer_id: PatternOutputLayerId(2104),
                site_mechanism_id: PatternMechanismId(1143),
            },
            PatternDefinitionEdit::SetOutputMarkPrototype {
                output_layer_id: PatternOutputLayerId(2104),
                prototype: MarkPrototype::Circle,
            },
            PatternDefinitionEdit::SetOutputOrientation {
                output_layer_id: PatternOutputLayerId(2104),
                orientation: MarkOrientation::Fixed,
            },
        ],
        PatternDefinitionEdit::SetExclusionMinimumCenterDistance {
            mechanism_id: PatternMechanismId(1142),
            minimum_center_distance: 1.25,
        },
    );
    write_stage17_case(
        &root,
        &sources,
        &guide_base,
        "visible-mark-excluded-random",
        PatternDefinition::random_sites(
            PatternDefinitionId(115),
            "Stage 17 evidence visible mark excluded random",
            PatternMechanismId(1150),
            PatternMechanismId(1151),
            PatternMechanismId(1152),
            PatternMechanismId(1153),
            PatternOutputLayerId(2105),
            RandomSiteCharacter::RawUniform,
            4,
            SiteDensityModulation::Uniform,
            SiteExclusionPolicy::None,
            16,
            32,
            CoveragePolicy {
                guard_steps: 2,
                additional_margin: 4.5,
            },
        ),
        &[
            PatternDefinitionEdit::SetRandomSeed {
                mechanism_id: PatternMechanismId(1150),
                seed: 41,
            },
            PatternDefinitionEdit::SetExclusionVariant {
                mechanism_id: PatternMechanismId(1152),
                policy: SiteExclusionPolicy::VisibleMarkMargin {
                    margin: 0.25,
                    sizing: VisibleMarkSizingPolicy::MaximumSupportRadius,
                },
            },
            PatternDefinitionEdit::SetRandomMaximumAttempts {
                mechanism_id: PatternMechanismId(1153),
                maximum_attempts: 64,
            },
            PatternDefinitionEdit::SetRandomMaximumNeighborChecks {
                mechanism_id: PatternMechanismId(1153),
                maximum_neighbor_checks: 100_000,
            },
        ],
        &[
            PatternDefinitionEdit::SetVisibleMarkSizingPolicy {
                mechanism_id: PatternMechanismId(1152),
                sizing: VisibleMarkSizingPolicy::MaximumSupportRadius,
            },
            PatternDefinitionEdit::SetOutputSiteProduct {
                output_layer_id: PatternOutputLayerId(2105),
                site_mechanism_id: PatternMechanismId(1153),
            },
            PatternDefinitionEdit::SetOutputMarkPrototype {
                output_layer_id: PatternOutputLayerId(2105),
                prototype: MarkPrototype::Circle,
            },
            PatternDefinitionEdit::SetOutputOrientation {
                output_layer_id: PatternOutputLayerId(2105),
                orientation: MarkOrientation::Fixed,
            },
        ],
        PatternDefinitionEdit::SetVisibleMarkMargin {
            mechanism_id: PatternMechanismId(1152),
            margin: 0.75,
        },
    );
    write_stage17_low_resolution_source_model_matrix(&root);
    write_stage17_frozen_v1_parity(&root);
}

#[test]
#[ignore = "explicit Stage 17 natural-resolution evidence generation"]
fn generate_stage17_natural_resolution_evidence() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/validation/stage-17/generated/natural");
    if root.exists() {
        fs::remove_dir_all(&root).unwrap();
    }
    fs::create_dir_all(&root).unwrap();
    for (source_label, source_asset, format, width, height) in [
        (
            "raster",
            "raster-sample.png",
            EmbeddedSourceFormat::Png,
            1024.0,
            1024.0,
        ),
        (
            "vector",
            "vector-sample.svg",
            EmbeddedSourceFormat::Svg,
            900.0,
            620.0,
        ),
    ] {
        let bytes = fs::read(baseline_path(source_asset)).unwrap();
        for kind in [
            NaturalRandomKind::Raw,
            NaturalRandomKind::Even,
            NaturalRandomKind::Clustered,
        ] {
            write_stage17_natural_case(
                &root,
                source_label,
                source_asset,
                bytes.clone(),
                format,
                width,
                height,
                HalftoneChannelModel::Rgb,
                kind,
            );
        }
        for model in [
            HalftoneChannelModel::Rgb,
            HalftoneChannelModel::Cmyk,
            HalftoneChannelModel::SourceColorAlpha,
        ] {
            write_stage17_natural_case(
                &root,
                source_label,
                source_asset,
                bytes.clone(),
                format,
                width,
                height,
                model,
                NaturalRandomKind::ArtworkWeighted,
            );
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NaturalRandomKind {
    Raw,
    Even,
    Clustered,
    ArtworkWeighted,
}

impl NaturalRandomKind {
    const fn label(self) -> &'static str {
        match self {
            Self::Raw => "raw",
            Self::Even => "even",
            Self::Clustered => "clustered",
            Self::ArtworkWeighted => "artwork-weighted",
        }
    }

    const fn seed(self) -> u32 {
        match self {
            Self::Raw => 1_701,
            Self::Even => 1_702,
            Self::Clustered => 1_703,
            Self::ArtworkWeighted => 1_704,
        }
    }
}

#[allow(clippy::too_many_arguments)] // The immutable source and exact natural contract are explicit evidence inputs.
fn write_stage17_natural_case(
    root: &Path,
    source_label: &str,
    source_asset: &str,
    source_bytes: Vec<u8>,
    format: EmbeddedSourceFormat,
    width: f64,
    height: f64,
    model: HalftoneChannelModel,
    kind: NaturalRandomKind,
) {
    let case_root = root.join(format!(
        "{source_label}-{}-{}",
        kind.label(),
        model_label(model)
    ));
    fs::create_dir_all(&case_root).unwrap();
    let source_id = SourceReferenceId::new(format!(
        "stage17-natural-{source_label}-{}-{}",
        kind.label(),
        model_label(model)
    ))
    .unwrap();
    let document = Document::new_default_document(
        CanvasSpec { width, height },
        SourceReference::Assigned(source_id.clone()),
    )
    .unwrap();
    let sources = SourceBundle::new([EmbeddedSource::new(
        source_id,
        format,
        source_bytes.clone(),
        Some(source_asset.into()),
    )
    .unwrap()])
    .unwrap();
    let mut history = DocumentHistory::new(DocumentSession::new(document).unwrap());
    let mut commands = Vec::new();
    if model != HalftoneChannelModel::Rgb {
        let seed = history.document().channel_topology().unwrap().channels()[0].clone();
        let topology = history
            .document()
            .canonical_channel_topology(
                model,
                ChannelTopologyTemplate {
                    pattern_definition_id: seed.pattern_definition_id,
                    layout: seed.layout,
                    mark_geometry_response: seed.mark_geometry_response,
                },
            )
            .unwrap();
        commands.push(
            history
                .apply(&DocumentCommand::ReplaceChannelTopology { model, topology })
                .unwrap(),
        );
    }
    let target_density_x = if source_label == "raster" {
        102.0
    } else {
        90.0
    };
    let channel_ids = history
        .document()
        .channel_topology()
        .unwrap()
        .channels()
        .iter()
        .map(|channel| channel.id)
        .collect::<Vec<_>>();
    for channel_id in &channel_ids {
        if source_label == "raster" {
            commands.push(
                history
                    .apply(&DocumentCommand::SetDensityAxis {
                        channel_id: *channel_id,
                        edited_axis: DensityEditedAxis::AcrossX,
                        value: target_density_x,
                    })
                    .unwrap(),
            );
        } else {
            commands.push(
                history
                    .apply(&DocumentCommand::SetDensityAspectLock {
                        channel_id: *channel_id,
                        aspect_locked: false,
                    })
                    .unwrap(),
            );
        }
    }
    let definition = natural_random_definition(kind, PatternDefinitionId(700));
    commands.push(
        history
            .apply(&DocumentCommand::AddTypedPatternDefinition { definition })
            .unwrap(),
    );
    for channel_id in channel_ids {
        commands.push(
            history
                .apply(&DocumentCommand::RetargetChannelPatternDefinition {
                    channel_id,
                    definition_id: PatternDefinitionId(700),
                })
                .unwrap(),
        );
    }
    let applied = history.document().clone();
    let container = case_root.join("apply.toniator");
    save(&container, &applied, &sources).unwrap();
    let reopened = load(&container).unwrap();
    assert_eq!(reopened.document(), &applied);
    assert_eq!(
        reopened.sources().entries().next().unwrap().bytes(),
        source_bytes.as_slice()
    );
    fs::write(
        case_root.join("apply.capabilities.txt"),
        descriptor_text(&applied),
    )
    .unwrap();
    let mut manifest = format!(
        "stage17-natural-case={source_label}-{}-{}\nsource.asset={source_asset}\nsource.sha256={}\ncanvas={width}x{height}\ncommands={commands:?}\ndocument={applied:#?}\nsvg.font_caveat=Decoded SVG text pixels are font-dependent and no text-pixel golden is used.\n",
        kind.label(),
        model_label(model),
        sha256_hex(&source_bytes),
    );
    let scheduler = EvaluationScheduler::new().unwrap();
    let hashes =
        capture_stage17_evaluation(&case_root, "apply", &container, &scheduler, &mut manifest);
    scheduler.shutdown().unwrap();
    let metrics = natural_random_metrics(&container);
    manifest.push_str(&format!(
        "natural.metrics={metrics}\noutput.hashes={hashes:?}\n"
    ));
    assert_natural_native_checks(&case_root.join("apply.rgba"), width as u32, height as u32);
    if Command::new("xmllint")
        .args(["--noout", case_root.join("apply.svg").to_str().unwrap()])
        .output()
        .is_ok_and(|output| output.status.success())
    {
        manifest.push_str("svg.xmllint=passed\n");
    } else {
        manifest.push_str("svg.xmllint=unavailable-or-failed\n");
    }
    if (source_label, kind, model) == ("raster", NaturalRandomKind::Raw, HalftoneChannelModel::Rgb)
        || (source_label, kind, model)
            == (
                "vector",
                NaturalRandomKind::ArtworkWeighted,
                HalftoneChannelModel::SourceColorAlpha,
            )
    {
        assert_cli_container_render_parity(&case_root, &container);
        manifest.push_str("cli.container_render_parity=passed\n");
    }
    fs::write(case_root.join("manifest.txt"), manifest).unwrap();
}

fn model_label(model: HalftoneChannelModel) -> &'static str {
    match model {
        HalftoneChannelModel::Rgb => "rgb",
        HalftoneChannelModel::Cmyk => "cmyk",
        HalftoneChannelModel::SourceColorAlpha => "source-color-alpha",
    }
}

fn natural_random_definition(
    kind: NaturalRandomKind,
    id: PatternDefinitionId,
) -> PatternDefinition {
    let (character, density_modulation) = match kind {
        NaturalRandomKind::Raw => (
            RandomSiteCharacter::RawUniform,
            SiteDensityModulation::Uniform,
        ),
        NaturalRandomKind::Even => (
            RandomSiteCharacter::Even {
                minimum_center_distance: 8.0,
            },
            SiteDensityModulation::Uniform,
        ),
        NaturalRandomKind::Clustered => (
            RandomSiteCharacter::Clustered {
                cluster_density: 0.001,
                cluster_spread: 18.0,
                cluster_strength: 1.0,
            },
            SiteDensityModulation::Uniform,
        ),
        NaturalRandomKind::ArtworkWeighted => (
            RandomSiteCharacter::RawUniform,
            SiteDensityModulation::ArtworkWeighted {
                mapping: SourceMapping::canonical(SourceMappingComponent::Luminance),
                strength: 1.0,
                response: ArtworkWeightResponse::Smoothstep,
            },
        ),
    };
    PatternDefinition::random_sites(
        id,
        format!("stage17-natural-{}", kind.label()),
        PatternMechanismId(710),
        PatternMechanismId(711),
        PatternMechanismId(712),
        PatternMechanismId(713),
        PatternOutputLayerId(714),
        character,
        kind.seed(),
        density_modulation,
        SiteExclusionPolicy::None,
        64,
        16_000_000,
        CoveragePolicy {
            guard_steps: 2,
            additional_margin: 4.5,
        },
    )
}

#[allow(clippy::too_many_arguments)] // The case records independent typed accepted/rejected leaves.
fn write_stage17_case(
    root: &Path,
    sources: &SourceBundle,
    base: &Document,
    case_name: &str,
    definition: PatternDefinition,
    setup_edits: &[PatternDefinitionEdit],
    rejected_edits: &[PatternDefinitionEdit],
    leaf_edit: PatternDefinitionEdit,
) {
    let case_root = root.join(case_name);
    fs::create_dir_all(&case_root).unwrap();
    let channel_id = base
        .channel_topology()
        .map(|topology| topology.channels()[0].id)
        .or_else(|| base.channels().map(|channels| channels[0].id))
        .unwrap();
    let definition_id = definition.id;
    let mut history = DocumentHistory::new(DocumentSession::new(base.clone()).unwrap());
    let add = history
        .apply(&DocumentCommand::AddTypedPatternDefinition { definition })
        .unwrap();
    let retarget = history
        .apply(&DocumentCommand::RetargetChannelPatternDefinition {
            channel_id,
            definition_id,
        })
        .unwrap();
    let mut setup_results = Vec::new();
    for setup_edit in setup_edits {
        let base_definition = history
            .document()
            .pattern_definitions()
            .iter()
            .find(|candidate| candidate.id == definition_id)
            .unwrap()
            .clone();
        setup_results.push(
            history
                .apply(&DocumentCommand::EditSharedPatternDefinition {
                    definition_id,
                    base_definition,
                    edit: setup_edit.clone(),
                })
                .unwrap(),
        );
    }
    let mut rejected_results = Vec::new();
    for rejected_edit in rejected_edits {
        let before_rejection = history.document().clone();
        let revision = history.revision();
        let base_definition = before_rejection
            .pattern_definitions()
            .iter()
            .find(|candidate| candidate.id == definition_id)
            .unwrap()
            .clone();
        let error = history
            .apply(&DocumentCommand::EditSharedPatternDefinition {
                definition_id,
                base_definition,
                edit: rejected_edit.clone(),
            })
            .unwrap_err();
        assert_eq!(history.document(), &before_rejection);
        assert_eq!(history.revision(), revision);
        rejected_results.push(format!("{rejected_edit:?}: {error}"));
    }
    let before = history.document().clone();
    let edit = history
        .apply(&DocumentCommand::EditSharedPatternDefinition {
            definition_id,
            base_definition: before
                .pattern_definitions()
                .iter()
                .find(|candidate| candidate.id == definition_id)
                .unwrap()
                .clone(),
            edit: leaf_edit,
        })
        .unwrap();
    let applied = history.document().clone();
    history.undo().unwrap();
    let undone = history.document().clone();
    history.redo().unwrap();
    let redone = history.document().clone();
    assert_eq!(before, undone, "{case_name} leaf undo restores exactly");
    assert_eq!(applied, redone, "{case_name} leaf redo restores exactly");

    let mut manifest = format!(
        "stage17-low-resolution-case={case_name}\nsvg.font_caveat=Decoded SVG text pixels remain font-dependent and are not raster goldens.\ncommand.add={add:?}\ncommand.retarget={retarget:?}\ncommand.rejected={rejected_results:?}\ncommand.setup={setup_results:?}\ncommand.leaf={edit:?}\n"
    );
    let mut saved_hashes = Vec::new();
    let scheduler = EvaluationScheduler::new().unwrap();
    let mut output_hashes = Vec::new();
    for (state, document) in [
        ("before", before),
        ("apply", applied),
        ("undo", undone),
        ("redo", redone),
    ] {
        let container = case_root.join(format!("{state}.toniator"));
        save(&container, &document, sources).unwrap();
        let bytes = fs::read(&container).unwrap();
        let hash = sha256_hex(&bytes);
        saved_hashes.push((state, hash.clone()));
        manifest.push_str(&format!(
            "{state}.sha256={}\n{state}.bytes={}\n{state}.definitions={:#?}\n",
            hash,
            bytes.len(),
            document.pattern_definitions(),
        ));
        fs::write(
            case_root.join(format!("{state}.capabilities.txt")),
            document
                .property_descriptors()
                .iter()
                .map(|descriptor| {
                    format!(
                        "field={:?}\ttarget={:?}\tcommand={:?}\tkind={:?}\tdependency={:?}\tsupport={:?}\treference={:?}\tinvalidation={:?}\tcopy_escalates={}\n",
                        descriptor.field,
                        descriptor.target,
                        descriptor.command_kind(),
                        descriptor.value_kind,
                        descriptor.dependency,
                        descriptor.structural_support,
                        descriptor.reference_constraint,
                        descriptor.invalidation,
                        descriptor.copy_on_edit_escalates_to_family,
                    )
                })
                .collect::<String>(),
        )
        .unwrap();
        output_hashes.push((
            state,
            capture_stage17_evaluation(&case_root, state, &container, &scheduler, &mut manifest),
        ));
    }
    assert_eq!(
        saved_hashes[0].1, saved_hashes[2].1,
        "{case_name} undo bytes"
    );
    assert_eq!(
        saved_hashes[1].1, saved_hashes[3].1,
        "{case_name} redo bytes"
    );
    assert_eq!(
        output_hashes[0].1, output_hashes[2].1,
        "{case_name} undo canonical outputs"
    );
    assert_eq!(
        output_hashes[1].1, output_hashes[3].1,
        "{case_name} redo canonical outputs"
    );
    scheduler.shutdown().unwrap();
    fs::write(case_root.join("manifest.txt"), manifest).unwrap();
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Stage17OutputHashes {
    geometry: String,
    rgba: String,
    png: String,
    svg: String,
}

/// Evaluates one modeled Stage 17 fixture and writes deterministic derived evidence.
fn capture_stage17_evaluation(
    case_root: &Path,
    state: &str,
    container: &Path,
    scheduler: &EvaluationScheduler,
    manifest: &mut String,
) -> Stage17OutputHashes {
    let loaded = load(container).unwrap();
    let source = loaded.sources().entries().next().unwrap();
    let format = match source.format() {
        EmbeddedSourceFormat::Png => SourceFormatHint::Png,
        EmbeddedSourceFormat::Svg => SourceFormatHint::Svg,
    };
    if loaded.document().channel_topology().is_none() {
        return capture_stage17_legacy_evaluation(
            case_root,
            state,
            loaded.document(),
            source,
            format,
            manifest,
        );
    }
    let session = DocumentSession::new(loaded.document().clone()).unwrap();
    let request = EvaluationRequest::new(
        session.document_evaluation_snapshot(),
        ResolvedSource::new(source.id().clone(), source.bytes().to_vec(), format).unwrap(),
    );
    let ticket = scheduler.submit(request).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let completion = loop {
        if let Some(completion) = scheduler.try_receive_latest().unwrap() {
            break completion;
        }
        assert!(
            Instant::now() < deadline,
            "{state} scheduler completion timed out"
        );
        std::thread::sleep(Duration::from_millis(2));
    };
    assert_eq!(completion.ticket(), ticket);
    assert!(scheduler.accept_completion(&completion, &session).unwrap());
    if let Some(error) = completion.error() {
        panic!("{state} evaluation failed: {error}");
    }
    let diagnostics = completion.cache_diagnostics().unwrap();
    let result = completion.result().unwrap();
    let scene = result.scene();
    let raster = result.raster();
    assert_eq!(raster.width(), scene.canvas().width as u32);
    assert_eq!(raster.height(), scene.canvas().height as u32);
    let rgba = raster.pixels();
    let png = encode_png(raster).unwrap();
    let svg = write_svg(scene);
    let geometry = canonical_geometry_summary(scene);
    let raw_summary = raw_rgba_summary(raster.width(), raster.height(), rgba);
    let clip_count = svg.matches("<clipPath id=\"canvas-clip\">").count();
    let visible_marks = scene
        .layers()
        .iter()
        .filter(|layer| layer.visible())
        .map(|layer| match layer.geometry() {
            GeometryOutput::CircularMarks(marks) => marks.len(),
            GeometryOutput::CanonicalMarks(marks) => marks.len(),
        })
        .sum::<usize>();
    let circle_count = svg.matches("<circle ").count();
    let live_text_elements = svg.matches("<text").count();
    let positive_radii = scene
        .layers()
        .iter()
        .map(|layer| {
            match layer.geometry() {
            GeometryOutput::CircularMarks(marks) => (
                layer.channel_id(),
                marks.len(),
                marks.iter().filter(|mark| mark.radius > 0.0).count(),
            ),
            GeometryOutput::CanonicalMarks(marks) => (
                layer.channel_id(),
                marks.len(),
                marks
                    .iter()
                    .filter(|mark| {
                        matches!(mark, CanonicalMark::Circle { radius, .. } if *radius > 0.0)
                    })
                    .count(),
            ),
        }
        })
        .collect::<Vec<_>>();
    assert_eq!(clip_count, 1, "{state} SVG uses one canvas clip");
    assert_eq!(
        circle_count, visible_marks,
        "{state} SVG preserves editable circles"
    );

    fs::write(case_root.join(format!("{state}.rgba")), rgba).unwrap();
    fs::write(case_root.join(format!("{state}.png")), &png).unwrap();
    fs::write(case_root.join(format!("{state}.svg")), &svg).unwrap();
    fs::write(case_root.join(format!("{state}.geometry.txt")), &geometry).unwrap();
    let hashes = Stage17OutputHashes {
        geometry: sha256_hex(geometry.as_bytes()),
        rgba: sha256_hex(rgba),
        png: sha256_hex(&png),
        svg: sha256_hex(svg.as_bytes()),
    };
    manifest.push_str(&format!(
        "{state}.scheduler.ticket={}\n{state}.cache={diagnostics:?}\n{state}.source_identity={:?}\n{state}.channels={:?}\n{state}.scene.identity={:?}\n{state}.scene.layers={}\n{state}.scene.circular_marks={}\n{state}.scene.positive_radii={positive_radii:?}\n{state}.structural={}\n{state}.geometry.sha256={}\n{state}.rgba.sha256={}\n{state}.png.sha256={}\n{state}.svg.sha256={}\n{state}.svg.canvas_clips={clip_count}\n{state}.svg.editable_circles={circle_count}\n{state}.svg.live_text_elements={live_text_elements}\n{state}.rgba.summary={raw_summary}\n",
        ticket.value(),
        result.source_identity(),
        result.channels(),
        scene.identity(),
        scene.layers().len(),
        scene.circular_mark_count(),
        structural_product_summary(loaded.document(), source.bytes(), format),
        hashes.geometry,
        hashes.rgba,
        hashes.png,
        hashes.svg,
    ));
    hashes
}

/// Evaluates one retained legacy Stage 17 fixture through its diagnostic adapter.
fn capture_stage17_legacy_evaluation(
    case_root: &Path,
    state: &str,
    document: &Document,
    source: &EmbeddedSource,
    format: SourceFormatHint,
    manifest: &mut String,
) -> Stage17OutputHashes {
    let channel_id = document.channels().unwrap()[0].id;
    let session = DocumentSession::new(document.clone()).unwrap();
    let request = ChannelDiagnosticRequest::new(
        session.evaluation_snapshot(channel_id).unwrap(),
        ResolvedSource::new(source.id().clone(), source.bytes().to_vec(), format).unwrap(),
    );
    let scheduler = ChannelDiagnosticScheduler::new().unwrap();
    let ticket = scheduler.submit(request).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let completion = loop {
        if let Some(completion) = scheduler.try_receive_latest().unwrap() {
            break completion;
        }
        assert!(
            Instant::now() < deadline,
            "{state} legacy scheduler completion timed out"
        );
        std::thread::sleep(Duration::from_millis(2));
    };
    assert_eq!(completion.ticket(), ticket);
    assert!(scheduler.accept_completion(&completion, &session).unwrap());
    if let Some(error) = completion.error() {
        panic!("{state} legacy evaluation failed: {error}");
    }
    let diagnostics = completion.cache_diagnostics().unwrap();
    let result = completion.result().unwrap();
    let scene = result.scene();
    let raster = result.raster();
    assert_eq!(raster.width(), scene.canvas().width as u32);
    assert_eq!(raster.height(), scene.canvas().height as u32);
    let rgba = raster.pixels();
    let png = encode_png(raster).unwrap();
    let svg = write_svg(scene);
    let geometry = canonical_geometry_summary(scene);
    let raw_summary = raw_rgba_summary(raster.width(), raster.height(), rgba);
    let clip_count = svg.matches("<clipPath id=\"canvas-clip\">").count();
    let visible_marks = scene
        .layers()
        .iter()
        .filter(|layer| layer.visible())
        .map(|layer| match layer.geometry() {
            GeometryOutput::CircularMarks(marks) => marks.len(),
            GeometryOutput::CanonicalMarks(marks) => marks.len(),
        })
        .sum::<usize>();
    let circle_count = svg.matches("<circle ").count();
    let live_text_elements = svg.matches("<text").count();
    let positive_radii = scene
        .layers()
        .iter()
        .map(|layer| {
            match layer.geometry() {
            GeometryOutput::CircularMarks(marks) => (
                layer.channel_id(),
                marks.len(),
                marks.iter().filter(|mark| mark.radius > 0.0).count(),
            ),
            GeometryOutput::CanonicalMarks(marks) => (
                layer.channel_id(),
                marks.len(),
                marks
                    .iter()
                    .filter(|mark| {
                        matches!(mark, CanonicalMark::Circle { radius, .. } if *radius > 0.0)
                    })
                    .count(),
            ),
        }
        })
        .collect::<Vec<_>>();
    assert_eq!(clip_count, 1, "{state} legacy SVG uses one canvas clip");
    assert_eq!(
        circle_count, visible_marks,
        "{state} legacy SVG preserves editable circles"
    );
    fs::write(case_root.join(format!("{state}.rgba")), rgba).unwrap();
    fs::write(case_root.join(format!("{state}.png")), &png).unwrap();
    fs::write(case_root.join(format!("{state}.svg")), &svg).unwrap();
    fs::write(case_root.join(format!("{state}.geometry.txt")), &geometry).unwrap();
    let hashes = Stage17OutputHashes {
        geometry: sha256_hex(geometry.as_bytes()),
        rgba: sha256_hex(rgba),
        png: sha256_hex(&png),
        svg: sha256_hex(svg.as_bytes()),
    };
    manifest.push_str(&format!(
        "{state}.legacy.scheduler.ticket={}\n{state}.cache={diagnostics:?}\n{state}.source_identity={:?}\n{state}.channels=[legacy:{channel_id:?}]\n{state}.scene.identity={:?}\n{state}.scene.layers={}\n{state}.scene.circular_marks={}\n{state}.scene.positive_radii={positive_radii:?}\n{state}.structural={}\n{state}.geometry.sha256={}\n{state}.rgba.sha256={}\n{state}.png.sha256={}\n{state}.svg.sha256={}\n{state}.svg.canvas_clips={clip_count}\n{state}.svg.editable_circles={circle_count}\n{state}.svg.live_text_elements={live_text_elements}\n{state}.rgba.summary={raw_summary}\n",
        ticket.value(),
        result.source_identity(),
        scene.identity(),
        scene.layers().len(),
        scene.circular_mark_count(),
        structural_product_summary(document, source.bytes(), format),
        hashes.geometry,
        hashes.rgba,
        hashes.png,
        hashes.svg,
    ));
    scheduler.shutdown().unwrap();
    hashes
}

/// Summarizes both retained and generalized canonical marks for derived validation evidence.
fn structural_product_summary(
    document: &Document,
    source_bytes: &[u8],
    format: SourceFormatHint,
) -> String {
    let definition_id = if let Some(topology) = document.channel_topology() {
        topology.channels()[0].pattern_definition_id
    } else {
        document.channels().unwrap()[0].pattern_definition_id
    };
    let definition = document
        .pattern_definitions()
        .iter()
        .find(|definition| definition.id == definition_id)
        .unwrap();
    let scene = evaluate_scene_for_evidence(document, source_bytes, format);
    let (realized_mark_count, canvas_marks) =
        scene
            .layers()
            .iter()
            .fold((0usize, 0usize), |(total, canvas), layer| {
                match layer.geometry() {
                    GeometryOutput::CircularMarks(marks) => (
                        total + marks.len(),
                        canvas
                            + marks
                                .iter()
                                .filter(|mark| mark.scope == SiteScope::Canvas)
                                .count(),
                    ),
                    GeometryOutput::CanonicalMarks(marks) => (
                        total + marks.len(),
                        canvas
                            + marks
                                .iter()
                                .filter(|mark| match mark {
                                    CanonicalMark::Circle { scope, .. } => {
                                        *scope == SiteScope::Canvas
                                    }
                                    CanonicalMark::ClosedPath(mark) => {
                                        mark.scope == SiteScope::Canvas
                                    }
                                })
                                .count(),
                    ),
                }
            });
    let provenance_count = realized_mark_count;
    format!(
        "authority=engine-canonical-scene;family_fingerprint={};coverage={:?};mechanism_count={};output_layer_count={};realized_mark_count={};canvas_mark_count={canvas_marks};provenance_count={provenance_count}",
        scene.identity().family_fingerprint(),
        definition.coverage,
        definition.mechanisms.len(),
        definition.output_layers.len(),
        realized_mark_count,
    )
}

/// Evaluates a current document through the authoritative modeled or legacy path for evidence.
fn evaluate_scene_for_evidence(
    document: &Document,
    source_bytes: &[u8],
    format: SourceFormatHint,
) -> RenderScene {
    let source_id = match document.source() {
        SourceReference::Assigned(value) => value.clone(),
        SourceReference::Unassigned => panic!("evidence document requires a source"),
    };
    let source = ResolvedSource::new(source_id, source_bytes.to_vec(), format).unwrap();
    let session = DocumentSession::new(document.clone()).unwrap();
    if document.channel_topology().is_some() {
        evaluate_with_limits(
            EvaluationRequest::new(session.document_evaluation_snapshot(), source),
            EvaluationLimits::default(),
        )
        .unwrap()
        .scene()
        .clone()
    } else {
        let channel_id = document.channels().unwrap()[0].id;
        evaluate_channel_diagnostic(ChannelDiagnosticRequest::new(
            session.evaluation_snapshot(channel_id).unwrap(),
            source,
        ))
        .unwrap()
        .scene()
        .clone()
    }
}

/// Serializes stable source-site identities for retained or generalized canonical scene geometry.
fn canonical_geometry_summary(scene: &toniator_engine::RenderScene) -> String {
    let mut output = String::new();
    for layer in scene.layers() {
        match layer.geometry() {
            GeometryOutput::CircularMarks(marks) => {
                output.push_str(&format!(
                    "channel={:?};mark_count={}\n",
                    layer.channel_id(),
                    marks.len()
                ));
                for mark in marks {
                    output.push_str(&format!("{:?}\n", mark.source_site_id));
                }
            }
            GeometryOutput::CanonicalMarks(marks) => {
                output.push_str(&format!(
                    "channel={:?};mark_count={}\n",
                    layer.channel_id(),
                    marks.len()
                ));
                for mark in marks {
                    match mark {
                        CanonicalMark::Circle { source_site_id, .. } => {
                            output.push_str(&format!("{:?}\n", source_site_id));
                        }
                        CanonicalMark::ClosedPath(mark) => {
                            output.push_str(&format!("{:?}\n", mark.source_site_id));
                        }
                    }
                }
            }
        }
    }
    output
}

fn raw_rgba_summary(width: u32, height: u32, pixels: &[u8]) -> String {
    assert_eq!(pixels.len(), width as usize * height as usize * 4);
    let mut transparent = 0usize;
    let mut opaque = 0usize;
    let mut partial = 0usize;
    let mut hidden_rgb = 0usize;
    let mut alpha_min = u8::MAX;
    let mut alpha_max = 0u8;
    let mut edge_coverage = 0usize;
    for (index, pixel) in pixels.chunks_exact(4).enumerate() {
        let alpha = pixel[3];
        alpha_min = alpha_min.min(alpha);
        alpha_max = alpha_max.max(alpha);
        match alpha {
            0 => {
                transparent += 1;
                hidden_rgb += usize::from(pixel[0] != 0 || pixel[1] != 0 || pixel[2] != 0);
            }
            u8::MAX => opaque += 1,
            _ => partial += 1,
        }
        let x = index as u32 % width;
        let y = index as u32 / width;
        if alpha > 0 && (x == 0 || y == 0 || x + 1 == width || y + 1 == height) {
            edge_coverage += 1;
        }
    }
    format!(
        "width={width};height={height};pixels={};alpha.transparent={transparent};alpha.opaque={opaque};alpha.partial={partial};alpha.min={alpha_min};alpha.max={alpha_max};hidden_rgb={hidden_rgb};edge_covered_pixels={edge_coverage}",
        width as usize * height as usize
    )
}

/// Measures current circle-shaped random output while accepting generalized canonical mark storage.
fn natural_random_metrics(container: &Path) -> String {
    let loaded = load(container).unwrap();
    let document = loaded.document();
    let source = loaded.sources().entries().next().unwrap();
    let format = match source.format() {
        EmbeddedSourceFormat::Png => SourceFormatHint::Png,
        EmbeddedSourceFormat::Svg => SourceFormatHint::Svg,
    };
    let scene = evaluate_scene_for_evidence(document, source.bytes(), format);
    let marks = scene
        .layers()
        .iter()
        .flat_map(|layer| match layer.geometry() {
            GeometryOutput::CircularMarks(marks) => marks
                .iter()
                .map(|mark| {
                    (
                        mark.center,
                        mark.radius,
                        mark.scope,
                        format!("{:?}", mark.source_site_id),
                    )
                })
                .collect::<Vec<_>>()
                .into_iter(),
            GeometryOutput::CanonicalMarks(marks) => marks
                .iter()
                .filter_map(|mark| match mark {
                    CanonicalMark::Circle {
                        source_site_id,
                        center,
                        radius,
                        scope,
                        ..
                    } => Some((*center, *radius, *scope, format!("{:?}", source_site_id))),
                    CanonicalMark::ClosedPath(_) => None,
                })
                .collect::<Vec<_>>()
                .into_iter(),
        })
        .collect::<Vec<_>>();
    let canvas_marks = marks
        .iter()
        .filter(|mark| mark.2 == SiteScope::Canvas)
        .collect::<Vec<_>>();
    let guard_marks = marks.len() - canvas_marks.len();
    let mut bins = [0usize; 32 * 32];
    for mark in &canvas_marks {
        let x = ((mark.0.x / document.canvas().width) * 32.0)
            .floor()
            .clamp(0.0, 31.0) as usize;
        let y = ((mark.0.y / document.canvas().height) * 32.0)
            .floor()
            .clamp(0.0, 31.0) as usize;
        bins[y * 32 + x] += 1;
    }
    let bin_mean = canvas_marks.len() as f64 / bins.len() as f64;
    let bin_variance = bins
        .iter()
        .map(|count| (*count as f64 - bin_mean).powi(2))
        .sum::<f64>()
        / bins.len() as f64;
    let nearest_sample = canvas_marks.len().min(256);
    let nearest_mean = if nearest_sample == 0 {
        0.0
    } else {
        canvas_marks
            .iter()
            .take(nearest_sample)
            .map(|mark| {
                canvas_marks
                    .iter()
                    .filter(|other| other.3 != mark.3)
                    .map(|other| {
                        let dx = other.0.x - mark.0.x;
                        let dy = other.0.y - mark.0.y;
                        dx.mul_add(dx, dy * dy).sqrt()
                    })
                    .fold(f64::INFINITY, f64::min)
            })
            .sum::<f64>()
            / nearest_sample as f64
    };
    let positive_radii = canvas_marks.iter().filter(|mark| mark.1 > 0.0).count();
    let radius_mean = if canvas_marks.is_empty() {
        0.0
    } else {
        canvas_marks.iter().map(|mark| mark.1).sum::<f64>() / canvas_marks.len() as f64
    };
    let channel = &document.channel_topology().unwrap().channels()[0];
    format!(
        "authority=engine-canonical-scene;requested_density={:?};achieved_realized_marks={};canvas_marks={};guard_marks={guard_marks};site_provenance_count={};site_order_hash={};bins32.occupied={};bins32.mean={bin_mean:.6};bins32.variance={bin_variance:.6};nearest_neighbor_sample_mean={nearest_mean:.6};positive_radii={positive_radii};realized_radius_mean={radius_mean:.6}",
        channel.layout.density,
        marks.len(),
        canvas_marks.len(),
        marks.len(),
        sha256_hex(
            marks
                .iter()
                .map(|mark| format!("{}\n", mark.3))
                .collect::<String>()
                .as_bytes(),
        ),
        bins.iter().filter(|count| **count > 0).count(),
    )
}

fn assert_natural_native_checks(rgba_path: &Path, width: u32, height: u32) {
    let pixels = fs::read(rgba_path).unwrap();
    assert_eq!(pixels.len(), width as usize * height as usize * 4);
    assert!(pixels.chunks_exact(4).any(|pixel| pixel[3] > 0));
}

fn assert_cli_container_render_parity(case_root: &Path, container: &Path) {
    let png = case_root.join("cli-container.png");
    let svg = case_root.join("cli-container.svg");
    for output in [&png, &svg] {
        let result = Command::new(env!("CARGO_BIN_EXE_toniator"))
            .args([
                "render",
                "--input",
                container.to_str().unwrap(),
                "--output",
                output.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    assert_eq!(
        fs::read(png).unwrap(),
        fs::read(case_root.join("apply.png")).unwrap()
    );
    assert_eq!(
        fs::read(svg).unwrap(),
        fs::read(case_root.join("apply.svg")).unwrap()
    );
}

fn write_stage17_low_resolution_source_model_matrix(root: &Path) {
    let matrix_root = root.join("source-model-matrix");
    fs::create_dir_all(&matrix_root).unwrap();
    for (source_label, source_format, source_asset) in [
        ("raster", EmbeddedSourceFormat::Png, "raster-sample.png"),
        ("vector", EmbeddedSourceFormat::Svg, "vector-sample.svg"),
    ] {
        let source_bytes = fs::read(baseline_path(source_asset)).unwrap();
        for (model_label, model) in [
            ("legacy", None),
            ("rgb", Some(HalftoneChannelModel::Rgb)),
            ("cmyk", Some(HalftoneChannelModel::Cmyk)),
            (
                "source-color-alpha",
                Some(HalftoneChannelModel::SourceColorAlpha),
            ),
        ] {
            let case_root = matrix_root.join(format!("{source_label}-{model_label}"));
            fs::create_dir_all(&case_root).unwrap();
            let source_id =
                SourceReferenceId::new(format!("stage17-{source_label}-{model_label}-source"))
                    .unwrap();
            let (document, sources) = low_resolution_parity_document(
                source_bytes.clone(),
                source_format,
                source_id,
                model,
            );
            let mut history = DocumentHistory::new(DocumentSession::new(document).unwrap());
            let channel_id = history
                .document()
                .channel_topology()
                .map(|topology| topology.channels()[0].id)
                .or_else(|| history.document().channels().map(|channels| channels[0].id))
                .unwrap();
            let mut results = Vec::new();
            if model.is_none() {
                results.push(
                    history
                        .apply(&DocumentCommand::SetLegacyMappingField {
                            channel_id,
                            edit: LegacyMappingFieldEdit::Component(SourceComponent::Alpha),
                        })
                        .unwrap(),
                );
            } else {
                results.push(
                    history
                        .apply(&DocumentCommand::SetModeledMappingField {
                            channel_id,
                            edit: ModeledMappingFieldEdit::Gain(1.2),
                        })
                        .unwrap(),
                );
            }
            if model != Some(HalftoneChannelModel::SourceColorAlpha) {
                results.push(
                    history
                        .apply(&DocumentCommand::SetColorComponent {
                            channel_id,
                            component: ColorComponent::Red,
                            value: 0.2,
                        })
                        .unwrap(),
                );
            }
            results.push(
                history
                    .apply(&DocumentCommand::SetOpacity {
                        channel_id,
                        opacity: 0.75,
                    })
                    .unwrap(),
            );
            let applied = history.document().clone();
            let container = case_root.join("apply.toniator");
            save(&container, &applied, &sources).unwrap();
            let reopened = load(&container).unwrap();
            assert_eq!(reopened.document(), &applied);
            assert_eq!(
                reopened.sources().entries().next().unwrap().bytes(),
                source_bytes.as_slice()
            );
            fs::write(
                case_root.join("apply.capabilities.txt"),
                descriptor_text(&applied),
            )
            .unwrap();
            let mut manifest = format!(
                "stage17-low-resolution-source-model={source_label}-{model_label}\nsource.asset={source_asset}\nsource.sha256={}\ncommands={results:?}\ndocument={applied:#?}\n",
                sha256_hex(&source_bytes),
            );
            let scheduler = EvaluationScheduler::new().unwrap();
            let hashes = capture_stage17_evaluation(
                &case_root,
                "apply",
                &container,
                &scheduler,
                &mut manifest,
            );
            scheduler.shutdown().unwrap();
            manifest.push_str(&format!("output.hashes={hashes:?}\n"));
            fs::write(case_root.join("manifest.txt"), manifest).unwrap();
        }
    }
}

fn low_resolution_parity_document(
    bytes: Vec<u8>,
    format: EmbeddedSourceFormat,
    source_id: SourceReferenceId,
    model: Option<HalftoneChannelModel>,
) -> (Document, SourceBundle) {
    let canvas = CanvasSpec {
        width: 96.0,
        height: 64.0,
    };
    let layout = ChannelPatternLayout {
        density: DensityMetric2D {
            across_x: 9.6,
            across_y: 6.4,
            aspect_locked: true,
        },
        rotation_degrees: 0.0,
        translation_x: 0.0,
        translation_y: 0.0,
    };
    let definition = PatternDefinition::supported_straight_grid(
        PatternDefinitionId(1),
        "stage17-low-resolution-parity",
        PatternMechanismId(1),
        PatternMechanismId(2),
        PatternOutputLayerId(1),
        CoveragePolicy {
            guard_steps: 2,
            additional_margin: 4.5,
        },
    );
    let legacy = Document::with_source(
        DocumentId(1),
        canvas,
        SourceReference::Assigned(source_id.clone()),
        vec![definition],
        vec![ChannelState {
            id: ChannelId(1),
            pattern_definition_id: PatternDefinitionId(1),
            layout: layout.clone(),
            appearance: ChannelAppearance {
                visible: true,
                color: ColorValue {
                    red: 0.0,
                    green: 0.0,
                    blue: 0.0,
                    alpha: 1.0,
                },
                opacity: 1.0,
            },
            mark_geometry_response: MarkGeometryResponse {
                minimum_fill: 2.0,
                maximum_fill: 9.0,
                rotation_offset_degrees: 0.0,
            },
            source_mapping: ChannelSourceMapping {
                component: SourceComponent::Luminance,
                placement: SourcePlacement::StretchToCanvas,
            },
        }],
    )
    .unwrap();
    let document = match model {
        None => legacy,
        Some(model) => {
            let topology = legacy
                .canonical_channel_topology(
                    model,
                    ChannelTopologyTemplate {
                        pattern_definition_id: PatternDefinitionId(1),
                        layout,
                        mark_geometry_response: MarkGeometryResponse {
                            minimum_fill: 2.0,
                            maximum_fill: 9.0,
                            rotation_offset_degrees: 0.0,
                        },
                    },
                )
                .unwrap();
            Document::with_source_and_topology(
                legacy.id(),
                legacy.canvas().clone(),
                legacy.source().clone(),
                legacy.pattern_definitions().to_vec(),
                model,
                topology,
            )
            .unwrap()
        }
    };
    let sources =
        SourceBundle::new([EmbeddedSource::new(source_id, format, bytes, None).unwrap()]).unwrap();
    (document, sources)
}

fn write_stage17_frozen_v1_parity(root: &Path) {
    let frozen_root = root.join("frozen-v1-parity");
    fs::create_dir_all(&frozen_root).unwrap();
    let mut historical = String::from(
        "Stage15 saved-v2 baselines are constructed in-code by crates/toniator-engine/tests/document_evaluation.rs; there are no standalone Stage15/16A/16B v2 fixture files.\n",
    );
    historical.push_str("stage15.raster.rgb=7135531041b8a4f9136731267b356ce4b3acbdb74c6e12c6670817e0613436cf\nstage15.raster.cmyk=9aa1ec4c5fe5fca6b023278719ebe56160ec526617ec46eb2f4864277c3ea588\nstage15.raster.source_color_alpha=1137a5bd4ccc0905087081ff62aa70feb0bf195a7c10272b12bfc323760db6d2\nstage15.vector.rgb=b2d6f3116d9b5aa4bef37d89268be5aa6092a9eb195b33049d53ecad7e910d97\nstage15.vector.cmyk=9424c9d9278fe0e4780a1b4c2ba7688a8b46292c1be7e24144fb3ce1ae81041a\nstage15.vector.source_color_alpha=419e16a7e8b6de45799dd3780e8c2a781e5050ede74dfd2a33cb114097e0b515\n");
    historical.push_str("stage16a/stage16b persistence inputs are in-code deterministic construction tests in crates/toniator-io/tests/persistence.rs; they have no committed standalone v2 archive bytes.\n");
    fs::write(frozen_root.join("historical-v2-baselines.txt"), historical).unwrap();

    for (fixture, source_asset, expected_fixture_hash, expected_v2_hash) in [
        (
            "raster-sample-v1.toniator",
            "raster-sample.png",
            "9efac3250e2c4a6650648fd2f5c3820283ea0bf5703459ea996204197aee2a8f",
            "7135531041b8a4f9136731267b356ce4b3acbdb74c6e12c6670817e0613436cf",
        ),
        (
            "vector-sample-v1.toniator",
            "vector-sample.svg",
            "34fc138e8d194c57dc239986da2df73f603fb4f51adaa5e7698170582bf6e4ea",
            "b2d6f3116d9b5aa4bef37d89268be5aa6092a9eb195b33049d53ecad7e910d97",
        ),
    ] {
        let case_root = frozen_root.join(fixture.trim_end_matches(".toniator"));
        fs::create_dir_all(&case_root).unwrap();
        let fixture_path = baseline_path(fixture);
        let fixture_bytes = fs::read(&fixture_path).unwrap();
        assert_eq!(sha256_hex(&fixture_bytes), expected_fixture_hash);
        let loaded = load(&fixture_path).unwrap();
        let source = loaded.sources().entries().next().unwrap();
        let baseline_bytes = fs::read(baseline_path(source_asset)).unwrap();
        assert_eq!(source.bytes(), baseline_bytes.as_slice());
        let migrated = case_root.join("migrated-v2.toniator");
        let duplicate = case_root.join("migrated-v2-repeat.toniator");
        save(&migrated, loaded.document(), loaded.sources()).unwrap();
        save(&duplicate, loaded.document(), loaded.sources()).unwrap();
        let migrated_bytes = fs::read(&migrated).unwrap();
        assert_eq!(migrated_bytes, fs::read(&duplicate).unwrap());
        assert_eq!(sha256_hex(&migrated_bytes), expected_v2_hash);
        let reopened = load(&migrated).unwrap();
        assert_eq!(reopened.document(), loaded.document());
        assert_eq!(
            reopened.sources().entries().next().unwrap().bytes(),
            source.bytes()
        );
        let mut manifest = format!(
            "fixture={fixture}\nfixture.sha256={}\nsource.asset={source_asset}\nsource.sha256={}\nloaded.versions={:?}\nloaded.migration={:?}\nmigrated-v2.sha256={}\n",
            expected_fixture_hash,
            sha256_hex(&baseline_bytes),
            loaded.versions(),
            loaded.migration_report(),
            expected_v2_hash,
        );
        let scheduler = EvaluationScheduler::new().unwrap();
        let frozen_hashes = capture_stage17_evaluation(
            &case_root,
            "frozen-v1",
            &fixture_path,
            &scheduler,
            &mut manifest,
        );
        let reopened_hashes = capture_stage17_evaluation(
            &case_root,
            "migrated-v2",
            &migrated,
            &scheduler,
            &mut manifest,
        );
        assert_eq!(frozen_hashes, reopened_hashes);
        scheduler.shutdown().unwrap();
        manifest.push_str(&format!(
            "frozen-v1.output.hashes={frozen_hashes:?}\nmigrated-v2.output.hashes={reopened_hashes:?}\n"
        ));
        fs::write(case_root.join("manifest.txt"), manifest).unwrap();
    }
}

fn descriptor_text(document: &Document) -> String {
    document
        .property_descriptors()
        .iter()
        .map(|descriptor| {
            format!(
                "field={:?}\ttarget={:?}\tcommand={:?}\tkind={:?}\tdependency={:?}\tsupport={:?}\treference={:?}\tinvalidation={:?}\tcopy_escalates={}\n",
                descriptor.field,
                descriptor.target,
                descriptor.command_kind(),
                descriptor.value_kind,
                descriptor.dependency,
                descriptor.structural_support,
                descriptor.reference_constraint,
                descriptor.invalidation,
                descriptor.copy_on_edit_escalates_to_family,
            )
        })
        .collect()
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[test]
fn zero_density_exits_two_with_schema_path_and_no_success_output() {
    let output = Command::new(env!("CARGO_BIN_EXE_toniator"))
        .args([
            "validate",
            "--canvas",
            "900x600",
            "--density-x",
            "0",
            "--density-y",
            "60.0",
            "--opacity",
            "0.75",
        ])
        .output()
        .expect("CLI runs");

    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8(output.stderr)
            .expect("utf8 stderr")
            .contains("channel.pattern.layout.density.across_x")
    );
    assert_eq!(String::from_utf8(output.stdout).expect("utf8 stdout"), "");
}

#[test]
fn inspect_grid_accepts_negative_offsets_and_emits_deterministic_json() {
    let output_path = std::env::temp_dir().join(format!(
        "toniator-stage-3-{}.json",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos()
    ));
    let output = Command::new(env!("CARGO_BIN_EXE_toniator"))
        .args([
            "inspect",
            "grid",
            "--output",
            output_path.to_str().expect("utf8 temp path"),
            "--canvas",
            "900x600",
            "--density-x",
            "90.0",
            "--density-y",
            "60.0",
            "--rotation",
            "17.0",
            "--offset-x",
            "3.25",
            "--offset-y",
            "-4.5",
            "--guard-steps",
            "2",
            "--support-radius",
            "4.5",
            "--format",
            "json",
        ])
        .output()
        .expect("CLI runs");

    assert!(output.status.success());
    let json: serde_json::Value =
        serde_json::from_slice(&fs::read(&output_path).expect("output")).expect("valid JSON");
    assert_eq!(json["coverage"][0]["first_index"], -11);
    assert_eq!(json["coverage"][1]["last_index"], 76);
    assert_eq!(json["sites"].as_array().expect("sites array").len(), 6_185);
    let fixture: serde_json::Value = serde_json::from_slice(
        &fs::read("../../fixtures/canonical/stage-3-sites.sorted.json").expect("fixture"),
    )
    .expect("fixture JSON");
    assert_eq!(
        json, fixture,
        "CLI output must match the canonical Stage 3 sites"
    );
    fs::remove_file(output_path).expect("remove temporary artifact");
}

#[test]
fn every_evaluation_command_accepts_the_candidate_limit_and_rejects_an_oversized_grid() {
    let grid = Command::new(env!("CARGO_BIN_EXE_toniator"))
        .args([
            "inspect",
            "grid",
            "--canvas",
            "900x600",
            "--density-x",
            "90",
            "--density-y",
            "60",
            "--rotation",
            "17",
            "--offset-x",
            "3.25",
            "--offset-y",
            "-4.5",
            "--guard-steps",
            "2",
            "--support-radius",
            "4.5",
            "--max-family-candidates",
            "1",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert_eq!(grid.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&grid.stderr).contains("coverage.candidate_limit"));

    let marks = Command::new(env!("CARGO_BIN_EXE_toniator"))
        .args([
            "inspect",
            "marks",
            "--source",
            "../../assets/raster-sample.png",
            "--canvas",
            "900x600",
            "--density-x",
            "90",
            "--density-y",
            "60",
            "--rotation",
            "17",
            "--offset-x",
            "3.25",
            "--offset-y",
            "-4.5",
            "--guard-steps",
            "2",
            "--support-radius",
            "4.5",
            "--max-family-candidates",
            "1",
            "--source-component",
            "luminance",
            "--size-min",
            "2",
            "--size-max",
            "9",
            "--color",
            "#00b7ff",
            "--opacity",
            "0.72",
            "--summary",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert_eq!(marks.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&marks.stderr).contains("coverage.candidate_limit"));

    let output = std::env::temp_dir().join("toniator-stage-8-limit.png");
    let render = Command::new(env!("CARGO_BIN_EXE_toniator"))
        .args([
            "render",
            "-i",
            "../../assets/raster-sample.png",
            "-o",
            output.to_str().unwrap(),
            "--channel-model",
            "rgb",
            "--canvas",
            "900x600",
            "--density-x",
            "90",
            "--density-y",
            "60",
            "--rotation",
            "17",
            "--offset-x",
            "3.25",
            "--offset-y",
            "-4.5",
            "--guard-steps",
            "2",
            "--max-family-candidates",
            "1",
            "--size-min",
            "2",
            "--size-max",
            "9",
            "--opacity",
            "0.72",
        ])
        .output()
        .unwrap();
    assert_eq!(render.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&render.stderr).contains("coverage.candidate_limit"));
}

#[test]
fn inspect_marks_compact_summaries_match_both_canonical_fixtures() {
    for (source, fixture) in [
        (
            "../../assets/raster-sample.png",
            "../../fixtures/canonical/stage-4-raster-summary.json",
        ),
        (
            "../../assets/vector-sample.svg",
            "../../fixtures/canonical/stage-4-svg-summary.json",
        ),
    ] {
        let output_path = std::env::temp_dir().join(format!(
            "toniator-stage-4-{}.json",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let output = Command::new(env!("CARGO_BIN_EXE_toniator"))
            .args([
                "inspect",
                "marks",
                "--source",
                source,
                "--output",
                output_path.to_str().unwrap(),
                "--canvas",
                "900x600",
                "--density-x",
                "90.0",
                "--density-y",
                "60.0",
                "--rotation",
                "17.0",
                "--offset-x",
                "3.25",
                "--offset-y",
                "-4.5",
                "--guard-steps",
                "2",
                "--support-radius",
                "4.5",
                "--source-component",
                "luminance",
                "--size-min",
                "2.0",
                "--size-max",
                "9.0",
                "--color",
                "#00b7ff",
                "--opacity",
                "0.72",
                "--summary",
                "--format",
                "json",
            ])
            .output()
            .expect("CLI runs");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let actual: serde_json::Value =
            serde_json::from_slice(&fs::read(&output_path).unwrap()).unwrap();
        let expected: serde_json::Value =
            serde_json::from_slice(&fs::read(fixture).unwrap()).unwrap();
        assert_eq!(actual, expected);
        fs::remove_file(output_path).unwrap();
    }
}

#[test]
fn inspect_marks_presentation_changes_leave_geometry_summary_identical() {
    let mut summaries = Vec::new();
    for (color, opacity) in [("#00b7ff", "0.72"), ("#ff5500", "0.19")] {
        let output = Command::new(env!("CARGO_BIN_EXE_toniator"))
            .args([
                "inspect",
                "marks",
                "--source",
                "../../assets/raster-sample.png",
                "--canvas",
                "900x600",
                "--density-x",
                "90.0",
                "--density-y",
                "60.0",
                "--rotation",
                "17.0",
                "--offset-x",
                "3.25",
                "--offset-y",
                "-4.5",
                "--guard-steps",
                "2",
                "--support-radius",
                "4.5",
                "--source-component",
                "luminance",
                "--size-min",
                "2.0",
                "--size-max",
                "9.0",
                "--color",
                color,
                "--opacity",
                opacity,
                "--summary",
                "--format",
                "json",
            ])
            .output()
            .unwrap();
        assert!(output.status.success());
        summaries.push(serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap());
    }
    let [mut first, mut second] = summaries.try_into().unwrap();
    let presentation_a = first
        .as_object_mut()
        .unwrap()
        .remove("presentation")
        .unwrap();
    let presentation_b = second
        .as_object_mut()
        .unwrap()
        .remove("presentation")
        .unwrap();
    assert_ne!(presentation_a, presentation_b);
    assert_eq!(first, second);
}

fn render_command(source: &str, output: &std::path::Path, model: &str) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_toniator"));
    command.args([
        "render",
        "--input",
        source,
        "--output",
        output.to_str().unwrap(),
        "--channel-model",
        model,
        "--canvas",
        "900x600",
        "--density-x",
        "90.0",
        "--density-y",
        "60.0",
        "--rotation",
        "17.0",
        "--offset-x",
        "3.25",
        "--offset-y",
        "-4.5",
        "--guard-steps",
        "2",
        "--size-min",
        "2.0",
        "--size-max",
        "9.0",
        "--opacity",
        "0.72",
    ]);
    command
}

fn direct_render_without_canvas(source: &str, output: &Path, model: &str) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_toniator"));
    command.args([
        "render",
        "--input",
        source,
        "--output",
        output.to_str().unwrap(),
        "--channel-model",
        model,
        "--density-x",
        "90",
        "--density-y",
        "60",
        "--rotation",
        "17",
        "--offset-x",
        "3.25",
        "--offset-y",
        "-4.5",
        "--guard-steps",
        "2",
        "--size-min",
        "2",
        "--size-max",
        "9",
    ]);
    command
}

fn png_dimensions(bytes: &[u8]) -> (u32, u32) {
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    (
        u32::from_be_bytes(bytes[16..20].try_into().unwrap()),
        u32::from_be_bytes(bytes[20..24].try_into().unwrap()),
    )
}

#[test]
fn direct_source_intrinsic_canvas_and_png_antialiasing_are_consumer_only() {
    let directory = temporary_directory("stage-13b-cli");
    let raster_default = directory.join("raster-default.png");
    let vector_default = directory.join("vector-default.png");
    let vector_svg = directory.join("vector-default.svg");
    for (source, output, dimensions) in [
        (
            "../../assets/raster-sample.png",
            &raster_default,
            (1024, 1024),
        ),
        (
            "../../assets/vector-sample.svg",
            &vector_default,
            (900, 620),
        ),
    ] {
        let output = direct_render_without_canvas(source, output, "rgb")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            png_dimensions(
                &fs::read(if source.ends_with("png") {
                    &raster_default
                } else {
                    &vector_default
                })
                .unwrap()
            ),
            dimensions
        );
    }
    let output = direct_render_without_canvas("../../assets/vector-sample.svg", &vector_svg, "rgb")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(&vector_svg).unwrap();
    assert!(svg.contains("width=\"900\" height=\"620\" viewBox=\"0 0 900 620\""));

    let overridden = directory.join("raster-overridden.png");
    let mut command =
        direct_render_without_canvas("../../assets/raster-sample.png", &overridden, "rgb");
    command.args(["--canvas", "320x180"]);
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(png_dimensions(&fs::read(&overridden).unwrap()), (320, 180));

    let default_aa = directory.join("aa-default.png");
    let explicit_aa = directory.join("aa-on.png");
    let hard_edges = directory.join("aa-off.png");
    for (path, mode) in [
        (&default_aa, None),
        (&explicit_aa, Some("on")),
        (&hard_edges, Some("off")),
    ] {
        let mut command = direct_render_without_canvas(
            "../../assets/raster-sample.png",
            path,
            "source-color-alpha",
        );
        if let Some(mode) = mode {
            command.args(["--antialiasing", mode]);
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert_eq!(
        fs::read(&default_aa).unwrap(),
        fs::read(&explicit_aa).unwrap()
    );
    assert_ne!(
        fs::read(&default_aa).unwrap(),
        fs::read(&hard_edges).unwrap()
    );
    let invalid = direct_render_without_canvas(
        "../../assets/raster-sample.png",
        &directory.join("invalid.png"),
        "rgb",
    )
    .args(["--antialiasing", "soft"])
    .output()
    .unwrap();
    assert_eq!(invalid.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("antialiasing"));
    let unsafe_output = directory.join("unsafe-output.png");
    let unsafe_canvas = Command::new(env!("CARGO_BIN_EXE_toniator"))
        .args([
            "render",
            "--input",
            "../../assets/raster-sample.png",
            "--output",
            unsafe_output.to_str().unwrap(),
            "--channel-model",
            "rgb",
            "--canvas",
            "67108865x1",
            "--density-x",
            "1",
            "--density-y",
            "1",
            "--rotation",
            "0",
            "--offset-x",
            "0",
            "--offset-y",
            "0",
            "--guard-steps",
            "0",
            "--size-min",
            "2",
            "--size-max",
            "2",
        ])
        .output()
        .unwrap();
    assert_eq!(unsafe_canvas.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&unsafe_canvas.stderr).contains("output.target"),
        "{}",
        String::from_utf8_lossy(&unsafe_canvas.stderr)
    );
    assert!(!unsafe_output.exists(), "unsafe output must not be created");
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn container_render_uses_its_stored_canvas_not_direct_source_defaulting() {
    let directory = temporary_directory("stage-13b-container");
    for fixture in ["raster-sample-v1.toniator", "vector-sample-v1.toniator"] {
        let output_path = directory.join(format!("{fixture}.png"));
        let output = Command::new(env!("CARGO_BIN_EXE_toniator"))
            .args([
                "render",
                "--input",
                &format!("../../assets/{fixture}"),
                "--output",
                output_path.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let loaded = load(&baseline_path(fixture)).unwrap();
        assert_eq!(
            png_dimensions(&fs::read(output_path).unwrap()),
            (
                loaded.document().canvas().width as u32,
                loaded.document().canvas().height as u32,
            )
        );
    }
    fs::remove_dir_all(directory).unwrap();
}

fn temporary_directory(label: &str) -> std::path::PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "toniator-{label}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&directory).unwrap();
    directory
}

fn baseline_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets")
        .join(name)
}

fn parity_document(
    bytes: Vec<u8>,
    format: EmbeddedSourceFormat,
    width: f64,
    height: f64,
    model: HalftoneChannelModel,
) -> (Document, SourceBundle) {
    let source_id = SourceReferenceId::new("source-1").unwrap();
    let layout = ChannelPatternLayout {
        density: DensityMetric2D {
            across_x: width / 10.0,
            across_y: height / 10.0,
            aspect_locked: true,
        },
        rotation_degrees: 0.0,
        translation_x: 0.0,
        translation_y: 0.0,
    };
    let legacy = Document::with_source(
        DocumentId(1),
        CanvasSpec { width, height },
        SourceReference::Assigned(source_id.clone()),
        vec![PatternDefinition::supported_straight_grid(
            PatternDefinitionId(1),
            "straight-grid",
            PatternMechanismId(1),
            PatternMechanismId(2),
            PatternOutputLayerId(1),
            CoveragePolicy {
                guard_steps: 2,
                additional_margin: 4.5,
            },
        )],
        vec![ChannelState {
            id: ChannelId(1),
            pattern_definition_id: PatternDefinitionId(1),
            layout: layout.clone(),
            appearance: ChannelAppearance {
                visible: true,
                color: ColorValue {
                    red: 0.0,
                    green: 0.0,
                    blue: 0.0,
                    alpha: 1.0,
                },
                opacity: 1.0,
            },
            mark_geometry_response: MarkGeometryResponse {
                minimum_fill: 2.0,
                maximum_fill: 9.0,
                rotation_offset_degrees: 0.0,
            },
            source_mapping: ChannelSourceMapping {
                component: SourceComponent::Luminance,
                placement: SourcePlacement::StretchToCanvas,
            },
        }],
    )
    .unwrap();
    let topology = legacy
        .canonical_channel_topology(
            model,
            ChannelTopologyTemplate {
                pattern_definition_id: PatternDefinitionId(1),
                layout,
                mark_geometry_response: MarkGeometryResponse {
                    minimum_fill: 2.0,
                    maximum_fill: 9.0,
                    rotation_offset_degrees: 0.0,
                },
            },
        )
        .unwrap();
    let document = Document::with_source_and_topology(
        legacy.id(),
        legacy.canvas().clone(),
        legacy.source().clone(),
        legacy.pattern_definitions().to_vec(),
        model,
        topology,
    )
    .unwrap();
    let bundle =
        SourceBundle::new([EmbeddedSource::new(source_id, format, bytes, None).unwrap()]).unwrap();
    (document, bundle)
}

fn evaluate_parity(
    document: &Document,
    source: &EmbeddedSource,
) -> toniator_engine::EvaluationResult {
    let session = DocumentSession::new(document.clone()).unwrap();
    let format = match source.format() {
        EmbeddedSourceFormat::Png => SourceFormatHint::Png,
        EmbeddedSourceFormat::Svg => SourceFormatHint::Svg,
    };
    evaluate_with_limits(
        EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(source.id().clone(), source.bytes().to_vec(), format).unwrap(),
        ),
        EvaluationLimits::default(),
    )
    .unwrap()
}

#[test]
fn direct_and_container_evaluation_are_identical_for_both_baselines_and_all_models() {
    for (baseline, format, width, height) in [
        (
            "raster-sample.png",
            EmbeddedSourceFormat::Png,
            1024.0,
            1024.0,
        ),
        ("vector-sample.svg", EmbeddedSourceFormat::Svg, 900.0, 620.0),
    ] {
        for model in [
            HalftoneChannelModel::Rgb,
            HalftoneChannelModel::Cmyk,
            HalftoneChannelModel::SourceColorAlpha,
        ] {
            let (document, sources) = parity_document(
                fs::read(baseline_path(baseline)).unwrap(),
                format,
                width,
                height,
                model,
            );
            let direct = evaluate_parity(&document, sources.entries().next().unwrap());
            let directory = temporary_directory("stage-12-parity");
            let path = directory.join("document.toniator");
            save(&path, &document, &sources).unwrap();
            let loaded = load(&path).unwrap();
            let container = evaluate_parity(
                loaded.document(),
                loaded.sources().entries().next().unwrap(),
            );
            assert_eq!(container.source_identity(), direct.source_identity());
            assert_eq!(container.channels(), direct.channels());
            assert_eq!(container.scene().identity(), direct.scene().identity());
            assert_eq!(container.raster().pixels(), direct.raster().pixels());
            assert_eq!(write_svg(container.scene()), write_svg(direct.scene()));
            fs::remove_dir_all(directory).unwrap();
        }
    }
}

#[test]
fn render_uses_authoritative_document_models_for_both_immutable_sources() {
    let directory = temporary_directory("stage-9e-models");
    for (source, source_label) in [
        ("../../assets/raster-sample.png", "raster"),
        ("../../assets/vector-sample.svg", "vector"),
    ] {
        for (model, title, roles) in [
            ("rgb", "Toniator RGB halftone", &[1, 2, 3][..]),
            ("cmyk", "Toniator CMYK halftone", &[4, 5, 6, 7][..]),
            (
                "source-color-alpha",
                "Toniator source-colored halftone",
                &[8][..],
            ),
        ] {
            let output_path = directory.join(format!("{model}-{source_label}.svg"));
            let output = render_command(source, &output_path, model)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{model}/{source_label}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let svg = String::from_utf8(fs::read(&output_path).unwrap()).unwrap();
            assert!(svg.contains(title));
            for channel_id in roles {
                assert!(svg.contains(&format!("id=\"channel-{channel_id}\"")));
            }
            assert!(svg.contains("id=\"canvas\""));
            assert!(svg.contains("<circle "));
        }
    }
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn render_background_is_consumer_only_and_png_is_straight_srgba() {
    let directory = temporary_directory("stage-9e-background");
    let transparent = directory.join("transparent-cmyk.png");
    let black = directory.join("black.png");
    let white = directory.join("white.png");

    for model in ["rgb", "cmyk", "source-color-alpha"] {
        let output_path = directory.join(format!("transparent-{model}.png"));
        let output = render_command("../../assets/raster-sample.png", &output_path, model)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let bytes = fs::read(output_path).unwrap();
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert_eq!(bytes[25], 6, "PNG must use RGBA color type");
    }

    for (path, background) in [
        (&transparent, None),
        (&black, Some("black")),
        (&white, Some("white")),
    ] {
        let mut command = render_command("../../assets/raster-sample.png", path, "cmyk");
        if let Some(background) = background {
            command.args(["--background", background]);
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let bytes = fs::read(path).unwrap();
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert_eq!(bytes[25], 6, "PNG must use RGBA color type");
    }
    assert_ne!(fs::read(&transparent).unwrap(), fs::read(&black).unwrap());
    assert_ne!(fs::read(&black).unwrap(), fs::read(&white).unwrap());

    let svg = directory.join("opaque.svg");
    let output = render_command("../../assets/raster-sample.png", &svg, "rgb")
        .args(["--background", "black"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("error at render.background"));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn render_requires_channel_model_and_rejects_obsolete_render_options() {
    let directory = temporary_directory("stage-9e-arguments");
    let output_path = directory.join("output.png");
    let required = Command::new(env!("CARGO_BIN_EXE_toniator"))
        .args([
            "render",
            "--input",
            "../../assets/raster-sample.png",
            "--output",
            output_path.to_str().unwrap(),
            "--canvas",
            "900x600",
            "--density-x",
            "90",
            "--density-y",
            "60",
            "--rotation",
            "0",
            "--offset-x",
            "0",
            "--offset-y",
            "0",
            "--guard-steps",
            "2",
            "--size-min",
            "2",
            "--size-max",
            "9",
        ])
        .output()
        .unwrap();
    assert_eq!(required.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&required.stderr).contains("--channel-model"));

    for (option, value) in [
        ("--mode", "rgb"),
        ("--source-component", "luminance"),
        ("--color", "#00b7ff"),
    ] {
        let output = render_command("../../assets/raster-sample.png", &output_path, "rgb")
            .args([option, value])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&output.stderr).contains(option));
    }

    let invalid_extension = render_command(
        "../../assets/raster-sample.png",
        &directory.join("output.txt"),
        "rgb",
    )
    .output()
    .unwrap();
    assert_eq!(invalid_extension.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&invalid_extension.stderr)
            .contains("output extension must be .png or .svg")
    );

    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn portable_document_create_validate_render_and_argument_matrix() {
    let directory = temporary_directory("stage-12-document-cli");
    let source = directory.join("temporary-source.png");
    fs::copy("../../assets/raster-sample.png", &source).unwrap();
    let document = directory.join("created.toniator");
    let create = Command::new(env!("CARGO_BIN_EXE_toniator"))
        .args([
            "document",
            "create",
            "-i",
            source.to_str().unwrap(),
            "-o",
            document.to_str().unwrap(),
            "--channel-model",
            "rgb",
            "--canvas",
            "1024x1024",
            "--density-x",
            "102.4",
            "--density-y",
            "102.4",
            "--rotation",
            "0",
            "--offset-x",
            "0",
            "--offset-y",
            "0",
            "--guard-steps",
            "2",
            "--size-min",
            "2",
            "--size-max",
            "9",
            "--opacity",
            "1",
        ])
        .output()
        .unwrap();
    assert!(
        create.status.success(),
        "{}",
        String::from_utf8_lossy(&create.stderr)
    );
    assert!(document.exists());
    assert_eq!(
        fs::read_dir(&directory).unwrap().count(),
        2,
        "create must not render as a side effect"
    );
    let validated = Command::new(env!("CARGO_BIN_EXE_toniator"))
        .args(["validate", "-i", document.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(validated.status.success());
    assert!(
        String::from_utf8_lossy(&validated.stdout)
            .contains("container v1, document v2, migrations: empty")
    );
    let mutual = Command::new(env!("CARGO_BIN_EXE_toniator"))
        .args([
            "validate",
            "-i",
            document.to_str().unwrap(),
            "--canvas",
            "1x1",
        ])
        .output()
        .unwrap();
    assert_eq!(mutual.status.code(), Some(2));
    assert_eq!(String::from_utf8_lossy(&mutual.stdout), "");
    let moved = directory.join("moved.toniator");
    fs::rename(&document, &moved).unwrap();
    fs::remove_file(&source).unwrap();
    let rendered = directory.join("rendered.png");
    let render = Command::new(env!("CARGO_BIN_EXE_toniator"))
        .args([
            "render",
            "-i",
            moved.to_str().unwrap(),
            "-o",
            rendered.to_str().unwrap(),
            "--background",
            "black",
            "--max-family-candidates",
            "1048576",
        ])
        .output()
        .unwrap();
    assert!(
        render.status.success(),
        "{}",
        String::from_utf8_lossy(&render.stderr)
    );
    assert!(rendered.exists());
    let override_failure = Command::new(env!("CARGO_BIN_EXE_toniator"))
        .args([
            "render",
            "-i",
            moved.to_str().unwrap(),
            "-o",
            rendered.to_str().unwrap(),
            "--canvas",
            "1x1",
        ])
        .output()
        .unwrap();
    assert_eq!(override_failure.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&override_failure.stderr)
            .contains("container rendering does not accept document override flags")
    );
    let bad_create_input = Command::new(env!("CARGO_BIN_EXE_toniator"))
        .args([
            "document",
            "create",
            "-i",
            "../../assets/README.md",
            "-o",
            directory.join("bad.toniator").to_str().unwrap(),
            "--channel-model",
            "rgb",
            "--canvas",
            "1x1",
            "--density-x",
            "1",
            "--density-y",
            "1",
            "--rotation",
            "0",
            "--offset-x",
            "0",
            "--offset-y",
            "0",
            "--guard-steps",
            "2",
            "--size-min",
            "0",
            "--size-max",
            "1",
        ])
        .output()
        .unwrap();
    assert_eq!(bad_create_input.status.code(), Some(2));
    let bad_create_output = Command::new(env!("CARGO_BIN_EXE_toniator"))
        .args([
            "document",
            "create",
            "-i",
            "../../assets/raster-sample.png",
            "-o",
            directory.join("bad.txt").to_str().unwrap(),
            "--channel-model",
            "rgb",
            "--canvas",
            "1x1",
            "--density-x",
            "1",
            "--density-y",
            "1",
            "--rotation",
            "0",
            "--offset-x",
            "0",
            "--offset-y",
            "0",
            "--guard-steps",
            "2",
            "--size-min",
            "0",
            "--size-max",
            "1",
        ])
        .output()
        .unwrap();
    assert_eq!(bad_create_output.status.code(), Some(2));
    fs::remove_dir_all(directory).unwrap();
}
