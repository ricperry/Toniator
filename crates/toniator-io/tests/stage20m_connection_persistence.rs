use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use sha2::{Digest, Sha256};
use toniator_domain::{
    CanvasSpec, ConnectedGeometryResponse, ConnectionAdjacencyIntent, ConnectionProgram,
    CoveragePolicy, Document, DocumentId, GeneralizedSiteProduct, GeneralizedSiteProductDraft,
    GridMazeAlgorithm, GridSpanningTreeAlgorithm, GuideDimensionDraft, GuideDimensionId,
    MarkOrientation, MarkOrientationDraft, MazeProgram, PathStrokeStyle, PatternDefinition,
    PatternDefinitionDraft, PatternDefinitionId, PatternDefinitionRecipe, PatternGeometryResponse,
    PatternMechanism, PatternMechanismId, PatternOutputLayer, PatternOutputLayerId, PresetMetadata,
    PresetRecord, RandomSiteCharacter, SiteDensityModulation, SiteExclusionPolicy, SourceReference,
    SourceReferenceId, StraightGuideDimension, StraightGuideRepetition,
};
use toniator_io::{
    EmbeddedSource, EmbeddedSourceFormat, SourceBundle, load, load_preset, save, save_preset,
};

/// Owns one test-private directory and removes it when the focused test ends.
struct TemporaryDirectory(PathBuf);

impl TemporaryDirectory {
    /// Creates a fresh system-temporary directory without touching project validation artifacts.
    fn new(label: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "toniator-stage20m-{label}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("test directory");
        Self(path)
    }

