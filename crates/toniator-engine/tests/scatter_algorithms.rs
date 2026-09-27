use std::collections::BTreeSet;

use toniator_domain::{
    CanvasSpec, CoveragePolicy, Document, DocumentCommand, DocumentHistory, DocumentSession,
    MarkGeometryResponse, PatternDefinitionRecipe, PatternGeometryResponse,
    PatternOutputSettingsRecipe, PatternStructureRecipe, RandomSiteCharacter, RandomSiteRefinement,
    SiteDensityModulation, SiteExclusionPolicy, SiteUseFilterRecipe, SourceReference,
    SourceReferenceId,
};
use toniator_engine::{EvaluationRequest, ResolvedSource, SourceFormatHint, evaluate};

/// Reproduces the full-size SVG Poisson refinement case without exhausting optional overflow work.
/// Native PNG/SVG witnesses are written only when the evidence directory is explicitly supplied.
///
/// # Panics
/// Panics if a normal Feature size change fails, loses population, or cannot write requested evidence.
#[test]
fn poisson_feature_size_increases_full_canvas_population() {
    let source_id = SourceReferenceId::new("feature-size-vector").unwrap();
    let source = ResolvedSource::new(
        source_id.clone(),
        std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/vector-sample.svg"),
        )
        .unwrap(),
        SourceFormatHint::Svg,
    )
    .unwrap();
    let mut counts = Vec::new();
    for size in [2.0, 0.5] {
        let canvas = CanvasSpec {
            width: 900.0,
            height: 620.0,
        };
        let mut density = toniator_domain::DensityMetric2D::default_for_canvas(&canvas).unwrap();
        density.density /= size;
        let document =
            Document::new_default_document(canvas, SourceReference::Assigned(source_id.clone()))
                .unwrap();
        let mut history = DocumentHistory::new(DocumentSession::new(document).unwrap());
        let base = history.document().pattern_settings().clone();
        let base_definition = history
            .document()
            .pattern_definition_bundles()
            .iter()
            .find(|bundle| bundle.definition.id == base.definition_id)
            .unwrap()
            .definition
            .clone();
        history
            .apply(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
                base,
                base_definition,
                recipe: recipe(
                    RandomSiteCharacter::Even {
                        minimum_center_distance: 8.0,
                    },
                    RandomSiteRefinement::default(),
                ),
            })
            .unwrap();
        for channel in [
            toniator_domain::ChannelId(1),
            toniator_domain::ChannelId(2),
            toniator_domain::ChannelId(3),
        ] {
            let command = history
                .document()
                .set_channel_density_for_effective(channel, density.clone())
                .unwrap();
            history.apply(&command).unwrap();
        }
        let result = evaluate(EvaluationRequest::new(
            history.session().document_evaluation_snapshot(),
            source.clone(),
        ))
        .unwrap();
        counts.push(
            result
                .family_output(toniator_domain::ChannelId(1))
                .unwrap()
                .site_set()
                .len(),
        );
        if let Some(directory) = std::env::var_os("TONIATOR_SCATTER_EVIDENCE") {
            let directory = std::path::PathBuf::from(directory);
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(
                directory.join(format!("poisson-size-{size}.png")),
                toniator_render::encode_png(result.raster()).unwrap(),
            )
            .unwrap();
            std::fs::write(
                directory.join(format!("poisson-size-{size}.svg")),
                toniator_render::write_svg(result.scene()),
            )
            .unwrap();
        }
    }
    assert!(
        counts[1] > counts[0] * 8,
        "coarse/fine populations: {counts:?}"
    );
}

/// Builds one single-output scatter recipe with current production work limits.
fn recipe(
    character: RandomSiteCharacter,
    refinement: RandomSiteRefinement,
) -> PatternDefinitionRecipe {
    PatternDefinitionRecipe {
        structure: PatternStructureRecipe::RandomSites {
            name: "Scatter algorithm witness".into(),
            coverage: CoveragePolicy {
                guard_steps: 1,
                additional_margin: 0.0,
            },
            character,
            seed: 19,
            density_modulation: SiteDensityModulation::Uniform,
            exclusion: SiteExclusionPolicy::None,
            maximum_attempts: 16_000_000,
            maximum_neighbor_checks: 16_000_000,
            refinement,
        },
        output_settings: vec![PatternOutputSettingsRecipe {
            source_filter: SiteUseFilterRecipe::All,
            response: PatternGeometryResponse::Marks(MarkGeometryResponse {
                minimum_fill: 0.0,
                maximum_fill: 1.0,
            }),
        }],
    }
}

