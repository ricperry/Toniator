use toniator_domain::{
    ArtworkWeightResponse, CanvasSpec, DensityMetric2D, Document, DocumentCommand,
    DocumentConfiguration, DocumentHistory, DocumentId, DocumentSession, DocumentSessionError,
    InvalidationLevel, ModeledMappingFieldEdit, PatternDefinition, PatternDefinitionBundle,
    PatternMechanism, PatternMechanismId, PatternOutputLayer, PatternOutputLayerId,
    PatternOutputRealization, PatternOutputSettings, RandomSiteCharacter, RegionGeometryResponse,
    RegionResizeAlgorithm, RegionSamplingStrategy, RegionSourceIntent, Revision,
    SiteDensityModulation, SiteExclusionPolicy, SiteUseFilter, SourceMapping,
    SourceMappingComponent, SourceReference, SourceReferenceId,
};

/// Builds one source-backed current RGB document with explicit project identity and dimensions.
fn document(id: u64, width: f64, height: f64, source: &str) -> Document {
    Document::new_default_document(
        CanvasSpec { width, height },
        SourceReference::Assigned(
            SourceReferenceId::new(source.to_owned()).expect("source identity validates"),
        ),
    )
    .and_then(|default| {
        Document::with_source_topology_and_authored_structures(
            DocumentId(id),
            default.canvas().clone(),
            default.source().clone(),
            default.pattern_definition_bundles().to_vec(),
            default.pattern_settings().clone(),
            default.channel_model().expect("modeled default"),
            default.channel_topology().expect("modeled default").clone(),
            default.authored_structures().to_vec(),
        )
    })
    .expect("current document validates")
}

/// Rebuilds one modeled document with replacement definition bundles and unchanged project authority.
fn with_bundles(base: &Document, bundles: Vec<PatternDefinitionBundle>) -> Document {
    Document::with_source_topology_and_authored_structures(
        base.id(),
        base.canvas().clone(),
        base.source().clone(),
        bundles,
        base.pattern_settings().clone(),
        base.channel_model().expect("modeled document"),
        base.channel_topology().expect("modeled document").clone(),
        base.authored_structures().to_vec(),
    )
    .expect("replacement bundles validate")
}

/// Builds one two-output mark document for site-filter and painter-order classification.
fn multi_output_document() -> Document {
    let base = document(70, 640.0, 480.0, "multi-output-source");
    let mut bundle = base.pattern_definition_bundles()[0].clone();
    let mut second_output = bundle.definition.output_layers[0].clone();
    second_output.id = PatternOutputLayerId(2);
    bundle.definition.output_layers.push(second_output);
    bundle.output_settings.push(PatternOutputSettings {
        output_layer_id: PatternOutputLayerId(2),
        response: bundle.output_settings[0].response.clone(),
    });
    with_bundles(&base, vec![bundle])
}

/// Builds one artwork-weighted region document whose response can alter family construction.
fn artwork_weighted_region_document() -> Document {
    let base = document(71, 640.0, 480.0, "region-source");
    let site_id = PatternMechanismId(5);
    let output_id = PatternOutputLayerId(6);
    let definition = PatternDefinition::random_sites(
        base.pattern_settings().definition_id,
        "Artwork-weighted regions",
        PatternMechanismId(2),
        PatternMechanismId(3),
        PatternMechanismId(4),
        site_id,
        output_id,
        RandomSiteCharacter::RawUniform,
        19,
        SiteDensityModulation::ArtworkWeighted {
            mapping: SourceMapping::canonical(SourceMappingComponent::Luminance),
            strength: 0.8,
            response: ArtworkWeightResponse::Smoothstep,
        },
        SiteExclusionPolicy::None,
        20_000,
        20_000,
        toniator_domain::CoveragePolicy {
            guard_steps: 2,
            additional_margin: 0.0,
        },
    );
    let mut definition = definition;
    definition.output_layers = vec![PatternOutputLayer::all(
        output_id,
        PatternOutputRealization::Regions {
            source: RegionSourceIntent::VoronoiSites {
                site_mechanism_id: site_id,
            },
        },
    )];
    let bundle = PatternDefinitionBundle {
        definition,
        output_settings: vec![PatternOutputSettings {
            output_layer_id: output_id,
            response: toniator_domain::PatternGeometryResponse::Regions(
                RegionGeometryResponse::default(),
            ),
        }],
    };
    with_bundles(&base, vec![bundle])
}

