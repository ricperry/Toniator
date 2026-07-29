//! Thin Weighted Voronoi adapter over neutral placement and geometry services.

use crate::CancellationToken;
use crate::artwork_pipeline::{OutputChannelId, ResolvedChannelField, ResolvedChannelFields};
use crate::model::{
    WeightedVoronoiArrangementPolicy, WeightedVoronoiDensityPolarity, WeightedVoronoiPlacementMode,
    WeightedVoronoiSettings,
};
use crate::pattern::{
    AffineTransform, ArtboardSpace, CanonicalBlendMode, CanonicalColor, CanonicalLayer,
    CanonicalLayerId, CanonicalPatternOutput, CanonicalPoint, CompositePatternOutput, FillRule,
    FilledRegion, GeometryPolarity, PolygonRing, RegionId, RegionPatternOutput, RingWinding,
};
use crate::site_distribution::{
    ArrangementPolicy, DistributionField, DistributionFingerprint, DistributionIdentity,
    DistributionLimits, DistributionMode, DistributionPolarity, DistributionRequest,
    DistributionRequestMetadata, DomainBounds, generate_site_distribution_cancellable,
};
use crate::voronoi_geometry::{
    GeometryLimits, build_voronoi_diagram_cancellable, inset_clipped_cell_for_response,
};
use anyhow::{Result, ensure};

/// Long edge of the bounded resolved source field used by this adapter.
pub const WEIGHTED_VORONOI_MAX_FIELD_EDGE: u32 = 256;

/// Per-channel cache provenance. This is metadata only; the adapter owns no
/// global cache and callers decide whether a completed result can be reused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeightedVoronoiCacheMetadata {
    pub channel: OutputChannelId,
    pub source_generation: u64,
    pub resolved_field_generation: u64,
    pub distribution_fingerprint: DistributionFingerprint,
    pub geometry_fingerprint: u64,
    pub view_key: &'static str,
}

/// Makes each positive cell and its subtractive boundary region inspectably
/// related without adding renderer-specific state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WeightedVoronoiCellRelationship {
    pub channel: OutputChannelId,
    pub site_index: usize,
    pub positive_region: RegionId,
    pub subtractive_boundary_region: RegionId,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WeightedVoronoiGeneratedOutput {
    pub output: CanonicalPatternOutput,
    pub cache_metadata: Vec<WeightedVoronoiCacheMetadata>,
    pub relationships: Vec<WeightedVoronoiCellRelationship>,
}

pub fn weighted_voronoi_field_dimensions(domain: DomainBounds) -> Result<(u32, u32)> {
    domain.validate()?;
    let longest = domain.width.max(domain.height);
    let scale = (f64::from(WEIGHTED_VORONOI_MAX_FIELD_EDGE) / f64::from(longest)).min(1.0);
    Ok((
        (f64::from(domain.width) * scale).round().max(1.0) as u32,
        (f64::from(domain.height) * scale).round().max(1.0) as u32,
    ))
}