    /// Returns one test-owned path below the directory.
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TemporaryDirectory {
    /// Removes only the directory allocated by this test fixture.
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Builds one intrinsic connection document with selected program and guide dimensionality.
fn connection_document_for(
    source_id: SourceReferenceId,
    width: f64,
    height: f64,
    three_guides: bool,
    program: ConnectionProgram,
) -> Document {
    let base = Document::new_default_document(
        CanvasSpec { width, height },
        SourceReference::Assigned(source_id),
    )
    .expect("base document");
    let guide = PatternMechanismId(801);
    let mut definition = PatternDefinition::generalized_straight_guides(
        PatternDefinitionId(800),
        "connection persistence",
        guide,
        PatternMechanismId(802),
        PatternOutputLayerId(803),
        {
            let mut dimensions = vec![
                StraightGuideDimension {
                    id: GuideDimensionId(804),
                    baseline_angle_degrees: 0.0,
                    phase: 0.0,
                    repetition: StraightGuideRepetition {
                        spacing_multiplier: 1.0,
                    },
                },
                StraightGuideDimension {
                    id: GuideDimensionId(805),
                    baseline_angle_degrees: if three_guides { 60.0 } else { 90.0 },
                    phase: 0.0,
                    repetition: StraightGuideRepetition {
                        spacing_multiplier: 1.0,
                    },
                },
            ];
            if three_guides {
                dimensions.push(StraightGuideDimension {
                    id: GuideDimensionId(806),
                    baseline_angle_degrees: 120.0,
                    // Aligned phases preserve one merged triangular-lattice vertex where all
                    // three authored guide directions concur.
                    phase: 0.0,
                    repetition: StraightGuideRepetition {
                        spacing_multiplier: 1.0,
                    },
                });
            }
            dimensions
        },
        GeneralizedSiteProduct::Intersections {
            dimensions: if three_guides {
                vec![
                    GuideDimensionId(804),
                    GuideDimensionId(805),
                    GuideDimensionId(806),
                ]
            } else {
                vec![GuideDimensionId(804), GuideDimensionId(805)]
            },
            merge_epsilon: 1e-8,
        },
        MarkOrientation::Fixed,
        CoveragePolicy {
            guard_steps: 1,
            additional_margin: 0.0,
        },
    );
    definition.output_layers = vec![PatternOutputLayer::ConnectionPaths {
        id: PatternOutputLayerId(803),
        site_mechanism_id: PatternMechanismId(802),
        program,
        style: PathStrokeStyle::default(),
    }];
    let mut settings = base.pattern_settings().clone();
    settings.definition_id = definition.id;
    settings.density.across_x = 8.0;
    settings.density.across_y = settings.density.across_x * height / width;
    settings.geometry_response = PatternGeometryResponse::Connected(ConnectedGeometryResponse {
        minimum_thickness: 0.1,
        maximum_thickness: 0.25,
    });
    Document::with_source_topology_and_authored_structures(
        DocumentId(800),
        base.canvas().clone(),
        base.source().clone(),
        vec![definition],
        settings,
        base.channel_model().expect("model").to_owned(),
        base.channel_topology().expect("topology").clone(),
        Vec::new(),
    )
    .expect("connection document validates")
}

/// Builds one typed wall-maze document from the same straight-intersection family authority as
/// the connection fixtures, with a 24-across aspect-locked lattice and authored maze intent.
fn maze_document_for(
    source_id: SourceReferenceId,
    width: f64,
    height: f64,
    three_guides: bool,
    seed: u32,
) -> Document {
    let connection = connection_document_for(
        source_id,
        width,
        height,
        three_guides,
        ConnectionProgram::NearestLinks {
            adjacency: ConnectionAdjacencyIntent {
                maximum_degree: if three_guides { 6 } else { 4 },
                maximum_distance: 192.0,
            },
        },
    );
    let mut definition = connection.pattern_definitions()[0].clone();
    definition.output_layers = vec![PatternOutputLayer::MazeWalls {
        id: PatternOutputLayerId(803),
        site_mechanism_id: PatternMechanismId(802),
        program: MazeProgram {
            algorithm: GridMazeAlgorithm::RecursiveBacktracker,
            seed,
        },
        style: PathStrokeStyle::default(),
    }];
    let mut settings = connection.pattern_settings().clone();
    settings.density.across_x = 24.0;
    settings.density.across_y = settings.density.across_x * height / width;
    Document::with_source_topology_and_authored_structures(
        connection.id(),
        connection.canvas().clone(),
        connection.source().clone(),
        vec![definition],
        settings,
        connection
            .channel_model()
            .expect("connection model")
            .to_owned(),
        connection
            .channel_topology()
            .expect("connection topology")
            .clone(),
        connection.authored_structures().to_vec(),
    )
    .expect("maze document validates against its straight intersection family")
}

/// Builds one connected dispersion document without persisting any derived adjacency or paths.
fn dispersion_document_for(source_id: SourceReferenceId, width: f64, height: f64) -> Document {
    let base = Document::new_default_document(
        CanvasSpec { width, height },
        SourceReference::Assigned(source_id),
    )
    .expect("base document");
    let mut definition = PatternDefinition::random_sites(
        PatternDefinitionId(820),
        "connected dispersion",
        PatternMechanismId(821),
        PatternMechanismId(822),
        PatternMechanismId(823),
        PatternMechanismId(824),
        PatternOutputLayerId(825),
        RandomSiteCharacter::Even {
            minimum_center_distance: 24.0,
        },
        31,
        SiteDensityModulation::Uniform,
        SiteExclusionPolicy::None,
        16_384,
        1_048_576,
        CoveragePolicy {
            guard_steps: 1,
            additional_margin: 0.0,
        },
    );
    definition.output_layers = vec![PatternOutputLayer::ConnectionPaths {
        id: PatternOutputLayerId(825),
        site_mechanism_id: PatternMechanismId(824),
        program: ConnectionProgram::RandomLinks {
            adjacency: ConnectionAdjacencyIntent {
                maximum_degree: 3,
                maximum_distance: 96.0,
            },
            minimum_degree: 1,
            seed: 37,
        },
        style: PathStrokeStyle::default(),
    }];
    let mut settings = base.pattern_settings().clone();
    settings.definition_id = definition.id;
    settings.density.across_x = 8.0;
    settings.density.across_y = 8.0;
    settings.geometry_response = PatternGeometryResponse::Connected(ConnectedGeometryResponse {
        minimum_thickness: 0.1,
        maximum_thickness: 0.25,
    });
    Document::with_source_topology_and_authored_structures(
        DocumentId(820),
        base.canvas().clone(),
        base.source().clone(),
        vec![definition],
        settings,
        base.channel_model().expect("model").to_owned(),
        base.channel_topology().expect("topology").clone(),
        Vec::new(),
    )
    .expect("dispersion document validates")
}

/// Builds ordinary metadata for one ID-free preset fixture.
fn preset_metadata(id: &str) -> PresetMetadata {
    PresetMetadata {
        id: id.into(),
        name: format!("Stage 20M {id}"),
        category: "Test".into(),
        description: "Connection-intent preset persistence fixture.".into(),
        thumbnail: None,
    }
}

/// Builds a legal generalized guide recipe with the supplied site-product contract.
fn generalized_recipe(product: GeneralizedSiteProductDraft) -> PatternDefinitionRecipe {
    PatternDefinitionRecipe::GeneralizedStraightGuides {
        name: "Connection guides".into(),
        coverage: CoveragePolicy {
            guard_steps: 1,
            additional_margin: 0.0,
        },
        dimensions: vec![
            GuideDimensionDraft {
                baseline_angle_degrees: 0.0,
                phase: 0.0,
                spacing_multiplier: 1.0,
            },
            GuideDimensionDraft {
                baseline_angle_degrees: 90.0,
                phase: 0.0,
                spacing_multiplier: 1.0,
            },
        ],
        product,
        orientation: MarkOrientationDraft::Fixed,
    }
}

/// Builds a legal random-site recipe for random-link preset coverage.
fn random_recipe() -> PatternDefinitionRecipe {
    PatternDefinitionRecipe::RandomSites {
        name: "Connection dispersion".into(),
        coverage: CoveragePolicy {
            guard_steps: 1,
            additional_margin: 0.0,
        },
        character: RandomSiteCharacter::Even {
            minimum_center_distance: 4.0,
        },
        seed: 31,
        density_modulation: SiteDensityModulation::Uniform,
        exclusion: SiteExclusionPolicy::None,
        maximum_attempts: 128,
        maximum_neighbor_checks: 256,
    }
}

/// Wraps one eligible site-family recipe with fully authored connection intent.
fn connection_preset(
    id: &str,
    definition: PatternDefinitionRecipe,
    program: ConnectionProgram,
) -> PresetRecord {
    PresetRecord {
        metadata: preset_metadata(id),
        recipe: PatternDefinitionRecipe::ConnectionPaths {
            definition: Box::new(definition),
            program,
            style: PathStrokeStyle::default(),
        },
    }
}

/// Wraps one legal two- or three-guide intersection recipe with typed conventional maze intent.
fn maze_preset(id: &str, definition: PatternDefinitionRecipe, seed: u32) -> PresetRecord {
    PresetRecord {
        metadata: preset_metadata(id),
        recipe: PatternDefinitionRecipe::MazeWalls {
            definition: Box::new(definition),
            program: MazeProgram {
                algorithm: GridMazeAlgorithm::RecursiveBacktracker,
                seed,
            },
            style: PathStrokeStyle::default(),
        },
    }
}

/// Proves all current-v4 connection programs retain authored intent and omit derived state.
#[test]
fn connection_v4_round_trips_all_programs_without_derived_state() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../");
    let source_id = SourceReferenceId::new("stage20m-io").expect("id");
    let sources = SourceBundle::new([EmbeddedSource::new(
        source_id.clone(),
        EmbeddedSourceFormat::Png,
        fs::read(root.join("assets/raster-sample.png")).expect("raster asset"),
        Some("raster.png".into()),
    )
    .expect("raster source")])
    .expect("bundle");
    let temporary = TemporaryDirectory::new("document-round-trip");
    let programs = [
        ConnectionProgram::NearestLinks {
            adjacency: ConnectionAdjacencyIntent {
                maximum_degree: 3,
                maximum_distance: 12.0,
            },
        },
        ConnectionProgram::RandomLinks {
            adjacency: ConnectionAdjacencyIntent {
                maximum_degree: 3,
                maximum_distance: 12.0,
            },
            minimum_degree: 1,
            seed: 11,
        },
        ConnectionProgram::GridSpanningTree {
            adjacency: ConnectionAdjacencyIntent {
                maximum_degree: 4,
                maximum_distance: 16.0,
            },
            algorithm: GridSpanningTreeAlgorithm::RandomizedPrim,
            seed: 7,
        },
    ];
    for (index, program) in programs.into_iter().enumerate() {
        let document = connection_document_for(source_id.clone(), 64.0, 48.0, false, program);
        let first = temporary.path(&format!("connection-{index}-a.toniator"));
        let second = temporary.path(&format!("connection-{index}-b.toniator"));
        save(&first, &document, &sources).expect("save");
        save(&second, &document, &sources).expect("repeat save");
        let bytes = fs::read(&first).expect("bytes");
        assert_eq!(bytes, fs::read(&second).expect("repeat bytes"));
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains("connection_paths"));
        for forbidden in [
            "selected_edges",
            "connection_path_set",
            "diagnostics",
            "maximum_inspections",
        ] {
            assert!(
                !text.contains(forbidden),
                "derived {forbidden} must not persist"
            );
        }
        assert_eq!(load(&first).expect("reload").document(), &document);
    }
    let vector_source_id = SourceReferenceId::new("stage20m-io-vector").expect("vector id");
    let vector_sources = SourceBundle::new([EmbeddedSource::new(
        vector_source_id.clone(),
        EmbeddedSourceFormat::Svg,
        fs::read(root.join("assets/vector-sample.svg")).expect("vector asset"),
        Some("vector.svg".into()),
    )
    .expect("vector source")])
    .expect("vector bundle");
    let maze = maze_document_for(vector_source_id, 64.0, 48.0, true, 29);
    let first = temporary.path("maze-a.toniator");
    let second = temporary.path("maze-b.toniator");
    save(&first, &maze, &vector_sources).expect("maze save");
    save(&second, &maze, &vector_sources).expect("repeat maze save");
    let bytes = fs::read(&first).expect("maze bytes");
    assert_eq!(bytes, fs::read(&second).expect("repeat maze bytes"));
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("maze_walls"));
    assert!(text.contains("recursive_backtracker"));
    assert!(text.contains("\"seed\":29"));
    for forbidden in [
        "source_walls",
        "removed_passage_walls",
        "retained_walls",
        "solutions",
        "diagnostics",
    ] {
        assert!(
            !text.contains(forbidden),
            "derived maze {forbidden} must not persist"
        );
    }
    assert_eq!(load(&first).expect("maze reload").document(), &maze);
}