/// Proves binding replaces only authored configuration and retains destination project authority.
///
/// # Panics
///
/// Panics when capture or complete destination binding loses exact authored
/// intent, or changes destination identity, canvas, or source reference.
#[test]
fn configuration_binding_retains_destination_identity_canvas_and_source() {
    let source = document(41, 900.0, 620.0, "source-artwork");
    let destination = document(99, 1024.0, 512.0, "destination-artwork");
    let configuration = DocumentConfiguration::capture(&source);

    let bound = configuration
        .bind(&destination)
        .expect("configuration binds to destination");

    assert_eq!(bound.id(), destination.id());
    assert_eq!(bound.canvas(), destination.canvas());
    assert_eq!(bound.source(), destination.source());
    assert_eq!(DocumentConfiguration::capture(&bound), configuration);
}

/// Proves one replacement classifies simultaneous geometry and mapping changes without masking Source.
///
/// # Panics
///
/// Panics when the atomic apply does not advance exactly once, misses an
/// affected channel, reports weaker invalidation, or fails exact Undo/Redo.
#[test]
fn configuration_apply_combines_geometry_and_mapping_invalidation_and_history() {
    let before = document(7, 640.0, 480.0, "current-artwork");
    let channel_id = before
        .channel_topology()
        .expect("modeled topology")
        .channels()[0]
        .id;
    let (mapped, _) = before
        .apply_command(&DocumentCommand::SetModeledMappingField {
            channel_id,
            edit: ModeledMappingFieldEdit::Gain(0.75),
        })
        .expect("mapping candidate applies");
    let mut settings = mapped.pattern_settings().clone();
    settings.density = DensityMetric2D {
        density: settings.density.density + 10.0,
        aspect: settings.density.aspect,
    };
    let (after, _) = mapped
        .apply_command(&DocumentCommand::SetDocumentPatternSettings {
            base: mapped.pattern_settings().clone(),
            settings,
        })
        .expect("geometry candidate applies");
    let configuration = DocumentConfiguration::capture(&after);
    let mut history =
        DocumentHistory::new(DocumentSession::new(before.clone()).expect("main history validates"));

    let result = history
        .apply_document_configuration(&before, Revision(0), &configuration)
        .expect("configuration applies");

    assert!(!result.unchanged);
    assert_eq!(result.invalidation, Some(InvalidationLevel::Source));
    assert_eq!(
        result.affected_channels,
        after
            .channel_topology()
            .expect("modeled topology")
            .channels()
            .iter()
            .map(|channel| channel.id)
            .collect::<Vec<_>>()
    );
    assert_eq!(history.revision(), Revision(1));
    assert_eq!(history.document(), &after);
    history.undo().expect("undo succeeds");
    assert_eq!(history.document(), &before);
    history.redo().expect("redo succeeds");
    assert_eq!(history.document(), &after);
}

/// Proves equal and stale replacement callbacks preserve revision and history stacks.
///
/// # Panics
///
/// Panics when a no-op clears redo or advances authority, or when a stale base
/// or revision publishes configuration.
#[test]
fn configuration_noop_and_stale_apply_preserve_authority() {
    let initial = document(5, 320.0, 180.0, "current-artwork");
    let channel_id = initial
        .channel_topology()
        .expect("modeled topology")
        .channels()[0]
        .id;
    let mut history = DocumentHistory::new(
        DocumentSession::new(initial.clone()).expect("main history validates"),
    );
    history
        .apply(&DocumentCommand::SetOpacity {
            channel_id,
            opacity: 0.5,
        })
        .expect("one history step applies");
    history.undo().expect("history step undoes");
    assert!(history.can_redo());
    let revision = history.revision();
    let configuration = DocumentConfiguration::capture(history.document());
    let base = history.document().clone();

    let result = history
        .apply_document_configuration(&base, revision, &configuration)
        .expect("equal configuration is accepted");
    assert!(result.unchanged);
    assert_eq!(history.revision(), revision);
    assert!(history.can_redo());

    let stale = history.apply_document_configuration(
        &base,
        Revision(revision.0.saturating_sub(1)),
        &DocumentConfiguration::capture(&initial),
    );
    assert!(matches!(stale, Err(DocumentSessionError::Validation(_))));
    assert_eq!(history.document(), &base);
    assert_eq!(history.revision(), revision);
    assert!(history.can_redo());
}