/// Evaluates every fresh Scatter choice plus ordinary and weighted Lloyd against both assets.
/// Writes native PNG/SVG witnesses only when `TONIATOR_SCATTER_EVIDENCE` supplies a directory.
///
/// # Panics
/// Panics if a default choice reaches late evaluation failure, emits no sites, or aliases another
/// algorithm/refinement identity.
#[test]
fn fresh_scatter_choices_and_relaxation_evaluate_for_both_sources() {
    for (asset, hint) in [
        ("raster-sample.png", SourceFormatHint::Png),
        ("vector-sample.svg", SourceFormatHint::Svg),
    ] {
        let source_id = SourceReferenceId::new(format!("scatter-{asset}")).unwrap();
        let source = ResolvedSource::new(
            source_id.clone(),
            std::fs::read(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../assets")
                    .join(asset),
            )
            .unwrap(),
            hint,
        )
        .unwrap();
        let cases = [
            (
                RandomSiteCharacter::RawUniform,
                RandomSiteRefinement::default(),
            ),
            (
                RandomSiteCharacter::Even {
                    minimum_center_distance: 8.0,
                },
                RandomSiteRefinement::default(),
            ),
            (
                RandomSiteCharacter::Even {
                    minimum_center_distance: 1.0,
                },
                RandomSiteRefinement::default(),
            ),
            (
                RandomSiteCharacter::Stratified { jitter: 0.8 },
                RandomSiteRefinement::default(),
            ),
            (
                RandomSiteCharacter::Clustered {
                    cluster_density: 5.0,
                    cluster_spread: 16.0,
                    cluster_strength: 0.85,
                },
                RandomSiteRefinement::default(),
            ),
            (
                RandomSiteCharacter::RawUniform,
                RandomSiteRefinement {
                    enabled: true,
                    density_weighted: false,
                    iterations: 4,
                },
            ),
            (
                RandomSiteCharacter::RawUniform,
                RandomSiteRefinement {
                    enabled: true,
                    density_weighted: true,
                    iterations: 4,
                },
            ),
        ];
        let mut fingerprints = BTreeSet::new();
        for (index, (character, refinement)) in cases.into_iter().enumerate() {
            let document = Document::new_default_document(
                CanvasSpec {
                    width: 96.0,
                    height: 64.0,
                },
                SourceReference::Assigned(source_id.clone()),
            )
            .unwrap();
            let mut history = DocumentHistory::new(DocumentSession::new(document).unwrap());
            let base = history.document().pattern_settings().clone();
            let base_definition = history
                .document()
                .pattern_definition_bundles()
                .iter()
                .find(|bundle| bundle.definition.id == base.definition_id)
                .unwrap()
                .definition
                .clone();
            history
                .apply(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
                    base,
                    base_definition,
                    recipe: recipe(character, refinement),
                })
                .unwrap();
            let result = evaluate(EvaluationRequest::new(
                history.session().document_evaluation_snapshot(),
                source.clone(),
            ))
            .unwrap();
            let family = result.family_output(toniator_domain::ChannelId(1)).unwrap();
            assert!(!family.site_set().sites().is_empty());
            assert!(fingerprints.insert(family.family_fingerprint().to_owned()));
            if let Some(directory) = std::env::var_os("TONIATOR_SCATTER_EVIDENCE") {
                let directory = std::path::PathBuf::from(directory);
                std::fs::create_dir_all(&directory).unwrap();
                std::fs::write(
                    directory.join(format!("{asset}-{index}.png")),
                    toniator_render::encode_png(result.raster()).unwrap(),
                )
                .unwrap();
                std::fs::write(
                    directory.join(format!("{asset}-{index}.svg")),
                    toniator_render::write_svg(result.scene()),
                )
                .unwrap();
            }
        }
    }
}
