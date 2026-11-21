use toniator_domain::*;
use toniator_patterns::*;
use toniator_sampling::SourceField;

/// Realizes a constant endpoint through either persisted adaptive output producer.
/// # Panics
/// Panics if the ordinary recipe, source, grid or canonical realization rejects fixed valid inputs.
fn endpoint(blue_noise: bool, tone: f64) -> CanonicalMarkRealization {
    let original = Document::new_default_document(
        CanvasSpec {
            width: 64.0,
            height: 64.0,
        },
        SourceReference::Unassigned,
    )
    .unwrap();
    let mut recipe = adaptive_square_marks_recipe();
    if blue_noise {
        let PatternStructureRecipe::OrderedOutputs { outputs, .. } = &mut recipe.structure else {
            panic!("ordered adaptive recipe")
        };
        outputs[0] = PatternOutputRealizationRecipe::AdaptiveSquareBlueNoiseMarks;
    }
    let document = original
        .apply_command(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base: original.pattern_settings().clone(),
            base_definition: original.pattern_definition_bundles()[0].definition.clone(),
            recipe,
        })
        .unwrap()
        .0;
    let plan = resolve_document_pattern_pipeline_for_channel(
        &document,
        document.pattern_definition_for(ChannelId(1)).unwrap(),
        SourceWeighting::canonical(SourceMappingComponent::Alpha),
    )
    .unwrap();
    let request = GridInspectRequest {
        canvas: document.canvas().clone(),
        density: DensityMetric2D {
            density: 16.0,
            aspect: 1.0,
        }
        .resolve(document.canvas())
        .unwrap(),
        rotation_degrees: 0.0,
        translation_x: 0.0,
        translation_y: 0.0,
        guard_steps: 2,
        support_radius: 32.0,
        max_family_candidates: 100000,
    };
    let family = evaluate_typed_family_product_with_source_cancellable(
        &plan.family,
        &request,
        None,
        &|| false,
    )
    .unwrap();
    let source = SourceField::from_straight_rgba8(1, 1, vec![255; 4]).unwrap();
    realize_typed_canonical_marks_with_response_cancellable(
        &document,
        &family,
        &plan,
        &source,
        document.canvas(),
        CanonicalMarkRequest {
            mapping: SourceMapping::canonical(SourceMappingComponent::Alpha),
            sampled_paint: false,
            response: MarkResponse {
                minimum_fill: 0.0,
                maximum_fill: 1.0,
                rotation_offset_degrees: 0.0,
            },
            max_transformed_curve_segment_instances: 100000,
        },
        GeometryResponseInput::RawProbe(tone),
        &|| false,
    )
    .unwrap()
    .output
}

/// Distinguishes producer/cache identities even when empty or full endpoint geometry is identical.
/// # Panics
/// Panics if a mode aliases realization identity or changes exact shared endpoint silhouettes.
#[test]
fn adaptive_blue_noise_endpoint_geometry_matches_but_mode_identity_remains_distinct() {
    for tone in [0.0, 1.0] {
        let nearest = endpoint(false, tone);
        let mixed = endpoint(true, tone);
        assert_eq!(nearest.family_fingerprint, mixed.family_fingerprint);
        assert_eq!(nearest.marks, mixed.marks);
        assert_ne!(
            nearest.realization_fingerprint,
            mixed.realization_fingerprint
        );
    }
}
