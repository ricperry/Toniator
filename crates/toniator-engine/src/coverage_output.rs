//! Frozen output producers shared by automatic geometry probes and final realization.

use std::collections::BTreeMap;

use toniator_domain::{
    ChannelPaint, EffectiveChannelPatternInstance, EffectivePatternOutputSettings,
    PatternDefinition, PatternGeometryResponse, PatternMechanismId, PatternOutputRealization,
    RegionSourceIntent,
};
use toniator_patterns::{
    CanonicalRegionSet, ConnectionPathSet, CurvePath, GeometryResponseInput, MazeProgramResult,
    RegionReference, RegionTreatment, RegionTreatmentRequest, SiteUsageSet, StrokeResponse,
    TypedFamilyOutput,
};
use toniator_render::GeometryOutput;

use crate::{
    Bounds, CanvasSpec, DocumentRealization, EvaluationError, EvaluationLimits, GridInspectRequest,
    Point2, RegionProducerCacheDiagnostics, SourceField, cache_stroke_realization,
    completed_region_diagnostics, shared_sampled_paints, voronoi_region_site_usage,
};

/// Original source-derived topology retained intact across constant probes and the final sample.
pub(crate) enum PreparedCoverageOutput {
    Motif {
        rows: Vec<CurvePath>,
        motif_segment_count: usize,
        structure_id: toniator_domain::AuthoredStructureId,
        style: toniator_domain::PathStrokeStyle,
        mirror_alternate_rows: bool,
        alternate_row_phase: Option<f64>,
        family_fingerprint: String,
    },
    Maze {
        maze: Box<MazeProgramResult>,
        style: toniator_domain::PathStrokeStyle,
        usage: SiteUsageSet,
    },
    Connection {
        paths: ConnectionPathSet,
        style: toniator_domain::PathStrokeStyle,
        usage: SiteUsageSet,
    },
    Regions {
        untreated: CanonicalRegionSet,
        references: Vec<RegionReference>,
        ordered_references: Vec<Point2>,
        producer: RegionProducerCacheDiagnostics,
        site_mechanism: Option<PatternMechanismId>,
    },
}

