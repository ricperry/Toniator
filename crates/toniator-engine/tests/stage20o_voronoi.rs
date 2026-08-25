use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};

use toniator_domain::{
    CanvasSpec, ChannelId, ColorValue, CoveragePolicy, CurveRepetition, CurveWinding, Document,
    DocumentCommand, DocumentHistory, DocumentSession, ParametricCurve, PatternDefinition,
    PatternDefinitionBundle, PatternDefinitionDraft, PatternDefinitionId, PatternDefinitionRecipe,
    PatternFamily, PatternGeometryResponse, PatternMechanism, PatternMechanismId,
    PatternModulation, PatternOutputLayer, PatternOutputLayerId, PatternOutputSettings,
    PatternStructureRecipe, RandomSiteCharacter, RegionGeometryResponse, RegionSourceIntent,
    SiteDensityModulation, SiteExclusionPolicy, SourceReference, SourceReferenceId, SpiralCurve,
    SpiralShape,
};
use toniator_engine::{
    CacheDisposition, EvaluationCompletion, EvaluationLimits, EvaluationRequest,
    EvaluationScheduler, GeometryOutput, RasterBackground, RenderLayer, RenderScene,
    ResolvedSource, SourceFormatHint, encode_png, evaluate_with_limits, rasterize, write_svg,
};
use toniator_io::{EmbeddedSource, EmbeddedSourceFormat, SourceBundle, load, save};
use toniator_patterns::{
    Bounds, FamilySite, FamilySiteId, FamilySiteProvenance, FamilySiteSet, NominalCellBasis,
    Point2, SiteScope, Vector2, VoronoiRegionLimits, VoronoiRegionRequest,
    build_voronoi_regions_cancellable,
};

/// Builds a current document whose sole selected output is a guarded ordinary Voronoi region set.
fn voronoi_document(width: f64, height: f64, source_id: SourceReferenceId) -> Document {
    let document = Document::new_default_document(
        CanvasSpec { width, height },
        SourceReference::Assigned(source_id),
    )
    .expect("default document");
    let mut history = DocumentHistory::new(DocumentSession::new(document).expect("session"));
    let base = history.document().pattern_settings().clone();
    let base_definition = history
        .document()
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == base.definition_id)
        .map(|bundle| &bundle.definition)
        .expect("selected definition")
        .clone();
    history
        .apply(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base,
            base_definition,
            recipe: PatternDefinitionRecipe::regions(PatternStructureRecipe::StraightGrid(
                PatternDefinitionDraft {
                    name: "Stage 20O ordinary Voronoi".into(),
                    coverage: CoveragePolicy {
                        guard_steps: 32,
                        additional_margin: 0.0,
                    },
                },
            )),
        })
        .expect("region recipe materializes");
    history.document().clone()
}

/// Builds a current random-site dispersion document whose ordinary regions consume its site set.
fn random_voronoi_document(width: f64, height: f64, source_id: SourceReferenceId) -> Document {
    let base = Document::new_default_document(
        CanvasSpec { width, height },
        SourceReference::Assigned(source_id),
    )
    .expect("default document");
    let definition_id = PatternDefinitionId(730);
    let site_id = PatternMechanismId(734);
    let output_id = PatternOutputLayerId(735);
    let mut definition = PatternDefinition::random_sites(
        definition_id,
        "Stage 20O random ordinary Voronoi",
        PatternMechanismId(731),
        PatternMechanismId(732),
        PatternMechanismId(733),
        site_id,
        output_id,
        RandomSiteCharacter::RawUniform,
        73,
        SiteDensityModulation::Uniform,
        SiteExclusionPolicy::None,
        200,
        1_000,
        CoveragePolicy {
            guard_steps: 32,
            additional_margin: 0.0,
        },
    );
    definition.output_layers = vec![PatternOutputLayer::Regions {
        id: output_id,
        source: RegionSourceIntent::VoronoiSites {
            site_mechanism_id: site_id,
        },
    }];
    let mut settings = base.pattern_settings().clone();
    settings.definition_id = definition_id;
    Document::with_source_topology_and_authored_structures(
        toniator_domain::DocumentId(730),
        base.canvas().clone(),
        base.source().clone(),
        vec![PatternDefinitionBundle {
            definition,
            output_settings: vec![PatternOutputSettings {
                output_layer_id: output_id,
                response: PatternGeometryResponse::Regions(RegionGeometryResponse::Full),
            }],
        }],
        settings,
        base.channel_model().expect("model"),
        base.channel_topology().expect("topology").clone(),
        Vec::new(),
    )
    .expect("random region document")
}