/// Round-trips every legal connection program/family preset without allocating derived graph state.
#[test]
fn preset_v2_round_trips_connection_programs_and_eligible_families() {
    let temporary = TemporaryDirectory::new("preset-round-trip");
    let adjacency = ConnectionAdjacencyIntent {
        maximum_degree: 3,
        maximum_distance: 24.0,
    };
    let presets = [
        connection_preset(
            "nearest-grid",
            PatternDefinitionRecipe::StraightGrid(PatternDefinitionDraft {
                name: "Connection grid".into(),
                coverage: CoveragePolicy {
                    guard_steps: 1,
                    additional_margin: 0.0,
                },
            }),
            ConnectionProgram::NearestLinks { adjacency },
        ),
        connection_preset(
            "random-along-guides",
            generalized_recipe(GeneralizedSiteProductDraft::AlongGuides {
                dimension_indices: vec![0],
                interval_multiplier: 1.0,
                phase: 0.25,
            }),
            ConnectionProgram::RandomLinks {
                adjacency,
                minimum_degree: 1,
                seed: 17,
            },
        ),
        connection_preset(
            "random-sites",
            random_recipe(),
            ConnectionProgram::RandomLinks {
                adjacency,
                minimum_degree: 2,
                seed: 19,
            },
        ),
        connection_preset(
            "tree-intersections",
            generalized_recipe(GeneralizedSiteProductDraft::Intersections {
                dimension_indices: vec![0, 1],
                merge_epsilon: 1e-9,
            }),
            ConnectionProgram::GridSpanningTree {
                adjacency,
                algorithm: GridSpanningTreeAlgorithm::RandomizedPrim,
                seed: 23,
            },
        ),
        maze_preset(
            "maze-intersections",
            generalized_recipe(GeneralizedSiteProductDraft::Intersections {
                dimension_indices: vec![0, 1],
                merge_epsilon: 1e-9,
            }),
            29,
        ),
    ];
    for preset in presets {
        let path = temporary.path(&format!("{}.preset.json", preset.metadata.id));
        save_preset(&path, &preset).expect("preset saves");
        assert_eq!(load_preset(&path).expect("preset reloads"), preset);
        let text = fs::read_to_string(path).expect("preset JSON");
        assert!(
            text.contains("\"kind\": \"connection_paths\"")
                || text.contains("\"kind\": \"maze_walls\"")
        );
        assert!(text.contains("\"style\""));
        for forbidden in [
            "selected_edges",
            "connection_path_set",
            "source_walls",
            "removed_passage_walls",
            "retained_walls",
            "solutions",
            "diagnostics",
            "maximum_inspections",
        ] {
            assert!(
                !text.contains(forbidden),
                "derived {forbidden} must not persist"
            );
        }
    }
}