/// Builds a dependent output producer once from the original filtered family and source layout.
///
/// Mark and guide geometry already live in the frozen typed family and return `None`. Maze,
/// connection, and region producers retain their complete topology and off-canvas guards.
///
/// # Errors
/// Returns the existing producer, resource-limit, identity, or cancellation diagnostic.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare(
    document: &toniator_domain::Document,
    definition: &PatternDefinition,
    effective: &EffectiveChannelPatternInstance,
    family: &TypedFamilyOutput,
    plan: &toniator_patterns::PatternPipelinePlan,
    capability: &toniator_patterns::OutputCapability,
    limits: EvaluationLimits,
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<Option<PreparedCoverageOutput>, EvaluationError> {
    if let Some(region_source) = capability.regions() {
        let canvas = Bounds::new(
            Point2::new(0.0, 0.0),
            Point2::new(document.canvas().width, document.canvas().height),
        )
        .ok_or(EvaluationError::new(
            "region.voronoi.geometry.canvas",
            "region producer requires finite canvas bounds",
        ))?;
        let (untreated, references, producer, site_mechanism) = match region_source {
            RegionSourceIntent::VoronoiSites { site_mechanism_id } => {
                if definition.coverage.guard_steps == 0
                    || family.site_set().product_mechanism_id() != *site_mechanism_id
                {
                    return Err(EvaluationError::new(
                        "region.voronoi.identity.site_mechanism",
                        "region output requires guard coverage and its own family mechanism",
                    ));
                }
                let (regions, diagnostics) = toniator_patterns::build_voronoi_regions_cancellable(
                    family.site_set(),
                    toniator_patterns::VoronoiRegionRequest {
                        output_layer_id: capability.layer_id,
                        canvas,
                    },
                    limits.voronoi_region_limits(),
                    is_cancelled,
                )
                .map_err(|error| EvaluationError::new(error.path(), error.message()))?;
                let references: Vec<RegionReference> =
                    toniator_patterns::voronoi_region_references(family.site_set(), &regions)
                        .map_err(|error| EvaluationError::new(error.path(), error.message()))?
                        .into_iter()
                        .map(|(region_id, point)| RegionReference { region_id, point })
                        .collect();
                (
                    regions,
                    references,
                    RegionProducerCacheDiagnostics::Voronoi(diagnostics),
                    Some(*site_mechanism_id),
                )
            }
            RegionSourceIntent::GuideFaces {
                guide_mechanism_id,
                dimensions,
            } => {
                if definition.coverage.guard_steps == 0 {
                    return Err(EvaluationError::new(
                        "region.guide_faces.coverage.guard_steps",
                        "guide-face regions require guard coverage",
                    ));
                }
                let paths = family
                    .structural_path_set()
                    .cloned()
                    .ok_or(EvaluationError::new(
                        "region.guide_faces.identity.paths",
                        "guide-face output requires structural paths",
                    ))?;
                let result = toniator_patterns::build_guide_faces_cancellable(
                    toniator_patterns::GuideFaceRequest {
                        output_layer_id: capability.layer_id,
                        guide_mechanism_id: *guide_mechanism_id,
                        dimensions: dimensions.clone(),
                        paths,
                        canvas,
                    },
                    limits.guide_face_limits(),
                    is_cancelled,
                )
                .map_err(|error| EvaluationError::new(error.path(), error.message()))?;
                let references: Vec<RegionReference> = result
                    .centroids
                    .into_iter()
                    .map(|(region_id, point)| RegionReference { region_id, point })
                    .collect();
                (
                    result.regions,
                    references,
                    RegionProducerCacheDiagnostics::GuideFaces,
                    None,
                )
            }
        };
        let table = references
            .iter()
            .map(|reference| (reference.region_id.clone(), reference.point))
            .collect::<BTreeMap<_, _>>();
        let ordered_references = untreated
            .regions()
            .iter()
            .map(|region| {
                table.get(&region.id).copied().ok_or(EvaluationError::new(
                    "region.reference.identity",
                    "producer reference is missing for an untreated region",
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(Some(PreparedCoverageOutput::Regions {
            untreated,
            references,
            ordered_references,
            producer,
            site_mechanism,
        }));
    }
    if let Some((_site_mechanism_id, program, style)) = capability.maze_walls() {
        let request = GridInspectRequest {
            canvas: document.canvas().clone(),
            density: effective.resolved_density,
            rotation_degrees: effective.pattern_rotation_degrees,
            translation_x: effective.translation_x,
            translation_y: effective.translation_y,
            guard_steps: crate::curve_motif_phase_guard_steps(
                definition.coverage.guard_steps,
                &plan.ordered_outputs,
            )?,
            support_radius: crate::required_support_radius_for_outputs(
                document.canvas(),
                effective,
                definition,
                &plan.family,
                &plan.ordered_outputs,
            )?,
            max_family_candidates: limits.max_family_candidates(),
        };
        let maze = toniator_patterns::evaluate_typed_maze_walls_from_family_cancellable(
            family,
            &request,
            capability.layer_id,
            program,
            limits.maze_limits(),
            is_cancelled,
        )
        .map_err(EvaluationError::from_pipeline)?;
        let members = maze
            .source_sites
            .iter()
            .map(|site| (site.id, site.source.id))
            .collect::<BTreeMap<_, _>>();
        let usage = SiteUsageSet::new(
            family.site_set().product_mechanism_id(),
            maze.retained_walls
                .iter()
                .flat_map(|wall| [wall.id.first, wall.id.second])
                .filter_map(|vertex| members.get(&vertex).copied())
                .collect(),
        )
        .map_err(EvaluationError::from_pipeline)?;
        return Ok(Some(PreparedCoverageOutput::Maze {
            maze: Box::new(maze),
            style,
            usage,
        }));
    }
    if let Some((_site_mechanism_id, program, style)) = capability.connection_paths() {
        // The frozen producer must consume the same Feature-scaled reach reserved by
        // required_support_radius_for_output; the persisted program stays authored.
        let resolved_program = toniator_patterns::resolve_connection_program_for_feature_size(
            program,
            document.canvas(),
            &effective.resolved_density,
        )
        .map_err(EvaluationError::from_pipeline)?;
        let adjacency = resolved_program.adjacency();
        let support = crate::required_connection_base_support(
            document.canvas(),
            effective,
            definition,
            &plan.family,
        )?;
        let graph = toniator_patterns::build_typed_site_adjacency_cancellable(
            family,
            support,
            toniator_patterns::SiteAdjacencyPolicy::MutualNearest {
                maximum_degree: adjacency.maximum_degree as usize,
                maximum_distance: adjacency.maximum_distance,
            },
            limits.site_adjacency_limits(),
            is_cancelled,
        )
        .map_err(EvaluationError::from_pipeline)?;
        let paths = toniator_patterns::build_connection_paths_cancellable(
            capability.layer_id,
            &graph,
            &resolved_program,
            limits.connection_path_limits(),
            is_cancelled,
        )
        .map_err(|error| EvaluationError::new(error.path(), error.message()))?;
        let usage = SiteUsageSet::new(
            family.site_set().product_mechanism_id(),
            paths
                .selected_edges
                .iter()
                .flat_map(|edge| [edge.first, edge.second])
                .collect(),
        )
        .map_err(EvaluationError::from_pipeline)?;
        return Ok(Some(PreparedCoverageOutput::Connection {
            paths,
            style,
            usage,
        }));
    }
    let authored = definition
        .output_layers
        .iter()
        .find(|output| output.id == capability.layer_id)
        .ok_or(EvaluationError::new(
            "evaluation.output_binding",
            "output is missing",
        ))?;
    if let PatternOutputRealization::CurveMotifPaths {
        structure_id,
        style,
        mirror_alternate_rows,
        alternate_row_phase,
        ..
    } = authored.realization
    {
        let structure = document
            .authored_structure(structure_id)
            .ok_or(EvaluationError::new(
                "evaluation.curve_motif.resource",
                "Curve Motif resource is missing",
            ))?;
        let motif = CurvePath::from_authored_structure(structure).map_err(|_| {
            EvaluationError::new(
                "evaluation.curve_motif.resource",
                "Curve Motif requires a valid open path",
            )
        })?;
        let rows = toniator_patterns::chain_curve_motif_rows_cancellable(
            family.site_set(),
            &motif,
            mirror_alternate_rows,
            alternate_row_phase,
            is_cancelled,
        )
        .map_err(EvaluationError::from_pipeline)?;
        return Ok(Some(PreparedCoverageOutput::Motif {
            rows,
            motif_segment_count: motif.segments().len(),
            structure_id,
            style,
            mirror_alternate_rows,
            alternate_row_phase,
            family_fingerprint: family.family_fingerprint().to_owned(),
        }));
    }
    Ok(None)
}

impl PreparedCoverageOutput {
    /// Realizes a constant completed response on complete frozen topology without source pixels.
    ///
    /// # Errors
    /// Returns probe, treatment, profile, outline, or cancellation diagnostics.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn probe(
        &self,
        capability: &toniator_patterns::OutputCapability,
        setting: &EffectivePatternOutputSettings,
        source: &SourceField,
        canvas: &CanvasSpec,
        mapping: toniator_domain::SourceMapping,
        limits: EvaluationLimits,
        q: f64,
        is_cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<GeometryOutput, EvaluationError> {
        let input = GeometryResponseInput::RawProbe(q * 2.0);
        input.validate().map_err(EvaluationError::from_pipeline)?;
        match self {
            Self::Motif {
                rows,
                motif_segment_count,
                structure_id,
                style,
                mirror_alternate_rows,
                alternate_row_phase,
                family_fingerprint,
            } => {
                let PatternGeometryResponse::Connected(response) = &setting.response else {
                    return Err(EvaluationError::new(
                        "coverage.response.kind",
                        "motif needs connected response",
                    ));
                };
                let output = toniator_patterns::realize_prechained_curve_motif_strokes_with_response_cancellable(
                    family_fingerprint, rows, *motif_segment_count, *structure_id, *style,
                    *mirror_alternate_rows, *alternate_row_phase, source, canvas, mapping,
                    StrokeResponse { minimum_thickness: response.minimum_thickness,
                        maximum_thickness: response.maximum_thickness, bias: response.bias },
                    limits.max_stroke_profile_samples(), limits.max_stroke_outline_segments(),
                    input, is_cancelled,
                ).map_err(EvaluationError::from_pipeline)?;
                Ok(GeometryOutput::CanonicalStrokes(output.strokes))
            }
            Self::Maze { maze, style, .. } => {
                let PatternGeometryResponse::Connected(response) = &setting.response else {
                    return Err(EvaluationError::new(
                        "coverage.response.kind",
                        "maze needs connected response",
                    ));
                };
                let output =
                    toniator_patterns::realize_maze_canonical_strokes_with_response_cancellable(
                        maze,
                        source,
                        canvas,
                        mapping,
                        StrokeResponse {
                            minimum_thickness: response.minimum_thickness,
                            maximum_thickness: response.maximum_thickness,
                            bias: response.bias,
                        },
                        *style,
                        limits.max_stroke_profile_samples(),
                        limits.max_stroke_outline_segments(),
                        input,
                        is_cancelled,
                    )
                    .map_err(EvaluationError::from_pipeline)?;
                Ok(GeometryOutput::CanonicalStrokes(output.strokes))
            }
            Self::Connection { paths, style, .. } => {
                let PatternGeometryResponse::Connected(response) = &setting.response else {
                    return Err(EvaluationError::new(
                        "coverage.response.kind",
                        "connection needs connected response",
                    ));
                };
                let output = toniator_patterns::realize_connection_canonical_strokes_with_response_cancellable(
                    paths, source, canvas, mapping, StrokeResponse {
                        minimum_thickness: response.minimum_thickness,
                        maximum_thickness: response.maximum_thickness, bias: response.bias,
                    }, *style, limits.max_stroke_profile_samples(),
                    limits.max_stroke_outline_segments(), input, is_cancelled,
                ).map_err(EvaluationError::from_pipeline)?;
                Ok(GeometryOutput::CanonicalStrokes(output.strokes))
            }
            Self::Regions {
                untreated,
                ordered_references,
                ..
            } => {
                let PatternGeometryResponse::Regions(response) = &setting.response else {
                    return Err(EvaluationError::new(
                        "coverage.response.kind",
                        "region needs region response",
                    ));
                };
                let fill = input
                    .resolve_raw_size(q, response.minimum_fill, response.maximum_fill)
                    .map_err(EvaluationError::from_pipeline)?;
                let requests = untreated
                    .regions()
                    .iter()
                    .zip(ordered_references)
                    .map(|(region, reference)| RegionTreatmentRequest {
                        base_region_id: region.id.clone(),
                        reference: Some(*reference),
                        treatment: Some(RegionTreatment {
                            algorithm: response.algorithm,
                            fill,
                        }),
                    })
                    .collect::<Vec<_>>();
                let treated = toniator_patterns::treat_region_requests_cancellable(
                    capability.layer_id,
                    untreated,
                    &requests,
                    limits.region_treatment_limits(),
                    is_cancelled,
                )
                .map_err(|error| EvaluationError::new(error.path(), error.message()))?;
                Ok(GeometryOutput::CanonicalRegions(treated.regions))
            }
        }
    }

    /// Completes original source sampling once on the same frozen topology and inverse curve.
    ///
    /// # Errors
    /// Returns normal source, treatment, outline, or cancellation diagnostics atomically.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn realize(
        &self,
        capability: &toniator_patterns::OutputCapability,
        setting: &EffectivePatternOutputSettings,
        source: &SourceField,
        canvas: &CanvasSpec,
        mapping: toniator_domain::SourceMapping,
        paint: &ChannelPaint,
        limits: EvaluationLimits,
        input: GeometryResponseInput<'_>,
        is_cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<(DocumentRealization, SiteUsageSet), EvaluationError> {
        match self {
            Self::Motif {
                rows,
                motif_segment_count,
                structure_id,
                style,
                mirror_alternate_rows,
                alternate_row_phase,
                family_fingerprint,
            } => {
                let PatternGeometryResponse::Connected(response) = &setting.response else {
                    return Err(EvaluationError::new(
                        "coverage.response.kind",
                        "motif needs connected response",
                    ));
                };
                let output = toniator_patterns::realize_prechained_curve_motif_strokes_with_response_cancellable(
                    family_fingerprint, rows, *motif_segment_count, *structure_id, *style,
                    *mirror_alternate_rows, *alternate_row_phase, source, canvas, mapping,
                    StrokeResponse { minimum_thickness: response.minimum_thickness,
                        maximum_thickness: response.maximum_thickness, bias: response.bias },
                    limits.max_stroke_profile_samples(), limits.max_stroke_outline_segments(),
                    input, is_cancelled,
                ).map_err(EvaluationError::from_pipeline)?;
                Ok((
                    cache_stroke_realization(output),
                    SiteUsageSet::empty_non_site(),
                ))
            }
            Self::Maze { maze, style, usage } => {
                let PatternGeometryResponse::Connected(response) = &setting.response else {
                    return Err(EvaluationError::new(
                        "coverage.response.kind",
                        "maze needs connected response",
                    ));
                };
                let output =
                    toniator_patterns::realize_maze_canonical_strokes_with_response_cancellable(
                        maze,
                        source,
                        canvas,
                        mapping,
                        StrokeResponse {
                            minimum_thickness: response.minimum_thickness,
                            maximum_thickness: response.maximum_thickness,
                            bias: response.bias,
                        },
                        *style,
                        limits.max_stroke_profile_samples(),
                        limits.max_stroke_outline_segments(),
                        input,
                        is_cancelled,
                    )
                    .map_err(EvaluationError::from_pipeline)?;
                Ok((cache_stroke_realization(output), usage.clone()))
            }
            Self::Connection {
                paths,
                style,
                usage,
            } => {
                let PatternGeometryResponse::Connected(response) = &setting.response else {
                    return Err(EvaluationError::new(
                        "coverage.response.kind",
                        "connection needs connected response",
                    ));
                };
                let output = toniator_patterns::realize_connection_canonical_strokes_with_response_cancellable(
                    paths, source, canvas, mapping, StrokeResponse {
                        minimum_thickness: response.minimum_thickness,
                        maximum_thickness: response.maximum_thickness, bias: response.bias,
                    }, *style, limits.max_stroke_profile_samples(),
                    limits.max_stroke_outline_segments(), input, is_cancelled,
                ).map_err(EvaluationError::from_pipeline)?;
                Ok((cache_stroke_realization(output), usage.clone()))
            }
            Self::Regions {
                untreated,
                references,
                producer,
                site_mechanism,
                ..
            } => {
                let PatternGeometryResponse::Regions(response) = &setting.response else {
                    return Err(EvaluationError::new(
                        "coverage.response.kind",
                        "region needs region response",
                    ));
                };
                let realized = toniator_patterns::realize_region_output_with_response_cancellable(
                    capability,
                    setting,
                    untreated,
                    references,
                    Some(source),
                    canvas,
                    mapping,
                    paint,
                    limits.region_sampling_limits(),
                    limits.region_treatment_limits(),
                    input,
                    is_cancelled,
                )
                .map_err(|error| EvaluationError::new(error.path(), error.message()))?;
                let usage = match site_mechanism {
                    Some(id) => voronoi_region_site_usage(*id, &realized.regions)?,
                    None => SiteUsageSet::empty_non_site(),
                };
                let fingerprint = if let Some(id) = site_mechanism {
                    format!(
                        "{}:{}:{}:{}",
                        crate::VORONOI_REGION_CONTRACT_ID,
                        capability.layer_id.0,
                        id.0,
                        realized.fingerprint
                    )
                } else {
                    let RegionSourceIntent::GuideFaces {
                        guide_mechanism_id, ..
                    } = capability.regions().ok_or(EvaluationError::new(
                        "coverage.response.kind",
                        "region capability is missing",
                    ))?
                    else {
                        return Err(EvaluationError::new(
                            "coverage.response.kind",
                            "guide-face identity is missing",
                        ));
                    };
                    format!(
                        "{}:{}:{}:{}",
                        crate::GUIDE_FACE_CONTRACT_ID,
                        capability.layer_id.0,
                        guide_mechanism_id.0,
                        realized.fingerprint
                    )
                };
                let result = DocumentRealization::Regions {
                    fingerprint,
                    geometry: std::sync::Arc::new(GeometryOutput::CanonicalRegions(
                        realized.regions,
                    )),
                    paints: shared_sampled_paints(realized.paints),
                    diagnostics: completed_region_diagnostics(
                        response,
                        source,
                        realized.diagnostics,
                        producer.clone(),
                    ),
                };
                Ok((result, usage))
            }
        }
    }
}