/// Builds a typed parametric-site document without inventing an ID-free parametric recipe grammar.
fn parametric_voronoi_document(width: f64, height: f64, source_id: SourceReferenceId) -> Document {
    let base = Document::new_default_document(
        CanvasSpec { width, height },
        SourceReference::Assigned(source_id),
    )
    .expect("default document");
    let definition_id = PatternDefinitionId(740);
    let curve_id = PatternMechanismId(741);
    let site_id = PatternMechanismId(742);
    let output_id = PatternOutputLayerId(743);
    let definition = PatternDefinition {
        id: definition_id,
        name: "Stage 20O parametric regions".into(),
        family: PatternFamily::ParametricCurve {
            curve_mechanism_id: curve_id,
            site_mechanism_id: Some(site_id),
        },
        mechanisms: vec![
            PatternMechanism::ParametricCurveSource {
                id: curve_id,
                curve: ParametricCurve::Spiral(SpiralCurve {
                    shape: SpiralShape::Round,
                    turns: 12.0,
                    radial_spacing: 24.0,
                    phase_degrees: 0.0,
                    winding: CurveWinding::CounterClockwise,
                }),
                repetition: CurveRepetition::Single,
            },
            PatternMechanism::AlongParametricCurveSites {
                id: site_id,
                curve_mechanism_id: curve_id,
                interval: 8.0,
                phase: 0.0,
            },
        ],
        output_layers: vec![PatternOutputLayer::Regions {
            id: output_id,
            source: RegionSourceIntent::VoronoiSites {
                site_mechanism_id: site_id,
            },
        }],
        modulation: PatternModulation,
        coverage: CoveragePolicy {
            guard_steps: 1,
            additional_margin: 0.0,
        },
    };
    let mut settings = base.pattern_settings().clone();
    settings.definition_id = definition_id;
    Document::with_source_topology_and_authored_structures(
        toniator_domain::DocumentId(740),
        base.canvas().clone(),
        base.source().clone(),
        vec![PatternDefinitionBundle {
            definition,
            output_settings: vec![PatternOutputSettings {
                output_layer_id: output_id,
                response: PatternGeometryResponse::Regions(RegionGeometryResponse::Full),
            }],
        }],
        settings,
        base.channel_model().expect("model"),
        base.channel_topology().expect("topology").clone(),
        Vec::new(),
    )
    .expect("parametric document")
}

/// Evaluates one source-backed v5 ordinary-region document through save/load before rendering.
fn render_v5_artwork(
    input_name: &str,
    width: f64,
    height: f64,
    format: EmbeddedSourceFormat,
    hint: SourceFormatHint,
    stem: &str,
    build: fn(f64, f64, SourceReferenceId) -> Document,
) {
    let source_id = SourceReferenceId::new(format!("stage20o-{stem}")).expect("source ID");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let input = root.join("assets").join(input_name);
    let directory = root.join("target/validation/stage20o");
    fs::create_dir_all(&directory).expect("validation directory");
    let source = EmbeddedSource::new(
        source_id.clone(),
        format,
        fs::read(&input).expect("immutable source reads"),
        Some(input_name.into()),
    )
    .expect("embedded source");
    let sources = SourceBundle::new([source]).expect("one source bundle");
    let document = build(width, height, source_id.clone());
    let document_path = directory.join(format!("{stem}.toniator"));
    save(&document_path, &document, &sources).expect("v5 document saves");
    let loaded = load(&document_path).expect("v5 document loads");
    assert_eq!(loaded.versions().document(), 5);
    let source = loaded.sources().get(&source_id).expect("loaded source");
    let session = DocumentSession::new(loaded.document().clone()).expect("loaded session");
    let evaluated = evaluate_with_limits(
        EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(source_id, source.bytes().to_vec(), hint)
                .expect("resolved immutable source"),
        ),
        EvaluationLimits::new(1_048_576).expect("evaluation limits"),
    )
    .expect("ordinary-region document evaluates");
    fs::write(
        directory.join(format!("{stem}.png")),
        encode_png(evaluated.raster()).expect("native PNG encodes"),
    )
    .expect("native PNG writes");
    fs::write(
        directory.join(format!("{stem}.svg")),
        write_svg(evaluated.scene()),
    )
    .expect("raw SVG writes");
}

