//! Typed current regression fixtures, independent from superseded persisted Pattern schemas.

use toniator_domain::{
    CoveragePolicy, DEFAULT_RANDOM_NOMINAL_SPACING, PatternDefinitionRecipe,
    PatternStructureRecipe, PresetMetadata, PresetRecord, RandomSiteCharacter,
    RandomSiteRefinement, SiteDensityModulation, SiteExclusionPolicy,
};

/// Reconstructs the reported clustered, artwork-weighted Lloyd marks using current domain intent.
/// The fixture carries no invented calibration; structural previews bypass measurement and Apply
/// prepares the exact current binding through the production guarded worker.
pub(crate) fn clustered_weighted_lloyd() -> PresetRecord {
    PresetRecord {
        metadata: PresetMetadata {
            id: "user-00000000-0000-4000-88d7-26a841cca68a".into(),
            name: "Clustered weighted Lloyd regression".into(),
            category: "Scatter".into(),
            description: "Clustered marks with artwork-weighted Lloyd refinement.".into(),
            thumbnail: None,
        },
        recipe: PatternDefinitionRecipe::marks(PatternStructureRecipe::RandomSites {
            name: "Poisson disk circles".into(),
            coverage: CoveragePolicy {
                guard_steps: 2,
                additional_margin: 0.0,
            },
            character: RandomSiteCharacter::Clustered {
                cluster_density: 5.0,
                cluster_spread: 16.0,
                cluster_strength: 0.85,
            },
            nominal_spacing: Some(DEFAULT_RANDOM_NOMINAL_SPACING),
            seed: 19,
            density_modulation: SiteDensityModulation::Uniform,
            exclusion: SiteExclusionPolicy::None,
            maximum_attempts: 16_000_000,
            maximum_neighbor_checks: 16_000_000,
            refinement: RandomSiteRefinement {
                enabled: true,
                density_weighted: true,
                iterations: 4,
            },
        }),
        saved_calibration: None,
    }
}