/// Keeps an existing non-connection preset-v2 byte witness unchanged by the new wrapper variant.
#[test]
fn preset_v2_existing_straight_grid_serialization_is_stable() {
    let temporary = TemporaryDirectory::new("preset-witness");
    let preset = PresetRecord {
        metadata: PresetMetadata {
            id: "stable-grid".into(),
            name: "Stable Grid".into(),
            category: "Test".into(),
            description: "Existing v2 serialization witness.".into(),
            thumbnail: None,
        },
        recipe: PatternDefinitionRecipe::StraightGrid(PatternDefinitionDraft {
            name: "Stable grid recipe".into(),
            coverage: CoveragePolicy {
                guard_steps: 2,
                additional_margin: 1.25,
            },
        }),
    };
    let path = temporary.path("stable-grid.preset.json");
    save_preset(&path, &preset).expect("preset saves");
    assert_eq!(
        fs::read_to_string(path).expect("preset reads"),
        concat!(
            "{\n",
            "  \"preset_format_version\": 2,\n",
            "  \"metadata\": {\n",
            "    \"id\": \"stable-grid\",\n",
            "    \"name\": \"Stable Grid\",\n",
            "    \"category\": \"Test\",\n",
            "    \"description\": \"Existing v2 serialization witness.\",\n",
            "    \"thumbnail\": null\n",
            "  },\n",
            "  \"recipe\": {\n",
            "    \"kind\": \"straight_grid\",\n",
            "    \"name\": \"Stable grid recipe\",\n",
            "    \"coverage\": {\n",
            "      \"guard_steps\": 2,\n",
            "      \"additional_margin\": 1.25\n",
            "    }\n",
            "  }\n",
            "}"
        )
    );
}

