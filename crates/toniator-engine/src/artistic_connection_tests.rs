//! Focused frozen-topology singleton/probe/mixed-consumer regressions.
use super::*;
use toniator_domain::{
    ConnectedGeometryResponse, ConnectionAdjacencyIntent, ConnectionProgram,
    ConnectionSelectionIntent, EffectivePatternOutputSettings, PathStrokeStyle,
    PatternGeometryResponse, PatternMechanismId, PatternOutputLayerId, SiteUseFilter,
    SourceMapping, SourceMappingComponent,
};
use toniator_patterns::{
    FamilySite, FamilySiteId, FamilySiteProvenance, FamilySiteSet, GeometryResponseInput,
    NominalCellBasis, OutputCapability, OutputCapabilityPayload, SiteScope,
    StructuralProductCapability, Vector2,
};

/// Creates a selected pair and one unreachable singleton from finite original family vertices.
/// # Panics
/// Panics if finite family/adjacency/selection setup violates current authority.
fn frozen() -> (
    ConnectionPathSet,
    OutputCapability,
    EffectivePatternOutputSettings,
    SourceField,
    CanvasSpec,
) {
    let sites = FamilySiteSet::new(
        "mixed-test".into(),
        PatternMechanismId(9),
        [(4., 4.), (6., 4.), (14., 14.)]
            .iter()
            .enumerate()
            .map(|(ordinal, p)| FamilySite {
                id: FamilySiteId {
                    mechanism_id: PatternMechanismId(9),
                    ordinal,
                },
                position: Point2::new(p.0, p.1),
                nominal_cell_basis: NominalCellBasis::new(
                    Vector2::new(2., 0.),
                    Vector2::new(0., 2.),
                )
                .unwrap(),
                scope: SiteScope::Canvas,
                provenance: FamilySiteProvenance::Random {
                    candidate_ordinal: ordinal,
                    accepted_ordinal: ordinal,
                    exclusion_neighbor_ordinal: None,
                },
            })
            .collect(),
    )
    .unwrap();
    let program = ConnectionProgram::NearestLinks {
        adjacency: ConnectionAdjacencyIntent {
            maximum_degree: 1,
            maximum_distance: 3.,
        },
    };
    let graph = toniator_patterns::build_site_adjacency_cancellable(
        &sites,
        toniator_patterns::SiteAdjacencyPolicy::MutualNearest {
            maximum_degree: 1,
            maximum_distance: 3.,
        },
        toniator_patterns::SiteAdjacencyLimits::default(),
        &|| false,
    )
    .unwrap();
    let paths = toniator_patterns::build_connection_paths_with_selection_cancellable(
        PatternOutputLayerId(11),
        &graph,
        &program,
        ConnectionSelectionIntent::default(),
        toniator_patterns::ConnectionPathLimits::default(),
        &|| false,
    )
    .unwrap();
    let capability = OutputCapability {
        layer_id: PatternOutputLayerId(11),
        source_filter: SiteUseFilter::All,
        consumes: StructuralProductCapability::RandomSites,
        payload: OutputCapabilityPayload::ConnectionPaths {
            site_mechanism_id: PatternMechanismId(9),
            program,
            selection: Default::default(),
            style: PathStrokeStyle::default(),
        },
    };
    let setting = EffectivePatternOutputSettings {
        output_layer_id: PatternOutputLayerId(11),
        response: PatternGeometryResponse::Connected(ConnectedGeometryResponse {
            minimum_thickness: 0.,
            maximum_thickness: 1.,
            bias: 0.,
        }),
    };
    (
        paths,
        capability,
        setting,
        SourceField::from_straight_rgba8(1, 1, vec![128, 64, 32, 128]).unwrap(),
        CanvasSpec {
            width: 20.,
            height: 20.,
        },
    )
}
/// Checks that the actual prepared calibration probe includes positive singleton area and matches final geometry.
/// # Panics
/// Panics if dots vanish in probe occupancy, final replay differs or structural zero usage is lost.
#[test]
fn prepared_connection_probe_and_final_include_singleton_area_and_zero_usage() {
    let (paths, capability, setting, source, canvas) = frozen();
    assert_eq!(paths.isolated_points.len(), 1);
    let usage = SiteUsageSet::new(
        PatternMechanismId(9),
        paths
            .selected_edges
            .iter()
            .flat_map(|e| [e.first, e.second])
            .chain(paths.isolated_points.iter().map(|p| p.id))
            .collect(),
    )
    .unwrap();
    let prepared = coverage_output::PreparedCoverageOutput::Connection {
        paths,
        style: PathStrokeStyle::default(),
        usage: usage.clone(),
    };
    let mapping = SourceMapping::canonical(SourceMappingComponent::Red);
    let probe = prepared
        .probe(
            &capability,
            &setting,
            &source,
            &canvas,
            mapping,
            EvaluationLimits::default(),
            0.25,
            &|| false,
        )
        .unwrap();
    let (final_geometry, final_usage) = prepared
        .realize(
            &capability,
            &setting,
            &source,
            &canvas,
            mapping,
            &ChannelPaint::Solid(toniator_domain::ColorValue {
                red: 1.,
                green: 0.,
                blue: 0.,
                alpha: 1.,
            }),
            EvaluationLimits::default(),
            GeometryResponseInput::RawProbe(0.5),
            &|| false,
        )
        .unwrap();
    assert_eq!(final_geometry.geometry().as_ref(), &probe);
    assert_eq!(final_usage, usage);
    let GeometryOutput::CanonicalConnections(value) = &probe else {
        panic!("mixed output")
    };
    assert_eq!(value.isolated_points.len(), 1);
    assert!(!value.strokes.is_empty());
    let total = toniator_render::measure_geometry_vector_occupancy(
        &probe,
        &canvas,
        toniator_render::VectorOccupancyPrecision::Tight,
        RasterizationLimits::default(),
        &|| false,
    )
    .unwrap()
    .fraction;
    let stroke_only = toniator_render::measure_geometry_vector_occupancy(
        &GeometryOutput::CanonicalStrokes(value.strokes.clone()),
        &canvas,
        toniator_render::VectorOccupancyPrecision::Tight,
        RasterizationLimits::default(),
        &|| false,
    )
    .unwrap()
    .fraction;
    assert!(total > stroke_only + 0.001);
    let zero = SourceField::from_straight_rgba8(1, 1, vec![0, 0, 0, 0]).unwrap();
    let (zero_geometry, zero_usage) = prepared
        .realize(
            &capability,
            &setting,
            &zero,
            &canvas,
            mapping,
            &ChannelPaint::Solid(toniator_domain::ColorValue {
                red: 1.,
                green: 0.,
                blue: 0.,
                alpha: 1.,
            }),
            EvaluationLimits::default(),
            GeometryResponseInput::Identity,
            &|| false,
        )
        .unwrap();
    assert_eq!(zero_usage, usage);
    let GeometryOutput::CanonicalConnections(zero) = zero_geometry.geometry().as_ref() else {
        panic!("mixed")
    };
    assert!(matches!(
        zero.isolated_points[0],
        toniator_patterns::CanonicalMark::Circle { radius: 0.0, .. }
    ));
}
/// Shares source alpha/cutoff, emitted nominal basis and bias-independent circles across owned/borrowed routes.
/// # Panics
/// Panics if response authority, singleton identity or owned/borrowed geometry differs.
#[test]
fn singleton_response_basis_bias_and_owned_replay_share_authority() {
    let (paths, _, _, source, canvas) = frozen();
    let mapping = SourceMapping::canonical(SourceMappingComponent::Red);
    let response = toniator_patterns::StrokeResponse {
        minimum_thickness: 0.,
        maximum_thickness: 1.,
        bias: 0.,
    };
    let a = toniator_patterns::realize_connection_canonical_strokes_cancellable(
        &paths,
        &source,
        &canvas,
        mapping,
        response,
        PathStrokeStyle::default(),
        10000,
        10000,
        &|| false,
    )
    .unwrap();
    let owned = toniator_patterns::realize_owned_connection_canonical_strokes_cancellable(
        paths.clone(),
        &source,
        &canvas,
        mapping,
        response,
        PathStrokeStyle::default(),
        10000,
        10000,
        &|| false,
    )
    .unwrap();
    assert_eq!(a, owned);
    let ink = source
        .sample_mapping_response(paths.isolated_points[0].position, &canvas, mapping)
        .unwrap();
    let expected = ink * paths.isolated_points[0].nominal_cell_basis.diameter() / 2.;
    let toniator_patterns::CanonicalMark::Circle { radius, .. } = a.isolated_points[0] else {
        panic!("circle")
    };
    assert!((radius - expected).abs() < 1e-12);
    let b = toniator_patterns::realize_connection_canonical_strokes_cancellable(
        &paths,
        &source,
        &canvas,
        mapping,
        toniator_patterns::StrokeResponse {
            bias: 0.75,
            ..response
        },
        PathStrokeStyle::default(),
        10000,
        10000,
        &|| false,
    )
    .unwrap();
    assert_eq!(a.isolated_points, b.isolated_points);
    let cutoff = SourceMapping {
        tone: toniator_domain::SourceTone {
            cutoff: 0.9,
            ..mapping.tone
        },
        ..mapping
    };
    let c = toniator_patterns::realize_connection_canonical_strokes_cancellable(
        &paths,
        &source,
        &canvas,
        cutoff,
        response,
        PathStrokeStyle::default(),
        10000,
        10000,
        &|| false,
    )
    .unwrap();
    assert!(matches!(
        c.isolated_points[0],
        toniator_patterns::CanonicalMark::Circle { radius: 0.0, .. }
    ));
}

