use toniator_domain::{
    CanvasSpec, ChannelId, DensityMetric2D, Document, DocumentCommand, DocumentHistory,
    DocumentSession, MarkGeometryResponse, PatternDefinitionDraft, PatternDefinitionRecipe,
    PatternGeometryResponse, PatternStructureRecipe, SourceReference, TranslationEditedAxis,
};

/// Builds one current RGB document with deterministic base authority.
fn document(width: f64, height: f64) -> Document {
    Document::new_default_document(CanvasSpec { width, height }, SourceReference::Unassigned)
        .expect("finite positive canvas builds a current document")
}

/// Builds one current ID-free straight-grid mark recipe for replacement tests.
fn replacement_recipe(name: &str) -> PatternDefinitionRecipe {
    PatternDefinitionRecipe::marks(PatternStructureRecipe::StraightGrid(
        PatternDefinitionDraft {
            name: name.to_owned(),
            coverage: toniator_domain::CoveragePolicy {
                guard_steps: 3,
                additional_margin: 1.0,
            },
        },
    ))
}

/// Proves density/aspect round-trips and the 1:1, 2:1, and 1:2 spacing layouts.
#[test]
fn density_formula_round_trips_rectangular_spacing_ratios() {
    let canvas = CanvasSpec {
        width: 900.0,
        height: 600.0,
    };
    for aspect in [1.0, 2.0, 0.5] {
        let authored = DensityMetric2D {
            density: 72.0,
            aspect,
        };
        let resolved = authored.resolve(&canvas).expect("density resolves");
        assert!((resolved.across_x * resolved.across_y - 72.0_f64.powi(2)).abs() < 1.0e-9);
        assert!(
            (canvas.width * resolved.across_y / (canvas.height * resolved.across_x) - aspect).abs()
                < 1.0e-12
        );
        let round_trip =
            DensityMetric2D::from_resolved(&canvas, &resolved).expect("resolved pair reverses");
        assert!((round_trip.density - authored.density).abs() < 1.0e-12);
        assert!((round_trip.aspect - authored.aspect).abs() < 1.0e-12);
    }
}

/// Proves fresh density is resolution-independent and normalizes site coverage across canvas aspect.
///
/// # Panics
///
/// Panics when fresh square canvases stop resolving to one hundred sites per
/// edge, or rectangular canvases stop preserving one hundred sites on the long
/// edge and proportional coverage on the short edge.
#[test]
fn default_density_normalizes_geometry_count_across_resolution_and_aspect() {
    let small = DensityMetric2D::default_for_canvas(&CanvasSpec {
        width: 256.0,
        height: 256.0,
    })
    .expect("small canvas default");
    let large = DensityMetric2D::default_for_canvas(&CanvasSpec {
        width: 1024.0,
        height: 1024.0,
    })
    .expect("large canvas default");
    let landscape_canvas = CanvasSpec {
        width: 200.0,
        height: 100.0,
    };
    let portrait_canvas = CanvasSpec {
        width: 100.0,
        height: 200.0,
    };
    let landscape =
        DensityMetric2D::default_for_canvas(&landscape_canvas).expect("landscape canvas default");
    let portrait =
        DensityMetric2D::default_for_canvas(&portrait_canvas).expect("portrait canvas default");

    assert_eq!(small.density, 100.0);
    assert_eq!(large.density, 100.0);
    assert_eq!(small.aspect, 1.0);
    assert_eq!(large.aspect, 1.0);
    assert!((landscape.density - 100.0 / 2.0_f64.sqrt()).abs() < 1.0e-12);
    assert_eq!(portrait.density, landscape.density);
    let landscape_resolved = landscape
        .resolve(&landscape_canvas)
        .expect("landscape default resolves");
    let portrait_resolved = portrait
        .resolve(&portrait_canvas)
        .expect("portrait default resolves");
    assert!((landscape_resolved.across_x - 100.0).abs() < 1.0e-12);
    assert!((landscape_resolved.across_y - 50.0).abs() < 1.0e-12);
    assert!((portrait_resolved.across_x - 50.0).abs() < 1.0e-12);
    assert!((portrait_resolved.across_y - 100.0).abs() < 1.0e-12);
}

