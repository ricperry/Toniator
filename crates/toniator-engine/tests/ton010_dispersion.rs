use toniator_domain::{DispersionLayout, PatternStructureRecipe, RandomSiteCharacter};
use toniator_engine::reference_calibration::reference_document;
use toniator_patterns::PresetRegistry;

/// Reproduces the current bundled Wizard recipe with an authored 8px intrinsic layout.
/// # Panics
/// Panics if the bounded recipe loses its Scatter foundation.
fn set_layout(structure: &mut PatternStructureRecipe, layout: DispersionLayout) {
    match structure {
        PatternStructureRecipe::RandomSites {
            character,
            nominal_spacing,
            ..
        } => {
            *character = RandomSiteCharacter::Dispersion { layout };
            *nominal_spacing = Some(8.0);
        }
        PatternStructureRecipe::AuthoredResources { definition, .. }
        | PatternStructureRecipe::OrderedOutputs { definition, .. }
        | PatternStructureRecipe::AuthoredClosedShapeMarks { definition, .. } => {
            set_layout(definition, layout)
        }
        _ => panic!("expected Scatter foundation"),
    }
}

/// Keeps exact default density a valid low-tone reference rather than dispatching a no-op command.
/// # Panics
/// Panics if 128 authored spacings of 8px no longer construct the reference at density 100.
#[test]
fn authored_eight_pixel_low_reference_accepts_default_density_noop() {
    for layout in [
        DispersionLayout::Bayer,
        DispersionLayout::FloydSteinberg,
        DispersionLayout::InkQuadtree,
        DispersionLayout::TangentSpacing,
        DispersionLayout::TangentSize,
        DispersionLayout::TangentBlend,
        DispersionLayout::ClusterScreen,
    ] {
        let mut recipe = PresetRegistry::bundled()
            .entries()
            .iter()
            .find(|entry| entry.metadata.id == "even-random-circles")
            .unwrap()
            .recipe
            .clone();
        set_layout(&mut recipe.structure, layout);
        let (document, _, channel) = reference_document(&recipe, 128).unwrap();
        assert_eq!(document.pattern_settings().density.density, 100.0);
        let effective = document.effective_channel_pattern(channel).unwrap();
        let output = effective.output_settings[0].output_layer_id;
        let density = document.effective_mark_density(channel, output).unwrap();
        assert!(
            density
                .uniform_unavailable_reason
                .unwrap()
                .contains("dispersion owns")
        );
        assert!(
            recipe
                .with_construction_kind(
                    0,
                    toniator_domain::PatternRecipeConstructionKind::Connections
                )
                .is_err()
        );
        assert!(
            recipe
                .with_construction_kind(0, toniator_domain::PatternRecipeConstructionKind::Regions)
                .is_err()
        );
        let descriptors = document.property_descriptors();
        assert!(!descriptors.iter().any(|d| matches!(
            d.field,
            toniator_domain::PropertyFieldId::RandomLloydEnabled
                | toniator_domain::PropertyFieldId::RandomDensityModulation
                | toniator_domain::PropertyFieldId::RandomExclusion
        )));
        assert_eq!(
            descriptors
                .iter()
                .any(|d| d.field == toniator_domain::PropertyFieldId::RandomSeed),
            layout == DispersionLayout::ClusterScreen
        );
    }
}