/// Checks native mixed raster, banded PNG, editable SVG, identity and solid-only paint validation.
/// # Panics
/// Panics if either batch disappears, native consumers disagree or mixed paint restrictions weaken.
#[test]
fn mixed_connections_native_consumers_and_identity_include_both_batches() {
    let (paths, _, _, source, canvas) = frozen();
    let realized = toniator_patterns::realize_connection_canonical_strokes_cancellable(
        &paths,
        &source,
        &canvas,
        SourceMapping::canonical(SourceMappingComponent::Red),
        toniator_patterns::StrokeResponse {
            minimum_thickness: 1.,
            maximum_thickness: 1.,
            bias: 0.,
        },
        PathStrokeStyle::default(),
        10000,
        10000,
        &|| false,
    )
    .unwrap();
    let geometry = GeometryOutput::CanonicalConnections(toniator_patterns::CanonicalConnections {
        strokes: realized.strokes,
        isolated_points: realized.isolated_points,
    });
    let scene = |geometry: GeometryOutput, paints| {
        let layer = toniator_render::RenderLayer::new_outputs(
            ChannelId(1),
            true,
            toniator_domain::ColorValue {
                red: 1.,
                green: 0.,
                blue: 0.,
                alpha: 1.,
            },
            0.7,
            vec![toniator_render::RenderOutputLayer::new(
                PatternOutputLayerId(11),
                geometry,
                paints,
            )],
        )?;
        toniator_render::RenderScene::new(
            canvas.clone(),
            "mixed".into(),
            "mixed".into(),
            vec![layer],
        )
    };
    let mixed = scene(geometry.clone(), None).unwrap();
    let raster = toniator_render::rasterize(&mixed, RasterBackground::Transparent).unwrap();
    let mut png = Vec::new();
    toniator_render::write_png_streaming(
        &mixed,
        toniator_render::StreamingRasterTarget::new(20, 20).unwrap(),
        toniator_render::PngStreamOptions {
            maximum_band_rows: 3,
            ..Default::default()
        },
        RasterizationLimits::default(),
        &mut png,
        &|| false,
        &|_, _| {},
    )
    .unwrap();
    let decoded = image::load_from_memory(&png).unwrap().to_rgba8();
    assert_eq!(decoded.as_raw(), raster.pixels());
    let svg = toniator_render::write_svg(&mixed).unwrap();
    assert!(svg.contains("stroke-0"));
    assert!(
        svg.contains("<circle") && svg.contains("cx=\"14\""),
        "{svg}"
    );
    let GeometryOutput::CanonicalConnections(value) = geometry else {
        unreachable!()
    };
    let mut changed = value.clone();
    if let toniator_patterns::CanonicalMark::Circle { radius, .. } = &mut changed.isolated_points[0]
    {
        *radius *= 0.5;
    }
    let changed = scene(GeometryOutput::CanonicalConnections(changed), None).unwrap();
    assert_ne!(
        mixed.identity().scene_fingerprint(),
        changed.identity().scene_fingerprint()
    );
    assert!(scene(GeometryOutput::CanonicalConnections(value), Some(vec![])).is_err());
}