/// Builds one direct deterministic site authority for supplemental topology witnesses.
///
/// The direct scenes deliberately bypass document and source evaluation only for exact duplicate
/// and off-canvas relevance evidence; source-backed v5 documents remain the primary artifacts.
fn direct_family(points: &[(f64, f64)]) -> FamilySiteSet {
    let mechanism_id = PatternMechanismId(804);
    FamilySiteSet::new(
        "stage20o-direct-family".into(),
        mechanism_id,
        points
            .iter()
            .enumerate()
            .map(|(ordinal, &(x, y))| FamilySite {
                id: FamilySiteId {
                    mechanism_id,
                    ordinal,
                },
                position: Point2::new(x, y),
                nominal_cell_basis: NominalCellBasis::new(
                    Vector2::new(1.0, 0.0),
                    Vector2::new(0.0, 1.0),
                )
                .expect("unit direct-site basis"),
                scope: SiteScope::Canvas,
                provenance: FamilySiteProvenance::Random {
                    candidate_ordinal: ordinal,
                    accepted_ordinal: ordinal,
                    exclusion_neighbor_ordinal: None,
                },
            })
            .collect(),
    )
    .expect("direct family is valid")
}

/// Renders one direct canonical Region witness without asking a renderer to create topology or
/// use canvas clipping as a region boundary.
///
/// The builder receives guard-inclusive sites and emits only finite canonical cells. The renderer
/// then applies its ordinary final canvas clip while serializing the same immutable scene to PNG
/// and SVG.
fn render_direct_region_artwork(stem: &str, points: &[(f64, f64)]) {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("target/validation/stage20o");
    fs::create_dir_all(&directory).expect("validation directory");
    let canvas = CanvasSpec {
        width: 180.0,
        height: 120.0,
    };
    let family = direct_family(points);
    let (regions, diagnostics) = build_voronoi_regions_cancellable(
        &family,
        VoronoiRegionRequest {
            output_layer_id: PatternOutputLayerId(805),
            canvas: Bounds::new(
                Point2::new(0.0, 0.0),
                Point2::new(canvas.width, canvas.height),
            )
            .expect("finite direct canvas"),
        },
        VoronoiRegionLimits::default(),
        || false,
    )
    .expect("direct guard sites cover the canvas");
    assert!(
        diagnostics.regions > 0,
        "direct Region witness retains a finite cell"
    );
    let scene = RenderScene::new(
        canvas,
        "stage20o-direct-family".into(),
        regions.fingerprint().to_owned(),
        vec![
            RenderLayer::new_for_output(
                ChannelId(1),
                true,
                ColorValue {
                    red: 0.08,
                    green: 0.45,
                    blue: 0.9,
                    alpha: 1.0,
                },
                1.0,
                PatternOutputLayerId(805),
                GeometryOutput::CanonicalRegions(regions),
            )
            .expect("direct Region render layer"),
        ],
    )
    .expect("direct Region scene");
    fs::write(
        directory.join(format!("{stem}.png")),
        encode_png(
            &rasterize(&scene, RasterBackground::OpaqueWhite).expect("direct Region raster"),
        )
        .expect("direct Region PNG"),
    )
    .expect("direct Region PNG writes");
    fs::write(directory.join(format!("{stem}.svg")), write_svg(&scene))
        .expect("direct Region SVG writes");
}

/// Waits for one scheduler completion without depending on worker scheduling details.
fn wait_for_completion(scheduler: &EvaluationScheduler) -> EvaluationCompletion {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(completion) = scheduler.try_receive_latest().expect("scheduler receive") {
            return completion;
        }
        assert!(
            Instant::now() < deadline,
            "ordinary Voronoi evaluation timed out"
        );
        std::thread::yield_now();
    }
}

/// Proves a saved current document realizes fixed regions without source-derived region paint.
#[test]
fn saved_v5_region_document_evaluates() {
    let source_id = SourceReferenceId::new("stage20o-evaluation").expect("source ID");
    let document = voronoi_document(180.0, 120.0, source_id.clone());
    let session = DocumentSession::new(document).expect("session");
    let source =
        fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/raster-sample.png"))
            .expect("immutable source reads");
    let output = evaluate_with_limits(
        EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(source_id, source, SourceFormatHint::Png).expect("source"),
        ),
        EvaluationLimits::new(1_048_576).expect("limits"),
    )
    .expect("regions evaluate");
    assert!(!output.scene().layers().is_empty());
}

