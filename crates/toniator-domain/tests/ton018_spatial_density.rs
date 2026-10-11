use toniator_domain::{
    CanvasSpec, CoveragePolicy, DensityMetric2D, Document, DocumentCommand, PatternDefinition,
    PatternDefinitionEdit, PatternDefinitionId, PatternMechanism, PatternMechanismId,
    PatternOutputLayerId, PropertyFieldId, RandomSiteCharacter, SiteDensityModulation,
    SiteExclusionPolicy, SourceReference, SourceReferenceId, validate_pattern_definition,
};

/// Builds one current random definition with the process-owned spatial scale.
fn definition(character: RandomSiteCharacter) -> PatternDefinition {
    PatternDefinition::random_sites(
        PatternDefinitionId(1),
        "Spatial density",
        PatternMechanismId(2),
        PatternMechanismId(3),
        PatternMechanismId(4),
        PatternMechanismId(5),
        PatternOutputLayerId(6),
        character,
        7,
        SiteDensityModulation::Uniform,
        SiteExclusionPolicy::None,
        16_000_000,
        16_000_000,
        CoveragePolicy {
            guard_steps: 1,
            additional_margin: 0.0,
        },
    )
}

/// Proves inverse Feature size is canvas independent while guide axis counts keep their
/// established per-canvas appearance and the resolved conversion round-trips exactly.
///
/// # Panics
/// Panics if valid canvas projection changes scale, guide appearance, or inverse authority.
#[test]
fn feature_size_encoding_and_guide_projection_are_stable() {
    for canvas in [
        CanvasSpec {
            width: 1024.0,
            height: 1024.0,
        },
        CanvasSpec {
            width: 900.0,
            height: 620.0,
        },
        CanvasSpec {
            width: 620.0,
            height: 900.0,
        },
    ] {
        assert_eq!(
            DensityMetric2D::default_for_canvas(&canvas)
                .unwrap()
                .density,
            100.0
        );
        for size in [0.25, 0.5, 1.0, 2.0] {
            for aspect in [0.5, 1.0, 2.0] {
                let metric = DensityMetric2D {
                    density: 100.0 / size,
                    aspect,
                };
                let resolved = metric.resolve(&canvas).unwrap();
                let recovered = DensityMetric2D::from_resolved(&canvas, &resolved).unwrap();
                assert!((recovered.density - metric.density).abs() < 1.0e-10);
                assert!((recovered.aspect - aspect).abs() < 1.0e-12);
                if aspect == 1.0 {
                    let long = canvas.width.max(canvas.height);
                    assert!(
                        (canvas.width / resolved.across_x - long * size / 100.0).abs() < 1.0e-10
                    );
                    assert!(
                        (canvas.height / resolved.across_y - long * size / 100.0).abs() < 1.0e-10
                    );
                }
            }
        }
    }
}

/// Proves Raw, Clustered, and Jittered require authored spacing while Even has only its
/// minimum-center-distance scale; edits cannot publish a redundant second Even scale.
///
/// # Panics
/// Panics if validation accepts missing/redundant scale or rejects a valid structural edit.
#[test]
fn random_process_has_exactly_one_authored_spatial_scale() {
    for character in [
        RandomSiteCharacter::RawUniform,
        RandomSiteCharacter::Clustered {
            cluster_density: 12.0,
            cluster_spread: 5.0,
            cluster_strength: 0.8,
        },
        RandomSiteCharacter::Stratified { jitter: 0.5 },
    ] {
        let mut current = definition(character);
        validate_pattern_definition(&current).unwrap();
        let PatternMechanism::RandomSiteProcess {
            nominal_spacing, ..
        } = &mut current.mechanisms[0]
        else {
            panic!("random process must be first");
        };
        assert_eq!(*nominal_spacing, Some(10.24));
        *nominal_spacing = None;
        assert_eq!(
            validate_pattern_definition(&current).unwrap_err().path(),
            "pattern_definitions.mechanisms.random_sites.nominal_spacing"
        );
    }
    let mut even = definition(RandomSiteCharacter::Even {
        minimum_center_distance: 8.0,
    });
    validate_pattern_definition(&even).unwrap();
    let PatternMechanism::RandomSiteProcess {
        nominal_spacing, ..
    } = &mut even.mechanisms[0]
    else {
        panic!("random process must be first");
    };
    assert_eq!(*nominal_spacing, None);
    *nominal_spacing = Some(10.24);
    assert_eq!(
        validate_pattern_definition(&even).unwrap_err().path(),
        "pattern_definitions.mechanisms.random_sites.nominal_spacing"
    );
}

/// Proves the active descriptor exposes the authored spatial scale, hides ineffective scatter
/// Density aspect, and publishes a valid structural edit through document command authority.
///
/// # Panics
/// Panics if active descriptor or shared-definition command behavior diverges.
#[test]
fn scatter_spacing_descriptor_and_command_are_authoritative() {
    let base = Document::new_default_document(
        CanvasSpec {
            width: 160.0,
            height: 120.0,
        },
        SourceReference::Assigned(SourceReferenceId::new("density-source").unwrap()),
    )
    .unwrap();
    let mut bundle = base.pattern_definition_bundles()[0].clone();
    let definition_id = base.pattern_settings().definition_id;
    bundle.definition = definition(RandomSiteCharacter::RawUniform);
    bundle.definition.id = definition_id;
    bundle.definition.output_layers[0].id = bundle.output_settings[0].output_layer_id;
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
    let descriptors = document.property_descriptors();
    assert!(
        descriptors
            .iter()
            .any(|item| item.field == PropertyFieldId::RandomNominalSpacing)
    );
    assert!(
        !descriptors
            .iter()
            .any(|item| item.field == PropertyFieldId::DensityAspect)
    );
    let before = document.pattern_definition_bundles()[0].definition.clone();
    let command = DocumentCommand::EditSharedPatternDefinition {
        definition_id,
        base_definition: before.clone(),
        edit: PatternDefinitionEdit::SetRandomNominalSpacing {
            mechanism_id: PatternMechanismId(2),
            nominal_spacing: 12.0,
        },
    };
    let changed = document.apply_command(&command).unwrap().0;
    let PatternMechanism::RandomSiteProcess {
        nominal_spacing, ..
    } = &changed.pattern_definition_bundles()[0]
        .definition
        .mechanisms[0]
    else {
        panic!("random process must be first");
    };
    assert_eq!(*nominal_spacing, Some(12.0));
    let invalid = DocumentCommand::EditSharedPatternDefinition {
        definition_id,
        base_definition: before,
        edit: PatternDefinitionEdit::SetRandomNominalSpacing {
            mechanism_id: PatternMechanismId(2),
            nominal_spacing: 0.0,
        },
    };
    assert!(document.apply_command(&invalid).is_err());
}