/// Keeps a current-v4 document without connection intent byte-stable after the connection DTO addition.
#[test]
fn document_v4_existing_mark_serialization_is_stable() {
    let temporary = TemporaryDirectory::new("document-witness");
    let source_id = SourceReferenceId::new("stable-source").expect("source ID");
    let document = Document::new_default_document(
        CanvasSpec {
            width: 90.0,
            height: 60.0,
        },
        SourceReference::Assigned(source_id.clone()),
    )
    .expect("default document");
    let sources = SourceBundle::new([EmbeddedSource::new(
        source_id,
        EmbeddedSourceFormat::Svg,
        b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>".to_vec(),
        Some("stable.svg".into()),
    )
    .expect("embedded source")])
    .expect("source bundle");
    let first = temporary.path("stable-a.toniator");
    let second = temporary.path("stable-b.toniator");
    save(&first, &document, &sources).expect("first save");
    save(&second, &document, &sources).expect("second save");
    let bytes = fs::read(&first).expect("first bytes");
    assert_eq!(bytes, fs::read(&second).expect("second bytes"));
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "d8ade2814e110b1d30700c0d4a8cdb0d1e286f660f1be16a10492a98b21a9af7"
    );
}

/// Locks the 24-across maze and 8-across Prim fixture contract without writing artifacts.
#[test]
fn grid_connection_fixture_configuration_preserves_triangular_lattice_inputs() {
    let source_id = SourceReferenceId::new("stage20m-triangular-fixture").expect("source ID");
    let maze2 = maze_document_for(source_id.clone(), 1024.0, 1024.0, false, 17);
    let maze3 = maze_document_for(source_id.clone(), 900.0, 620.0, true, 23);
    let prim3 = connection_document_for(
        source_id,
        900.0,
        620.0,
        true,
        ConnectionProgram::GridSpanningTree {
            adjacency: ConnectionAdjacencyIntent {
                maximum_degree: 6,
                maximum_distance: 192.0,
            },
            algorithm: GridSpanningTreeAlgorithm::RandomizedPrim,
            seed: 29,
        },
    );
    let maze2_definition = &maze2.pattern_definitions()[0];
    let maze3_definition = &maze3.pattern_definitions()[0];
    let PatternMechanism::StraightGuideDimensions { dimensions, .. } =
        &maze3_definition.mechanisms[0]
    else {
        panic!("three-guide fixture retains its generalized straight dimensions")
    };
    assert_eq!(
        dimensions
            .iter()
            .map(|dimension| (dimension.baseline_angle_degrees, dimension.phase))
            .collect::<Vec<_>>(),
        vec![(0.0, 0.0), (60.0, 0.0), (120.0, 0.0)]
    );
    let PatternMechanism::SelectedGuideIntersections { merge_epsilon, .. } =
        &maze3_definition.mechanisms[1]
    else {
        panic!("three-guide fixture retains its selected intersection product")
    };
    assert!(*merge_epsilon >= 1e-9);
    assert_eq!(
        maze3.pattern_settings().density.across_y,
        maze3.pattern_settings().density.across_x * 620.0 / 900.0
    );
    assert_eq!(maze2.pattern_settings().density.across_x, 24.0);
    assert_eq!(maze3.pattern_settings().density.across_x, 24.0);
    assert_eq!(
        maze2.pattern_settings().density.across_y,
        maze2.pattern_settings().density.across_x
    );
    for (definition, expected_seed) in [(maze2_definition, 17), (maze3_definition, 23)] {
        let [PatternOutputLayer::MazeWalls { program, .. }] = definition.output_layers.as_slice()
        else {
            panic!("maze fixture retains exactly one typed wall-maze output")
        };
        assert_eq!(program.algorithm, GridMazeAlgorithm::RecursiveBacktracker);
        assert_eq!(program.seed, expected_seed);
    }
    let [PatternOutputLayer::ConnectionPaths { program, .. }] =
        prim3.pattern_definitions()[0].output_layers.as_slice()
    else {
        panic!("prim fixture retains its positive tree output")
    };
    assert_eq!(program.adjacency().maximum_degree, 6);
    assert_eq!(prim3.pattern_settings().density.across_x, 8.0);
    assert_eq!(
        prim3.pattern_settings().density.across_y,
        prim3.pattern_settings().density.across_x * 620.0 / 900.0
    );
}