/// Converts validated semantic fields into canonical positive and subtractive
/// regions. Uniform placement still resolves fields
/// for interior response, but never reads them for distribution.
pub fn generate_weighted_voronoi_cancellable(
    domain: DomainBounds,
    settings: &WeightedVoronoiSettings,
    fields: &ResolvedChannelFields,
    token: &CancellationToken,
) -> Result<WeightedVoronoiGeneratedOutput> {
    token.checkpoint()?;
    domain.validate()?;
    settings.validate()?;
    ensure!(
        fields.bounds.width > 0
            && fields.bounds.height > 0
            && fields.bounds.width <= WEIGHTED_VORONOI_MAX_FIELD_EDGE
            && fields.bounds.height <= WEIGHTED_VORONOI_MAX_FIELD_EDGE,
        "Weighted Voronoi resolved fields exceed the bounded field grid"
    );
    let artboard = ArtboardSpace {
        width: domain.width,
        height: domain.height,
    };
    let mut layers = Vec::new();
    let mut regions = Vec::new();
    let mut metadata = Vec::new();
    let mut relationships = Vec::new();

    for (channel_index, field) in fields.fields().iter().enumerate() {
        token.checkpoint()?;
        let channel_settings = settings.channel_settings(field.channel)?;
        if !channel_settings.enabled {
            continue;
        }
        validate_field(field, fields)?;
        let distribution = generate_distribution(domain, field, channel_settings, token)?;
        let diagram = build_voronoi_diagram_cancellable(
            domain,
            &distribution.points,
            GeometryLimits {
                max_sites: DistributionLimits::default().max_sites,
            },
            token,
        )?;
        let layer_id = CanonicalLayerId(layers.len() as u32 + 1);
        layers.push(channel_layer(field.channel, layer_id, layers.len() as u32));
        let geometry_fingerprint = fingerprint_geometry(
            &diagram
                .cells
                .iter()
                .flat_map(|cell| &cell.vertices)
                .copied()
                .collect::<Vec<_>>(),
        );
        metadata.push(WeightedVoronoiCacheMetadata {
            channel: field.channel,
            source_generation: fields.generation,
            resolved_field_generation: field.generation,
            distribution_fingerprint: distribution.fingerprint,
            geometry_fingerprint,
            // Preview/PNG/SVG must not influence source, field, distribution,
            // geometry, or channel cache keys; they consume this same output.
            view_key: "canonical-output-v1",
        });

        for cell in &diagram.cells {
            if cell.site_index % 64 == 0 {
                token.checkpoint()?;
            }
            let response = response_at(field, distribution.points[cell.site_index], domain);
            let scale = if channel_settings.response_strength == 0.0 {
                1.0
            } else {
                channel_settings.minimum_cell_scale
                    + (1.0 - channel_settings.minimum_cell_scale)
                        * response.powf(channel_settings.response_strength)
            };
            if scale <= 1.0e-6 {
                continue;
            }
            let inset = inset_clipped_cell_for_response(
                domain,
                cell,
                distribution.points[cell.site_index],
                scale,
                channel_settings.boundary_gap,
            )?;
            let region_offset = (channel_index as u32)
                .saturating_mul(DistributionLimits::default().max_sites as u32)
                .saturating_add(cell.site_index as u32)
                .saturating_add(1);
            // Canonical region IDs deliberately occupy disjoint ranges. The
            // relationship record below, not numeric proximity, owns pairing.
            let positive = RegionId(region_offset);
            let subtractive = RegionId(1_000_000u32.saturating_add(region_offset));
            regions.push(FilledRegion {
                id: positive,
                layer_id,
                order: cell.site_index as u32,
                rings: vec![ring(&cell.vertices)],
                fill_rule: FillRule::NonZero,
                polarity: GeometryPolarity::Positive,
                transform: AffineTransform::IDENTITY,
            });
            regions.push(FilledRegion {
                id: subtractive,
                layer_id,
                order: cell.site_index as u32,
                rings: vec![ring(&cell.vertices), ring(&inset)],
                fill_rule: FillRule::EvenOdd,
                polarity: GeometryPolarity::Subtractive,
                transform: AffineTransform::IDENTITY,
            });
            relationships.push(WeightedVoronoiCellRelationship {
                channel: field.channel,
                site_index: cell.site_index,
                positive_region: positive,
                subtractive_boundary_region: subtractive,
            });
        }
    }
    let output = CanonicalPatternOutput::Composite(CompositePatternOutput {
        artboard,
        regions: Some(RegionPatternOutput {
            artboard,
            layers,
            regions,
        }),
        network: None,
    });
    output.validate().map_err(anyhow::Error::new)?;
    Ok(WeightedVoronoiGeneratedOutput {
        output,
        cache_metadata: metadata,
        relationships,
    })
}

fn generate_distribution(
    domain: DomainBounds,
    field: &ResolvedChannelField,
    settings: &crate::model::WeightedVoronoiChannelSettings,
    token: &CancellationToken,
) -> Result<crate::site_distribution::SiteDistribution> {
    let channel = field.channel;
    let values = field
        .values()
        .iter()
        .enumerate()
        .map(|(index, _)| field.value_at(index))
        .collect();
    let field = DistributionField::new(field.bounds.width, field.bounds.height, values)?;
    let metadata = DistributionRequestMetadata {
        seed: settings.seed,
        identity: DistributionIdentity(channel_identity(channel)),
        arrangement: match settings.arrangement {
            WeightedVoronoiArrangementPolicy::Shared => ArrangementPolicy::Shared,
            WeightedVoronoiArrangementPolicy::Independent => ArrangementPolicy::Independent,
        },
        mode: match settings.placement {
            WeightedVoronoiPlacementMode::Uniform => DistributionMode::Uniform,
            WeightedVoronoiPlacementMode::SourceWeighted => DistributionMode::SourceWeighted,
        },
        polarity: match settings.density_polarity {
            WeightedVoronoiDensityPolarity::DarkerMoreDense => {
                DistributionPolarity::HigherValuesMoreDense
            }
            WeightedVoronoiDensityPolarity::LighterMoreDense => {
                DistributionPolarity::LowerValuesMoreDense
            }
        },
        strength_milli: (settings.density_strength * 1_000.0).round() as u32,
    };
    generate_site_distribution_cancellable(
        DistributionRequest {
            domain,
            count: settings.cell_count as usize,
            metadata,
            field: Some(&field),
            limits: DistributionLimits::default(),
        },
        token,
    )
}

