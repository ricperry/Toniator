use toniator_domain::{
    CanvasSpec, ChannelId, Document, DocumentCommand, DocumentHistory, DocumentSession,
    PatternCapabilityScope, PatternStructureRecipe, PropertyFieldId, SiteDensityModulation,
    SourceReference,
};
use toniator_patterns::{
    GridInspectRequest, PresetRegistry, evaluate_typed_family, resolve_pattern_pipeline,
};

/// Creates a fresh default document whose output is compared against a preset
/// reconstruction through the shared canonical typed family boundary.
fn history() -> DocumentHistory {
    let document = Document::new_default_document(
        CanvasSpec {
            width: 1024.0,
            height: 1024.0,
        },
        SourceReference::Unassigned,
    )
    .unwrap();
    DocumentHistory::new(DocumentSession::new(document).unwrap())
}

/// Resolves the current selected channel through document-owned inheritance
/// before constructing the shared family request; this test never recreates
/// effective layout arithmetic or adds a preset-specific evaluation path.
fn request(document: &Document) -> GridInspectRequest {
    let channel = document
        .effective_channel_pattern(ChannelId(1))
        .expect("the default selected channel resolves through document authority");
    GridInspectRequest {
        canvas: document.canvas().clone(),
        density: channel.resolved_density,
        rotation_degrees: channel.pattern_rotation_degrees,
        translation_x: channel.translation_x,
        translation_y: channel.translation_y,
        guard_steps: 2,
        support_radius: 4.5,
        max_family_candidates: 1_000_000,
    }
}

/// Reports whether one ID-free preset structure retains artwork-weighted site placement through
/// any supported wrapper, without assigning an evaluator identity or catalog name to the recipe.
fn recipe_uses_artwork_weighted_density(recipe: &PatternStructureRecipe) -> bool {
    match recipe {
        PatternStructureRecipe::RandomSites {
            density_modulation, ..
        } => matches!(
            density_modulation,
            SiteDensityModulation::ArtworkWeighted { .. }
        ),
        PatternStructureRecipe::ConnectionPaths { definition, .. }
        | PatternStructureRecipe::MazeWalls { definition, .. }
        | PatternStructureRecipe::AuthoredClosedShapeMarks { definition, .. }
        | PatternStructureRecipe::CurveMotifPaths { definition, .. }
        | PatternStructureRecipe::VoronoiRegions { definition }
        | PatternStructureRecipe::GuideFaceRegions { definition, .. }
        | PatternStructureRecipe::OrderedOutputs { definition, .. } => {
            recipe_uses_artwork_weighted_density(definition)
        }
        PatternStructureRecipe::StraightGrid(_)
        | PatternStructureRecipe::GeneralizedStraightGuides { .. }
        | PatternStructureRecipe::ParametricCurve { .. } => false,
    }
}

/// Proves representative grid and random recipes enter exactly the existing
/// canonical family evaluator with deterministic output and no preset-name path.
#[test]
fn bundled_grid_and_random_recipes_have_deterministic_canonical_family_parity() {
    let registry = PresetRegistry::bundled();
    let mut history = history();
    for id in ["straight-grid-circles", "even-random-circles"] {
        registry
            .apply_to_selected(&mut history, ChannelId(1), id)
            .unwrap();
        let definition = history
            .document()
            .pattern_definition_for(ChannelId(1))
            .unwrap();
        let first = evaluate_typed_family(definition, &request(history.document())).unwrap();
        let second = evaluate_typed_family(definition, &request(history.document())).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            resolve_pattern_pipeline(definition)
                .unwrap()
                .family
                .provenance
                .definition_id,
            definition.id.0
        );
    }
}

/// Proves every bundled recipe except source-weighted placement exposes and successfully consumes
/// one ordinary rotation, while the source-weighted catalog entry omits that incompatible control.
#[test]
fn bundled_rotation_capability_matches_rotated_family_evaluation() {
    let registry = PresetRegistry::bundled();
    let mut history = history();
    for (ordinal, entry) in registry.entries().iter().enumerate() {
        registry
            .apply_to_selected(&mut history, ChannelId(1), &entry.metadata.id)
            .expect("bundled recipe applies to the selected channel");
        let projection = history
            .document()
            .pattern_capabilities(PatternCapabilityScope::Channel(ChannelId(1)))
            .expect("bundled recipe projects selected-channel controls");
        let source_weighted = recipe_uses_artwork_weighted_density(&entry.recipe.structure);
        assert_eq!(
            projection
                .active_controls
                .iter()
                .any(|descriptor| descriptor.field == PropertyFieldId::RotationDegrees),
            !source_weighted,
            "{} rotation availability",
            entry.metadata.id
        );
        if source_weighted {
            assert_eq!(
                request(history.document()).rotation_degrees,
                0.0,
                "{} sends normalized zero rotation to its family/cache request",
                entry.metadata.id
            );
            continue;
        }
        let rotation = history
            .document()
            .set_channel_pattern_rotation_for_effective(
                ChannelId(1),
                23.0 + f64::from(u32::try_from(ordinal).expect("bundled ordinal fits u32")),
            )
            .expect("ordinary rotated preset builds a channel delta");
        history
            .apply(&rotation)
            .expect("ordinary rotated preset accepts a channel delta");
        let definition = history
            .document()
            .pattern_definition_for(ChannelId(1))
            .expect("selected definition remains available");
        evaluate_typed_family(definition, &request(history.document())).unwrap_or_else(|error| {
            panic!(
                "{} must evaluate a rotated family: {}",
                entry.metadata.id, error
            )
        });
    }
}

/// Proves a selected source-weighted replacement prunes its incompatible rotation delta while a
/// shared base rotation remains dormant and returns when a non-weighted recipe replaces it again.
#[test]
fn source_weighted_preset_temporarily_suppresses_and_then_restores_channel_rotation() {
    let registry = PresetRegistry::bundled();
    let mut history = history();
    let mut settings = history.document().pattern_settings().clone();
    settings.pattern_rotation_degrees = 11.0;
    history
        .apply(&DocumentCommand::SetDocumentPatternSettings {
            base: history.document().pattern_settings().clone(),
            settings,
        })
        .expect("shared base rotation applies");
    registry
        .apply_to_selected(&mut history, ChannelId(1), "even-random-circles")
        .expect("ordinary selected preset applies");
    let rotation = history
        .document()
        .set_channel_pattern_rotation_for_effective(ChannelId(1), 37.0)
        .expect("ordinary selected rotation builds");
    history
        .apply(&rotation)
        .expect("ordinary selected rotation applies");
    registry
        .apply_to_selected(
            &mut history,
            ChannelId(1),
            "source-weighted-dispersion-voronoi",
        )
        .expect("source-weighted selected preset applies");
    assert_eq!(
        history
            .document()
            .channel_pattern_instance(ChannelId(1))
            .expect("weighted selected intent remains stored")
            .layout_delta
            .rotation_degrees,
        None
    );
    assert_eq!(
        history
            .document()
            .effective_channel_pattern(ChannelId(1))
            .expect("source-weighted effective pattern resolves")
            .pattern_rotation_degrees,
        0.0
    );
    registry
        .apply_to_selected(&mut history, ChannelId(1), "even-random-circles")
        .expect("ordinary selected preset restores");
    assert_eq!(
        history
            .document()
            .effective_channel_pattern(ChannelId(1))
            .expect("ordinary effective pattern resolves")
            .pattern_rotation_degrees,
        11.0
    );
}