/// Proves whole-configuration classification preserves filter, painter-order, and region authority.
///
/// # Panics
///
/// Panics when a site-use filter is classified above Realization, painter-only
/// output reordering above Presentation, or a region algorithm/sampling change
/// below Family.
#[test]
fn configuration_apply_uses_precise_output_and_region_invalidation() {
    let base = multi_output_document();
    let mut filtered_bundle = base.pattern_definition_bundles()[0].clone();
    filtered_bundle.definition.output_layers[1].source_filter = SiteUseFilter::SitesUnusedBy {
        output_layer_id: PatternOutputLayerId(1),
    };
    let filtered = with_bundles(&base, vec![filtered_bundle]);
    let mut history =
        DocumentHistory::new(DocumentSession::new(base.clone()).expect("filter history validates"));
    let filter_result = history
        .apply_document_configuration(
            &base,
            Revision(0),
            &DocumentConfiguration::capture(&filtered),
        )
        .expect("filter configuration applies");
    assert_eq!(
        filter_result.invalidation,
        Some(InvalidationLevel::Realization)
    );

    let mut ordered_bundle = base.pattern_definition_bundles()[0].clone();
    ordered_bundle.definition.output_layers.swap(0, 1);
    ordered_bundle.output_settings.swap(0, 1);
    let ordered = with_bundles(&base, vec![ordered_bundle]);
    let mut history =
        DocumentHistory::new(DocumentSession::new(base.clone()).expect("order history validates"));
    let order_result = history
        .apply_document_configuration(
            &base,
            Revision(0),
            &DocumentConfiguration::capture(&ordered),
        )
        .expect("order configuration applies");
    assert_eq!(
        order_result.invalidation,
        Some(InvalidationLevel::Presentation)
    );

    let region = artwork_weighted_region_document();
    let mut changed_bundle = region.pattern_definition_bundles()[0].clone();
    let toniator_domain::PatternGeometryResponse::Regions(response) =
        &mut changed_bundle.output_settings[0].response
    else {
        panic!("region response exists")
    };
    response.algorithm = RegionResizeAlgorithm::UniformOffset;
    response.sampling = RegionSamplingStrategy::AreaAverage;
    let changed = with_bundles(&region, vec![changed_bundle]);
    let mut history = DocumentHistory::new(
        DocumentSession::new(region.clone()).expect("region history validates"),
    );
    let region_result = history
        .apply_document_configuration(
            &region,
            Revision(0),
            &DocumentConfiguration::capture(&changed),
        )
        .expect("region configuration applies");
    assert_eq!(region_result.invalidation, Some(InvalidationLevel::Family));
}

/// Proves nested artwork-weighted mapping retains Source precedence with simultaneous layout edits.
///
/// # Panics
///
/// Panics when a reusable source interpretation change is reported as ordinary
/// family construction or when the target channel is omitted.
#[test]
fn configuration_apply_classifies_artwork_weighted_mapping_as_source() {
    let base = artwork_weighted_region_document();
    let mut changed_bundle = base.pattern_definition_bundles()[0].clone();
    let mapping = changed_bundle
        .definition
        .mechanisms
        .iter_mut()
        .find_map(|mechanism| match mechanism {
            PatternMechanism::SiteDensityModulation {
                modulation: SiteDensityModulation::ArtworkWeighted { mapping, .. },
                ..
            } => Some(mapping),
            _ => None,
        })
        .expect("artwork-weighted mapping exists");
    mapping.gain = 0.5;
    let changed = with_bundles(&base, vec![changed_bundle]);
    let mut history = DocumentHistory::new(
        DocumentSession::new(base.clone()).expect("mapping history validates"),
    );
    let result = history
        .apply_document_configuration(
            &base,
            Revision(0),
            &DocumentConfiguration::capture(&changed),
        )
        .expect("mapping configuration applies");
    assert_eq!(result.invalidation, Some(InvalidationLevel::Source));
    assert_eq!(
        result.affected_channels,
        base.channel_topology()
            .expect("modeled topology")
            .channels()
            .iter()
            .map(|channel| channel.id)
            .collect::<Vec<_>>()
    );
    let mut settings = changed.pattern_settings().clone();
    settings.density.density *= 0.5;
    let combined = changed
        .apply_command(
            &toniator_domain::DocumentCommand::SetDocumentPatternSettings {
                base: changed.pattern_settings().clone(),
                settings,
            },
        )
        .expect("combined layout validates")
        .0;
    let mut combined_history = DocumentHistory::new(
        DocumentSession::new(base.clone()).expect("combined history validates"),
    );
    let result = combined_history
        .apply_document_configuration(
            &base,
            Revision(0),
            &DocumentConfiguration::capture(&combined),
        )
        .expect("combined configuration applies");
    assert_eq!(result.invalidation, Some(InvalidationLevel::Source));
}