fn validate_field(field: &ResolvedChannelField, fields: &ResolvedChannelFields) -> Result<()> {
    ensure!(
        field.bounds == fields.bounds,
        "Weighted Voronoi fields have inconsistent bounds"
    );
    ensure!(
        field.values().len()
            == (field.bounds.width as usize).saturating_mul(field.bounds.height as usize),
        "Weighted Voronoi field dimensions do not match values"
    );
    ensure!(
        field
            .values()
            .iter()
            .chain(field.coverage())
            .all(|value| value.is_finite() && (0.0..=1.0).contains(value)),
        "Weighted Voronoi fields must be finite normalized values"
    );
    Ok(())
}

fn response_at(
    field: &ResolvedChannelField,
    point: crate::site_distribution::OrderedPoint,
    domain: DomainBounds,
) -> f64 {
    let x = ((point.x / f64::from(domain.width)) * f64::from(field.bounds.width))
        .floor()
        .clamp(0.0, f64::from(field.bounds.width - 1)) as usize;
    let y = ((point.y / f64::from(domain.height)) * f64::from(field.bounds.height))
        .floor()
        .clamp(0.0, f64::from(field.bounds.height - 1)) as usize;
    field
        .value_at(y * field.bounds.width as usize + x)
        .clamp(0.0, 1.0)
}

fn ring(points: &[crate::site_distribution::OrderedPoint]) -> PolygonRing {
    PolygonRing {
        vertices: points
            .iter()
            .map(|point| CanonicalPoint {
                x: point.x as f32,
                y: point.y as f32,
            })
            .collect(),
        winding: RingWinding::Clockwise,
    }
}

fn channel_layer(channel: OutputChannelId, id: CanonicalLayerId, order: u32) -> CanonicalLayer {
    let (color, blend_mode) = match channel {
        OutputChannelId::CmykCyan => (
            CanonicalColor {
                red: 0,
                green: 174,
                blue: 239,
            },
            CanonicalBlendMode::Multiply,
        ),
        OutputChannelId::CmykMagenta => (
            CanonicalColor {
                red: 236,
                green: 0,
                blue: 140,
            },
            CanonicalBlendMode::Multiply,
        ),
        OutputChannelId::CmykYellow => (
            CanonicalColor {
                red: 255,
                green: 242,
                blue: 0,
            },
            CanonicalBlendMode::Multiply,
        ),
        OutputChannelId::CmykBlack => (
            CanonicalColor {
                red: 17,
                green: 17,
                blue: 17,
            },
            CanonicalBlendMode::Multiply,
        ),
        OutputChannelId::RgbRed => (
            CanonicalColor {
                red: 255,
                green: 0,
                blue: 0,
            },
            CanonicalBlendMode::Screen,
        ),
        OutputChannelId::RgbGreen => (
            CanonicalColor {
                red: 0,
                green: 255,
                blue: 0,
            },
            CanonicalBlendMode::Screen,
        ),
        OutputChannelId::RgbBlue => (
            CanonicalColor {
                red: 0,
                green: 0,
                blue: 255,
            },
            CanonicalBlendMode::Screen,
        ),
    };
    CanonicalLayer {
        id,
        channel: Some(channel),
        label: format!("Weighted Voronoi {}", channel.stable_id()),
        order,
        color,
        opacity: 1.0,
        blend_mode,
    }
}

fn channel_identity(channel: OutputChannelId) -> u64 {
    channel
        .stable_id()
        .bytes()
        .fold(0xcbf2_9ce4_8422_2325u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x1000_0000_01b3)
        })
}