/// Proves ALL replacement clears only pattern-relative channel intent and one Undo restores it exactly.
#[test]
fn all_recipe_replacement_is_one_exact_reversible_reset_transaction() {
    let mut history = DocumentHistory::new(
        DocumentSession::new(document(320.0, 180.0)).expect("main history starts"),
    );
    let original_base = history.document().pattern_settings().clone();
    let original_definition = history.document().pattern_definition_bundles()[0]
        .definition
        .clone();
    history
        .apply(
            &DocumentCommand::ReplaceChannelPatternDefinitionOverrideRecipe {
                base: original_base.clone(),
                channel_id: ChannelId(1),
                base_definition: original_definition.clone(),
                recipe: replacement_recipe("Channel custom"),
            },
        )
        .expect("channel override recipe applies");
    let density = history
        .document()
        .set_channel_density_for_effective(
            ChannelId(1),
            DensityMetric2D {
                density: 30.0,
                aspect: 2.0,
            },
        )
        .expect("density delta builds");
    history.apply(&density).expect("density delta applies");
    let rotation = history
        .document()
        .set_channel_pattern_rotation_for_effective(ChannelId(1), 27.0)
        .expect("rotation delta builds");
    history.apply(&rotation).expect("rotation delta applies");
    let shape_rotation = history
        .document()
        .set_channel_shape_rotation_for_effective(ChannelId(1), -12.0)
        .expect("shape rotation delta builds");
    history
        .apply(&shape_rotation)
        .expect("shape rotation delta applies");
    let output_id = history
        .document()
        .effective_channel_pattern(ChannelId(1))
        .expect("channel resolves")
        .output_settings[0]
        .output_layer_id;
    let response = history
        .document()
        .set_channel_output_response_for_effective(
            ChannelId(1),
            output_id,
            PatternGeometryResponse::Marks(MarkGeometryResponse {
                minimum_fill: 0.2,
                maximum_fill: 1.2,
            }),
        )
        .expect("output response delta builds");
    history.apply(&response).expect("output response applies");
    history
        .apply(&DocumentCommand::SetTranslationAxis {
            channel_id: ChannelId(1),
            edited_axis: TranslationEditedAxis::X,
            value: 9.5,
        })
        .expect("translation applies");
    history
        .apply(&DocumentCommand::SetOpacity {
            channel_id: ChannelId(1),
            opacity: 0.4,
        })
        .expect("opacity applies");
    let before = history.document().clone();
    assert_eq!(
        history
            .document()
            .channels_with_pattern_replacement_intent(),
        vec![ChannelId(1)]
    );

    history
        .apply(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base: history.document().pattern_settings().clone(),
            base_definition: original_definition,
            recipe: replacement_recipe("ALL replacement"),
        })
        .expect("ALL replacement applies atomically");
    let instance = history
        .document()
        .channel_pattern_instance(ChannelId(1))
        .expect("channel remains");
    assert_eq!(instance.definition_override, None);
    assert_eq!(instance.layout_delta.density, None);
    assert_eq!(instance.layout_delta.rotation_degrees, None);
    assert_eq!(instance.shape_rotation_delta_degrees, None);
    assert!(instance.output_response_deltas.is_empty());
    assert_eq!(instance.layout_delta.translation_x, 9.5);
    assert_eq!(
        history
            .document()
            .modeled_channel(ChannelId(1))
            .expect("modeled channel remains")
            .opacity,
        0.4
    );
    assert!(
        history
            .document()
            .channels_with_pattern_replacement_intent()
            .is_empty()
    );

    history
        .undo()
        .expect("one undo restores the exact former intent");
    assert_eq!(history.document(), &before);
}