/// Writes the intentional intrinsic Stage 20M grid-program documents consumed by the headless export matrix.
#[test]
#[ignore = "explicit validation artifact generator"]
fn generate_stage20m_grid_connection_documents() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../");
    let output = root.join("target/validation/stage20m");
    fs::create_dir_all(&output).expect("stage directory");
    for (program_name, three_guides, seed) in [("maze2", false, 17), ("maze3", true, 23)] {
        for (source_name, format, bytes, width, height) in [
            (
                "raster",
                EmbeddedSourceFormat::Png,
                fs::read(root.join("assets/raster-sample.png")).expect("raster"),
                1024.0,
                1024.0,
            ),
            (
                "vector",
                EmbeddedSourceFormat::Svg,
                fs::read(root.join("assets/vector-sample.svg")).expect("vector"),
                900.0,
                620.0,
            ),
        ] {
            let source_id =
                SourceReferenceId::new(format!("stage20m-{program_name}-{source_name}"))
                    .expect("id");
            let document = maze_document_for(source_id.clone(), width, height, three_guides, seed);
            let sources = SourceBundle::new([EmbeddedSource::new(
                source_id,
                format,
                bytes,
                Some(format!("{source_name}-sample")),
            )
            .expect("source")])
            .expect("bundle");
            save(
                &output.join(format!("{program_name}-{source_name}.toniator")),
                &document,
                &sources,
            )
            .expect("save document");
        }
    }
    for (source_name, format, bytes, width, height) in [
        (
            "raster",
            EmbeddedSourceFormat::Png,
            fs::read(root.join("assets/raster-sample.png")).expect("raster"),
            1024.0,
            1024.0,
        ),
        (
            "vector",
            EmbeddedSourceFormat::Svg,
            fs::read(root.join("assets/vector-sample.svg")).expect("vector"),
            900.0,
            620.0,
        ),
    ] {
        let source_id = SourceReferenceId::new(format!("stage20m-prim-{source_name}")).expect("id");
        let document = connection_document_for(
            source_id.clone(),
            width,
            height,
            true,
            ConnectionProgram::GridSpanningTree {
                adjacency: ConnectionAdjacencyIntent {
                    maximum_degree: 6,
                    maximum_distance: 192.0,
                },
                algorithm: GridSpanningTreeAlgorithm::RandomizedPrim,
                seed: 29,
            },
        );
        let sources = SourceBundle::new([EmbeddedSource::new(
            source_id,
            format,
            bytes,
            Some(format!("{source_name}-sample")),
        )
        .expect("source")])
        .expect("bundle");
        save(
            &output.join(format!("prim-{source_name}.toniator")),
            &document,
            &sources,
        )
        .expect("save document");
    }
    for (source_name, format, bytes, width, height) in [
        (
            "raster",
            EmbeddedSourceFormat::Png,
            fs::read(root.join("assets/raster-sample.png")).expect("raster"),
            1024.0,
            1024.0,
        ),
        (
            "vector",
            EmbeddedSourceFormat::Svg,
            fs::read(root.join("assets/vector-sample.svg")).expect("vector"),
            900.0,
            620.0,
        ),
    ] {
        let source_id =
            SourceReferenceId::new(format!("stage20m-dispersion-{source_name}")).expect("id");
        let document = dispersion_document_for(source_id.clone(), width, height);
        let sources = SourceBundle::new([EmbeddedSource::new(
            source_id,
            format,
            bytes,
            Some(format!("{source_name}-sample")),
        )
        .expect("source")])
        .expect("bundle");
        save(
            &output.join(format!("dispersion-{source_name}.toniator")),
            &document,
            &sources,
        )
        .expect("save document");
    }
}