fn fingerprint_geometry(points: &[crate::site_distribution::OrderedPoint]) -> u64 {
    points.iter().fold(0xcbf2_9ce4_8422_2325u64, |hash, point| {
        hash ^ point.x.to_bits().wrapping_mul(31) ^ point.y.to_bits().rotate_left(17)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artwork_pipeline::{
        ArtworkPipelineSettings, AutomaticSeparationStrategy, ChannelAssignment, OutputModel,
        PreparedSource, SourceAlphaPolicy, resolve_channel_fields,
    };
    use crate::model::{
        Document, DocumentEditor, OutputMode, SourceArtwork, WeightedVoronoiArrangementPolicy,
        WeightedVoronoiPlacementMode,
    };
    use crate::render::{
        generate_document_pattern_output, render_canonical_pattern_output_cancellable,
    };
    use crate::{
        canonical_pattern_png_bytes, canonical_pattern_svg_bytes, load_document,
        save_document_atomic,
    };
    use image::{ImageFormat, Rgba, RgbaImage};
    use std::io::Cursor;
    use std::sync::Arc;

    fn rgb_pipeline() -> ArtworkPipelineSettings {
        ArtworkPipelineSettings {
            output_model: OutputModel::RgbScreen,
            assignment: ChannelAssignment::automatic(
                AutomaticSeparationStrategy::RgbDirectEncodedComponentsV1,
            ),
            alpha_policy: SourceAlphaPolicy::Ignore,
            active_channel: Some(OutputChannelId::RgbRed),
            ..ArtworkPipelineSettings::default()
        }
    }

    fn fields(source: &RgbaImage) -> ResolvedChannelFields {
        let prepared = PreparedSource::from_rgba_image(source, 41);
        resolve_channel_fields(
            &prepared,
            &rgb_pipeline(),
            32,
            16,
            41,
            &OutputChannelId::RGB,
        )
        .unwrap()
    }

    fn settings(count: u32) -> WeightedVoronoiSettings {
        let mut settings = WeightedVoronoiSettings::default();
        for channel in OutputChannelId::CMYK
            .into_iter()
            .chain(OutputChannelId::RGB)
        {
            let channel_settings = settings.channel_settings_mut(channel).unwrap();
            channel_settings.enabled = channel.belongs_to(OutputModel::RgbScreen);
            channel_settings.cell_count = count;
            channel_settings.seed = 77;
            channel_settings.boundary_gap = 0.5;
        }
        settings
    }

    #[test]
    fn semantic_fields_and_weighted_channels_remain_distinct() {
        let source = RgbaImage::from_fn(64, 32, |x, _| {
            if x < 32 {
                Rgba([255, 0, 0, 255])
            } else {
                Rgba([0, 0, 255, 255])
            }
        });
        let fields = fields(&source);
        assert_ne!(
            fields.field(OutputChannelId::RgbRed).unwrap().values(),
            fields.field(OutputChannelId::RgbBlue).unwrap().values()
        );
        let generated = generate_weighted_voronoi_cancellable(
            DomainBounds {
                width: 64,
                height: 32,
            },
            &settings(24),
            &fields,
            &CancellationToken::new(),
        )
        .unwrap();
        let red = generated
            .cache_metadata
            .iter()
            .find(|entry| entry.channel == OutputChannelId::RgbRed)
            .unwrap();
        let blue = generated
            .cache_metadata
            .iter()
            .find(|entry| entry.channel == OutputChannelId::RgbBlue)
            .unwrap();
        assert_ne!(red.geometry_fingerprint, blue.geometry_fingerprint);
        let CanonicalPatternOutput::Composite(composite) = &generated.output else {
            panic!("Weighted Voronoi must use canonical composite output");
        };
        let regions = composite.regions.as_ref().unwrap();
        for relationship in &generated.relationships {
            assert_ne!(
                relationship.subtractive_boundary_region.0,
                relationship.positive_region.0
            );
            assert!(
                regions
                    .regions
                    .iter()
                    .any(|region| region.id == relationship.positive_region)
            );
            assert!(
                regions
                    .regions
                    .iter()
                    .any(|region| region.id == relationship.subtractive_boundary_region)
            );
        }
    }

    #[test]
    fn uniform_is_source_independent_while_shared_and_independent_are_explicit() {
        let left = RgbaImage::from_pixel(64, 32, Rgba([255, 0, 0, 255]));
        let right = RgbaImage::from_pixel(64, 32, Rgba([0, 0, 255, 255]));
        let first = fields(&left);
        let second = fields(&right);
        let mut configured = settings(24);
        let red = configured
            .channel_settings_mut(OutputChannelId::RgbRed)
            .unwrap();
        red.placement = WeightedVoronoiPlacementMode::Uniform;
        let first_distribution = generate_distribution(
            DomainBounds {
                width: 64,
                height: 32,
            },
            first.field(OutputChannelId::RgbRed).unwrap(),
            red,
            &CancellationToken::new(),
        )
        .unwrap();
        let second_distribution = generate_distribution(
            DomainBounds {
                width: 64,
                height: 32,
            },
            second.field(OutputChannelId::RgbRed).unwrap(),
            red,
            &CancellationToken::new(),
        )
        .unwrap();
        assert_eq!(first_distribution.points, second_distribution.points);
        let green = configured
            .channel_settings_mut(OutputChannelId::RgbGreen)
            .unwrap();
        green.placement = WeightedVoronoiPlacementMode::Uniform;
        green.arrangement = WeightedVoronoiArrangementPolicy::Shared;
        let shared = generate_distribution(
            DomainBounds {
                width: 64,
                height: 32,
            },
            first.field(OutputChannelId::RgbGreen).unwrap(),
            green,
            &CancellationToken::new(),
        )
        .unwrap();
        assert_eq!(first_distribution.points, shared.points);
        green.arrangement = WeightedVoronoiArrangementPolicy::Independent;
        let independent = generate_distribution(
            DomainBounds {
                width: 64,
                height: 32,
            },
            first.field(OutputChannelId::RgbGreen).unwrap(),
            green,
            &CancellationToken::new(),
        )
        .unwrap();
        assert_ne!(first_distribution.points, independent.points);
    }

    #[test]
    fn geometry_only_controls_preserve_distribution_fingerprints() {
        let source = RgbaImage::from_fn(64, 32, |x, _| {
            if x < 32 {
                Rgba([255, 0, 0, 255])
            } else {
                Rgba([0, 0, 255, 255])
            }
        });
        let fields = fields(&source);
        let domain = DomainBounds {
            width: 64,
            height: 32,
        };
        let first = generate_weighted_voronoi_cancellable(
            domain,
            &settings(24),
            &fields,
            &CancellationToken::new(),
        )
        .unwrap();
        let mut geometry_only = settings(24);
        for channel in OutputChannelId::RGB {
            geometry_only
                .channel_settings_mut(channel)
                .unwrap()
                .boundary_gap = 8.0;
        }
        let second = generate_weighted_voronoi_cancellable(
            domain,
            &geometry_only,
            &fields,
            &CancellationToken::new(),
        )
        .unwrap();
        for channel in OutputChannelId::RGB {
            let first_metadata = first
                .cache_metadata
                .iter()
                .find(|entry| entry.channel == channel)
                .unwrap();
            let second_metadata = second
                .cache_metadata
                .iter()
                .find(|entry| entry.channel == channel)
                .unwrap();
            assert_eq!(
                first_metadata.distribution_fingerprint, second_metadata.distribution_fingerprint,
                "boundary-gap-only changes must not alter site distribution"
            );
        }
    }

    #[test]
    fn canonical_preview_png_svg_share_cells_without_a_perimeter_border() {
        let source = SourceArtwork {
            name: "weighted.png".into(),
            media_type: "image/png".into(),
            bytes: Arc::from({
                let image = RgbaImage::from_fn(40, 24, |x, _| Rgba([(x * 6) as u8, 0, 255, 255]));
                let mut bytes = Cursor::new(Vec::new());
                image.write_to(&mut bytes, ImageFormat::Png).unwrap();
                bytes.into_inner()
            }),
        };
        let mut editor = DocumentEditor::new(Document::new(source));
        assert!(editor.set_output_mode(OutputMode::RgbScreen));
        assert!(editor.select_pattern(crate::pattern::PatternId::WEIGHTED_VORONOI_V1));
        let mut configured = settings(16);
        configured
            .channel_settings_mut(OutputChannelId::RgbRed)
            .unwrap()
            .enabled = true;
        assert!(editor.set_artwork_pipeline(rgb_pipeline()));
        assert!(editor.set_weighted_voronoi_settings(configured));
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("weighted.toniator");
        save_document_atomic(&path, editor.document()).unwrap();
        let reopened = load_document(&path).unwrap();
        assert_eq!(
            reopened.pattern_state.selected_pattern_id(),
            Some(crate::pattern::PatternId::WEIGHTED_VORONOI_V1)
        );
        let output = generate_document_pattern_output(editor.document()).unwrap();
        let preview = render_canonical_pattern_output_cancellable(
            &output,
            40,
            24,
            false,
            None,
            &CancellationToken::new(),
        )
        .unwrap();
        let png = canonical_pattern_png_bytes(&output, 40, 24, false, None).unwrap();
        let decoded = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!(preview, decoded);
        let svg = String::from_utf8(canonical_pattern_svg_bytes(&output, "weighted.png").unwrap())
            .unwrap();
        assert!(svg.contains("fill-rule=\"evenodd\""));
        assert!(!svg.contains("stroke-width=\"0.5\""));
    }
}
