//! Explicit current-schema fixture generation for native TON-018 optical inspection.

use std::{fs, path::PathBuf, sync::Arc};

use toniator_domain::{
    CanvasSpec, ChannelTopology, ChannelTopologyTemplate, CoveragePolicy, DensityMetric2D,
    Document, HalftoneChannelModel, MarkGeometryResponse, PatternDefinition,
    PatternDefinitionBundle, PatternDefinitionId, PatternGeometryResponse, PatternMechanismId,
    PatternOutputLayerId, PatternOutputSettings, RandomSiteCharacter, SiteDensityModulation,
    SiteExclusionPolicy, SourceReference, SourceReferenceId,
};
use toniator_io::{EmbeddedSource, EmbeddedSourceFormat, SourceBundle, load, save};

/// Writes current project-schema 14 projects from immutable raster and vector artwork.
///
/// This opt-in graphics witness keeps the original bytes and canvas dimensions, gives each
/// modeled channel an independent optical rotation, and leaves native renderer backgrounds
/// to the explicit CLI render calls. All outputs remain under this stage's target directory.
///
/// # Panics
/// Fails if current document, rotation, source-container, or round-trip authority rejects a case.
#[test]
#[ignore = "writes native optical-review fixtures under target/validation"]
fn write_uniform_native_projects() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let destination =
        root.join("target/validation/ton018-uniform-density-production-20261009/native");
    fs::create_dir_all(&destination).expect("stage target directory creates");
    for (name, format, dimensions) in [
        ("raster", EmbeddedSourceFormat::Png, (1024.0, 1024.0)),
        ("vector", EmbeddedSourceFormat::Svg, (900.0, 620.0)),
    ] {
        let source_path = root.join(if name == "raster" {
            "assets/raster-sample.png"
        } else {
            "assets/vector-sample.svg"
        });
        let source_bytes = fs::read(&source_path).expect("immutable source reads");
        for (model_name, model, rotations) in [
            ("rgb", HalftoneChannelModel::Rgb, &[37.5, 60.0, 82.5][..]),
            (
                "cmyk",
                HalftoneChannelModel::Cmyk,
                &[37.5, 60.0, 82.5, 15.0][..],
            ),
            (
                "source-alpha",
                HalftoneChannelModel::SourceColorAlpha,
                &[37.5][..],
            ),
        ] {
            for density in [40.0, 160.0] {
                if density == 160.0 && model == HalftoneChannelModel::SourceColorAlpha {
                    continue;
                }
                let suffix = if density == 160.0 { "-density160" } else { "" };
                let project =
                    destination.join(format!("{name}-{model_name}{suffix}-native.toniator"));
                if density == 40.0 && project.exists() {
                    let preserved =
                        load(&project).expect("previously inspected native project remains valid");
                    assert_eq!(preserved.document().canvas().width, dimensions.0);
                    continue;
                }
                let source_id = SourceReferenceId::new(format!("ton018-{name}-{model_name}"))
                    .expect("stage source ID validates");
                let canvas = CanvasSpec {
                    width: dimensions.0,
                    height: dimensions.1,
                };
                let base = Document::new_default_document(
                    canvas.clone(),
                    SourceReference::Assigned(source_id.clone()),
                )
                .expect("neutral document validates");
                let output_id = PatternOutputLayerId(1);
                let definition = PatternDefinition::random_sites(
                    PatternDefinitionId(1),
                    "Weighted uniform circular marks",
                    PatternMechanismId(11),
                    PatternMechanismId(12),
                    PatternMechanismId(13),
                    PatternMechanismId(14),
                    output_id,
                    RandomSiteCharacter::Stratified { jitter: 0.7 },
                    907,
                    SiteDensityModulation::ArtworkWeighted,
                    SiteExclusionPolicy::None,
                    30_000,
                    30_000,
                    CoveragePolicy {
                        guard_steps: 1,
                        additional_margin: 0.0,
                    },
                );
                let template = ChannelTopologyTemplate {
                    pattern_instance: base.channel_topology().expect("base topology").channels()[0]
                        .pattern_instance
                        .clone(),
                };
                let topology =
                    ChannelTopology::canonical(model, template).expect("canonical roles validate");
                let mut settings = base.pattern_settings().clone();
                settings.density = DensityMetric2D {
                    density,
                    aspect: 1.0,
                };
                let mut document = Document::with_source_and_topology(
                    base.id(),
                    canvas,
                    base.source().clone(),
                    vec![PatternDefinitionBundle {
                        definition,
                        output_settings: vec![PatternOutputSettings {
                            output_layer_id: output_id,
                            response: PatternGeometryResponse::Marks(MarkGeometryResponse {
                                minimum_fill: 0.0,
                                maximum_fill: 1.0,
                            }),
                        }],
                    }],
                    settings,
                    model,
                    topology,
                )
                .expect("weighted mark project validates");
                let channels = document
                    .channel_topology()
                    .expect("modeled topology")
                    .channels()
                    .to_vec();
                for (channel, rotation) in channels.iter().zip(rotations) {
                    let command = document
                        .set_channel_pattern_rotation_for_effective(channel.id, *rotation)
                        .expect("rotation command validates");
                    document = document
                        .apply_command(&command)
                        .expect("rotation applies")
                        .0;
                    assert_eq!(
                        document
                            .effective_mark_density(channel.id, output_id)
                            .unwrap()
                            .mode,
                        toniator_domain::MarkDensityMode::Uniform
                    );
                }
                let source = EmbeddedSource::new(
                    source_id,
                    format,
                    Arc::<[u8]>::from(source_bytes.clone()),
                    Some(format!("{name}-sample")),
                )
                .expect("source embeds");
                save(
                    &project,
                    &document,
                    &SourceBundle::new([source]).expect("bundle validates"),
                )
                .expect("current project saves");
                let restored = load(&project).expect("current project loads");
                assert_eq!(restored.document(), &document);
                eprintln!(
                    "{}: {} channels, {}x{} canvas",
                    project.display(),
                    channels.len(),
                    dimensions.0,
                    dimensions.1
                );
            }
        }
    }
}