/// Proves typed AlongParametricCurveSites realizes canonical Regions and participates in output caching.
#[test]
fn parametric_site_regions_evaluate_and_cache() {
    let source_id = SourceReferenceId::new("stage20o-parametric").expect("source");
    let session =
        DocumentSession::new(parametric_voronoi_document(180.0, 120.0, source_id.clone()))
            .expect("session");
    let bytes =
        fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/raster-sample.png"))
            .expect("source bytes");
    let scheduler =
        EvaluationScheduler::new_with_limits(EvaluationLimits::new(1_048_576).expect("limits"))
            .expect("scheduler");
    let request = || {
        EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(source_id.clone(), bytes.clone(), SourceFormatHint::Png)
                .expect("source"),
        )
    };
    let _ = scheduler.submit(request()).expect("first submit");
    let completion = wait_for_completion(&scheduler);
    assert!(completion.result().is_some());
    assert!(
        completion
            .cache_diagnostics()
            .expect("diagnostics")
            .channels[0]
            .outputs[0]
            .voronoi
            .is_some()
    );
    scheduler.shutdown().expect("shutdown");
}

/// Proves per-output region diagnostics and configured Voronoi limits replay from immutable cache units.
#[test]
fn region_cache_replays_geometry_diagnostics() {
    let source_id = SourceReferenceId::new("stage20o-cache").expect("source ID");
    let session =
        DocumentSession::new(voronoi_document(180.0, 120.0, source_id.clone())).expect("session");
    let bytes =
        fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/raster-sample.png"))
            .expect("immutable source reads");
    let limits = EvaluationLimits::new(1_048_576)
        .expect("limits")
        .with_voronoi_region_limits(
            VoronoiRegionLimits::new(1_048_576, 4_194_304, 1_048_576, 8_388_608, 67_108_864)
                .expect("Voronoi limits"),
        )
        .expect("configured limits");
    let scheduler = EvaluationScheduler::new_with_limits(limits).expect("scheduler");
    let request = || {
        EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(source_id.clone(), bytes.clone(), SourceFormatHint::Png)
                .expect("source"),
        )
    };
    let first_ticket = scheduler.submit(request()).expect("first submit");
    let first = wait_for_completion(&scheduler);
    assert_eq!(first.ticket(), first_ticket);
    let first_output = &first.cache_diagnostics().expect("diagnostics").channels[0].outputs[0];
    assert_eq!(first_output.realization, CacheDisposition::Miss);
    assert!(first_output.voronoi.is_some());
    assert!(
        scheduler
            .accept_completion(&first, &session)
            .expect("accepts first")
    );
    let second_ticket = scheduler.submit(request()).expect("second submit");
    let second = wait_for_completion(&scheduler);
    assert_eq!(second.ticket(), second_ticket);
    let second_output = &second.cache_diagnostics().expect("diagnostics").channels[0].outputs[0];
    assert_eq!(second_output.realization, CacheDisposition::Hit);
    assert_eq!(second_output.voronoi, first_output.voronoi);
    scheduler.shutdown().expect("scheduler shutdown");
}

/// Generates intrinsic v5 source-backed ordinary-region evidence for both immutable project inputs.
#[test]
#[ignore = "writes Stage 20O validation artifacts"]
fn generate_intrinsic_v5_region_artifacts() {
    render_v5_artwork(
        "raster-sample.png",
        1024.0,
        1024.0,
        EmbeddedSourceFormat::Png,
        SourceFormatHint::Png,
        "v5-raster-sample-1024",
        voronoi_document,
    );
    render_v5_artwork(
        "vector-sample.svg",
        900.0,
        620.0,
        EmbeddedSourceFormat::Svg,
        SourceFormatHint::Svg,
        "v5-vector-random-sample-900x620",
        random_voronoi_document,
    );
    render_v5_artwork(
        "vector-sample.svg",
        360.0,
        240.0,
        EmbeddedSourceFormat::Svg,
        SourceFormatHint::Svg,
        "v5-vector-parametric-sites-360x240",
        parametric_voronoi_document,
    );
    let mut duplicate_guard = [-90.0, 0.0, 90.0, 180.0, 270.0]
        .into_iter()
        .flat_map(|x| {
            [-80.0, 0.0, 60.0, 120.0, 200.0]
                .into_iter()
                .map(move |y| (x, y))
        })
        .collect::<Vec<_>>();
    duplicate_guard.push((90.0, 60.0));
    render_direct_region_artwork("direct-exact-duplicate-coowners-180x120", &duplicate_guard);
    render_direct_region_artwork(
        "direct-offcanvas-guard-coverage-180x120",
        &[
            (-180.0, -120.0),
            (0.0, -120.0),
            (180.0, -120.0),
            (360.0, -120.0),
            (-180.0, 0.0),
            (0.0, 0.0),
            (180.0, 0.0),
            (360.0, 0.0),
            (-180.0, 120.0),
            (0.0, 120.0),
            (180.0, 120.0),
            (360.0, 120.0),
            (-180.0, 240.0),
            (0.0, 240.0),
            (180.0, 240.0),
            (360.0, 240.0),
        ],
    );
}
