use std::{
    collections::BTreeSet,
    fs,
    io::Cursor,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

use sha2::{Digest, Sha256};

use toniator_domain::{
    AuthoredCurveSegment, AuthoredPoint2, AuthoredStructure, AuthoredStructureId,
    AuthoredStructureKind, CanvasSpec, ChannelId, ConnectedGeometryResponse, ConnectionProgram,
    CoveragePolicy, DensityEditedAxis, DensityMetric2D, Document, DocumentCommand, DocumentHistory,
    DocumentSession, GeneralizedSiteProduct, GuideDimension, GuideDimensionId, GuidePrototype,
    GuideRepetition, MarkOrientation, ModeledMappingFieldEdit, OffsetCleanup, OffsetSides,
    PathStrokeStyle, PatternCapabilityScope, PatternDefinition, PatternDefinitionBundle,
    PatternDefinitionEdit, PatternDefinitionId, PatternGeometryResponse, PatternMechanism,
    PatternMechanismId, PatternOutputLayerId, PatternOutputRealization, PatternOutputSettings,
    PropertyDescriptor, SourceMappingComponent, SourcePlacement, SourceReference,
    SourceReferenceId,
};
use toniator_engine::{
    CacheDisposition, ConnectionPathLimits, EvaluationCompletion, EvaluationLimits,
    EvaluationProfileCache, EvaluationRequest, EvaluationScheduler, GeometryOutput, ResolvedSource,
    SourceFormatHint, encode_png, evaluate, evaluate_profiled_cached_with_limits,
    evaluate_profiled_with_limits, evaluate_with_limits, write_svg,
};
use toniator_io::{load_preset, save_preset};
use toniator_patterns::PresetRegistry;

/// Stable validation-only curved-guide fixture IDs expected by future artifact inventory checks.
const CURVED_GUIDE_FIXTURE_IDS: [&str; 6] = [
    "curved-one-stack-paths",
    "curved-one-stack-sites",
    "curved-one-normal-offset-paths",
    "curved-one-normal-offset-sites",
    "curved-two-stack-paths",
    "curved-two-stack-intersections",
];

/// Stable catalog cards whose materialized spirals cover the document canvas
/// and therefore retain their separately measured 25%-linear evidence policy.
const CANVAS_COVERING_SPIRAL_PRESET_IDS: [&str; 3] = [
    "round-spiral-line",
    "round-spiral-marks",
    "square-spiral-marks",
];

/// Slow or request-budget-bound raster cards that use the deterministic 256-square
/// derived raster source and matching evidence canvas after measured preflight.
const DERIVED_RASTER_ARTIFACT_PRESET_IDS: [&str; 9] = [
    "clustered-dispersion-random-links",
    "grid-voronoi-scale",
    "one-guide-lines",
    "residual-sites-along-guide",
    "source-weighted-dispersion-voronoi",
    "two-guide-maze",
    "round-spiral-line",
    "round-spiral-marks",
    "square-spiral-marks",
];

/// Slow vector-assigned catalog cards that use the deterministic 225-by-155
/// derived SVG source and matching evidence canvas after measured preflight.
const DERIVED_SVG_ARTIFACT_PRESET_IDS: [&str; 4] = [
    "three-guide-cells-scale",
    "three-guide-maze",
    "triagrid-spanning-tree",
    "two-guide-cells-uniform-offset",
];

/// Immutable and derived-input provenance retained by the Stage 20S manifest.
#[derive(Clone, Debug, PartialEq, Eq)]
struct DerivedInputRecord {
    source_path: &'static str,
    derived_path: PathBuf,
    format: SourceFormatHint,
    dimensions: (u32, u32),
    source_sha256: String,
    derived_sha256: String,
    method: &'static str,
}

/// Couples one immutable or derived source to the document canvas used by an
/// artifact card while retaining bounded provenance text for the manifest.
struct ArtifactInput {
    source_path: PathBuf,
    format: SourceFormatHint,
    source_dimensions: (u32, u32),
    canvas_dimensions: (f64, f64),
    manifest_note: String,
}

/// Creates deterministic quarter-resolution raster and intrinsic SVG inputs
/// while preserving immutable source bytes and SVG viewBox/live-text content.
fn write_stage20s_derived_inputs(output: &Path) -> [DerivedInputRecord; 2] {
    let derived = output.join("derived-inputs");
    fs::create_dir_all(&derived).expect("derived input directory creates");
    let raster_source_path = Path::new("../../assets/raster-sample.png");
    let vector_source_path = Path::new("../../assets/vector-sample.svg");
    let raster_source = fs::read(raster_source_path).expect("immutable raster source reads");
    let vector_source = fs::read(vector_source_path).expect("immutable vector source reads");
    let raster_hash = sha256(&raster_source);
    let vector_hash = sha256(&vector_source);
    let raster_path = derived.join("raster-sample-25pct.png");
    let resized = image::load_from_memory(&raster_source)
        .expect("immutable raster source decodes")
        .resize_exact(256, 256, image::imageops::FilterType::Lanczos3);
    resized
        .save(&raster_path)
        .expect("deterministic raster resize writes");
    let dimensions = image::image_dimensions(&raster_path).expect("derived raster dimensions read");
    assert_eq!(dimensions, (256, 256));
    let vector_path = derived.join("vector-sample-25pct.svg");
    let vector_text = String::from_utf8(vector_source.clone()).expect("immutable SVG is UTF-8");
    let resized_vector = vector_text.replacen(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"900\" height=\"620\" viewBox=\"0 0 900 620\">",
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"225\" height=\"155\" viewBox=\"0 0 900 620\">",
        1,
    );
    assert!(resized_vector.contains("viewBox=\"0 0 900 620\""));
    assert!(resized_vector.contains("<text"));
    fs::write(&vector_path, resized_vector.as_bytes()).expect("derived SVG writes");
    assert_eq!(
        sha256(&fs::read(raster_source_path).expect("raster source rereads")),
        raster_hash
    );
    assert_eq!(
        sha256(&fs::read(vector_source_path).expect("vector source rereads")),
        vector_hash
    );
    [
        DerivedInputRecord {
            source_path: "assets/raster-sample.png",
            derived_path: raster_path.clone(),
            format: SourceFormatHint::Png,
            dimensions,
            source_sha256: raster_hash,
            derived_sha256: sha256(&fs::read(raster_path).expect("derived raster reads")),
            method: "image::imageops::FilterType::Lanczos3 resize_exact(256,256)",
        },
        DerivedInputRecord {
            source_path: "assets/vector-sample.svg",
            derived_path: vector_path.clone(),
            format: SourceFormatHint::Svg,
            dimensions: (225, 155),
            source_sha256: vector_hash,
            derived_sha256: sha256(&fs::read(vector_path).expect("derived SVG reads")),
            method: "root intrinsic width/height rewrite; preserved viewBox/content/live text",
        },
    ]
}

/// Exact canonical bytes and channel identities from one document evaluation.
/// This test-only value records the public engine boundary without introducing
/// a preset evaluator, renderer, or cache path.
#[derive(Clone, Debug, PartialEq, Eq)]
struct CanonicalOutput {
    png: Vec<u8>,
    svg: String,
    channels: Vec<ChannelCanonicalIdentity>,
}

/// Public per-channel identity exposed by document evaluation in authoritative
/// channel order. The values are compared across a red-only typed edit to
/// prove unaffected green and blue realization boundaries remain stable.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ChannelCanonicalIdentity {
    channel_id: ChannelId,
    family: String,
    realization: String,
}

/// Builds an ordinary RGB document history for tests that require solid path paint.
fn history(source_id: SourceReferenceId, width: f64, height: f64) -> DocumentHistory {
    let document =
        Document::new_default_document(CanvasSpec { width, height }, SourceReference::Unassigned)
            .unwrap();
    let mut history = DocumentHistory::new(DocumentSession::new(document).unwrap());
    history
        .apply(&DocumentCommand::SetSourceReference {
            source: SourceReference::Assigned(source_id),
        })
        .unwrap();
    history
}

/// Builds an ordinary visible RGB artifact history and returns its three
/// canonical channels plus the fixed topology/paint label.
///
/// The harness never changes product paint semantics: every modeled channel
/// retains its ordinary solid red, green, or blue paint and matching
/// Red/Green/Blue Stretch/gain-one/bias-zero mapping while the recipe is
/// independently materialized into all three channel definitions.
fn artifact_history(
    source_id: SourceReferenceId,
    width: f64,
    height: f64,
) -> (DocumentHistory, [ChannelId; 3], &'static str) {
    let mut history = history(source_id, width, height);
    let channels = history
        .document()
        .channel_topology()
        .unwrap()
        .channels()
        .iter()
        .map(|channel| channel.id)
        .collect::<Vec<_>>();
    assert_eq!(channels, vec![ChannelId(1), ChannelId(2), ChannelId(3)]);
    assert!(
        history
            .document()
            .channel_topology()
            .unwrap()
            .channels()
            .iter()
            .all(|channel| channel.visible)
    );
    for (channel_id, component) in [
        (ChannelId(1), SourceMappingComponent::Red),
        (ChannelId(2), SourceMappingComponent::Green),
        (ChannelId(3), SourceMappingComponent::Blue),
    ] {
        if history
            .document()
            .channel_topology()
            .expect("artifact topology remains available")
            .channels()
            .iter()
            .find(|channel| channel.id == channel_id)
            .expect("artifact channel remains available")
            .mapping
            .component
            != component
        {
            history
                .apply(&DocumentCommand::SetModeledMappingField {
                    channel_id,
                    edit: ModeledMappingFieldEdit::Component(component),
                })
                .expect("artifact channel accepts its ordinary RGB mapping");
        }
    }
    assert!(
        history
            .document()
            .channel_topology()
            .unwrap()
            .channels()
            .iter()
            .zip([
                SourceMappingComponent::Red,
                SourceMappingComponent::Green,
                SourceMappingComponent::Blue,
            ])
            .all(|(channel, component)| {
                channel.mapping.component == component
                    && channel.mapping.placement == SourcePlacement::StretchToCanvas
                    && channel.mapping.gain == 1.0
                    && channel.mapping.bias == 0.0
            })
    );
    (
        history,
        [ChannelId(1), ChannelId(2), ChannelId(3)],
        "RGB / three visible solid channel paints; Red, Green, Blue respectively with Stretch/gain=1/bias=0 mappings",
    )
}

/// Builds one validation-only cubic TransformStack document with either guide
/// paths or AlongGuides circle marks shared by three visible RGB channels.
///
/// Authored points remain centered-local because generic guide prototype
/// placement owns their local-to-document translation; their post-placement
/// envelope intentionally spans past both horizontal canvas boundaries.
fn curved_one_stack_history_for_canvas(
    source_id: SourceReferenceId,
    canvas: CanvasSpec,
    paths: bool,
    normal_offset: bool,
) -> DocumentHistory {
    let base = Document::new_default_document(canvas.clone(), SourceReference::Assigned(source_id))
        .expect("small RGB document validates");
    let mut definition = PatternDefinition::generalized_guides(
        PatternDefinitionId(1),
        "curved-one-stack-paths",
        PatternMechanismId(1),
        PatternMechanismId(2),
        PatternOutputLayerId(1),
        vec![GuideDimension {
            id: GuideDimensionId(1),
            baseline_angle_degrees: 0.0,
            phase: 0.0,
            prototype: GuidePrototype::AuthoredOpenPath {
                structure_id: AuthoredStructureId(1),
            },
            repetition: if normal_offset {
                GuideRepetition::NormalOffset {
                    spacing: canvas.height * 0.5,
                    sides: OffsetSides::Both,
                    cleanup: OffsetCleanup::DissolveCrossings,
                }
            } else {
                GuideRepetition::TransformStack {
                    direction_degrees: 90.0,
                    spacing_multiplier: 1.0,
                }
            },
        }],
        GeneralizedSiteProduct::AlongGuides {
            dimensions: vec![GuideDimensionId(1)],
            interval_multiplier: 1.0,
            phase: 0.0,
        },
        MarkOrientation::Fixed,
        CoveragePolicy {
            guard_steps: 2,
            additional_margin: 0.0,
        },
    );
    if paths {
        definition.output_layers[0].realization = PatternOutputRealization::GuidePaths {
            guide_mechanism_id: PatternMechanismId(1),
            style: PathStrokeStyle::default(),
        };
    }
    let structure = AuthoredStructure::new(
        AuthoredStructureId(1),
        AuthoredStructureKind::OpenPath,
        vec![if normal_offset {
            AuthoredCurveSegment::CubicBezier {
                start: AuthoredPoint2 {
                    x: -0.75 * canvas.width,
                    y: 5.0 / 24.0 * canvas.height,
                },
                control_1: AuthoredPoint2 {
                    x: -0.375 * canvas.width,
                    y: -7.0 / 24.0 * canvas.height,
                },
                control_2: AuthoredPoint2 {
                    x: 0.25 * canvas.width,
                    y: -7.0 / 24.0 * canvas.height,
                },
                end: AuthoredPoint2 {
                    x: 0.75 * canvas.width,
                    y: 5.0 / 24.0 * canvas.height,
                },
            }
        } else {
            AuthoredCurveSegment::CubicBezier {
                start: AuthoredPoint2 {
                    x: -0.75 * canvas.width,
                    y: 0.0,
                },
                control_1: AuthoredPoint2 {
                    x: -0.375 * canvas.width,
                    y: -5.0 / 6.0 * canvas.height,
                },
                control_2: AuthoredPoint2 {
                    x: 0.25 * canvas.width,
                    y: 5.0 / 6.0 * canvas.height,
                },
                end: AuthoredPoint2 {
                    x: 0.75 * canvas.width,
                    y: 0.0,
                },
            }
        }],
    )
    .expect("open cubic structure validates");
    let document = Document::with_source_topology_and_authored_structures(
        base.id(),
        base.canvas().clone(),
        base.source().clone(),
        vec![PatternDefinitionBundle {
            definition,
            output_settings: vec![PatternOutputSettings {
                output_layer_id: PatternOutputLayerId(1),
                response: if paths {
                    PatternGeometryResponse::Connected(ConnectedGeometryResponse {
                        minimum_thickness: 0.15,
                        maximum_thickness: 0.65,
                    })
                } else {
                    PatternGeometryResponse::Marks(toniator_domain::MarkGeometryResponse {
                        minimum_fill: 0.25,
                        maximum_fill: 0.85,
                    })
                },
            }],
        }],
        base.pattern_settings().clone(),
        base.channel_model().expect("modeled RGB").to_owned(),
        base.channel_topology().expect("modeled RGB").clone(),
        vec![structure],
    )
    .expect("cubic guide document validates");
    let mut history = DocumentHistory::new(
        DocumentSession::new(document).expect("cubic guide session validates"),
    );
    for (channel_id, component) in [
        (ChannelId(1), SourceMappingComponent::Red),
        (ChannelId(2), SourceMappingComponent::Green),
        (ChannelId(3), SourceMappingComponent::Blue),
    ] {
        if history
            .document()
            .channel_topology()
            .expect("cubic RGB topology remains available")
            .channels()
            .iter()
            .find(|channel| channel.id == channel_id)
            .expect("cubic RGB channel remains available")
            .mapping
            .component
            != component
        {
            history
                .apply(&DocumentCommand::SetModeledMappingField {
                    channel_id,
                    edit: ModeledMappingFieldEdit::Component(component),
                })
                .expect("cubic RGB mapping command applies");
        }
    }
    history
}

/// Builds the established 32-by-24 curved fixture through the canvas-aware builder.
fn curved_one_stack_history(
    source_id: SourceReferenceId,
    paths: bool,
    normal_offset: bool,
) -> DocumentHistory {
    curved_one_stack_history_for_canvas(
        source_id,
        CanvasSpec {
            width: 32.0,
            height: 24.0,
        },
        paths,
        normal_offset,
    )
}

/// Builds the validation-only two-cubic TransformStack path or mark document.
///
/// Both authored cubics retain centered-local horizontal baseline coordinates.
/// The second dimension's 90-degree baseline rotates its distinct local cubic
/// vertical while each relative 90-degree TransformStack repeats orthogonally.
fn curved_two_stack_history_for_canvas(
    source_id: SourceReferenceId,
    canvas: CanvasSpec,
    paths: bool,
) -> DocumentHistory {
    let base = Document::new_default_document(canvas.clone(), SourceReference::Assigned(source_id))
        .expect("small RGB document validates");
    let mut definition = PatternDefinition::generalized_guides(
        PatternDefinitionId(1),
        "curved-two-stack-paths",
        PatternMechanismId(1),
        PatternMechanismId(2),
        PatternOutputLayerId(1),
        vec![
            GuideDimension {
                id: GuideDimensionId(1),
                baseline_angle_degrees: 0.0,
                phase: 0.0,
                prototype: GuidePrototype::AuthoredOpenPath {
                    structure_id: AuthoredStructureId(11),
                },
                repetition: GuideRepetition::TransformStack {
                    direction_degrees: 90.0,
                    spacing_multiplier: 1.0,
                },
            },
            GuideDimension {
                id: GuideDimensionId(2),
                baseline_angle_degrees: 90.0,
                phase: 0.0,
                prototype: GuidePrototype::AuthoredOpenPath {
                    structure_id: AuthoredStructureId(12),
                },
                repetition: GuideRepetition::TransformStack {
                    direction_degrees: 90.0,
                    spacing_multiplier: 1.0,
                },
            },
        ],
        GeneralizedSiteProduct::Intersections {
            dimensions: vec![GuideDimensionId(1), GuideDimensionId(2)],
            merge_epsilon: 1e-9,
        },
        MarkOrientation::Fixed,
        CoveragePolicy {
            guard_steps: 2,
            additional_margin: 0.0,
        },
    );
    if paths {
        definition.output_layers[0].realization = PatternOutputRealization::GuidePaths {
            guide_mechanism_id: PatternMechanismId(1),
            style: PathStrokeStyle::default(),
        };
    }
    let structures = vec![
        AuthoredStructure::new(
            AuthoredStructureId(11),
            AuthoredStructureKind::OpenPath,
            vec![AuthoredCurveSegment::CubicBezier {
                start: AuthoredPoint2 {
                    x: -0.75 * canvas.width,
                    y: 5.0 / 24.0 * canvas.height,
                },
                control_1: AuthoredPoint2 {
                    x: -0.375 * canvas.width,
                    y: -7.0 / 24.0 * canvas.height,
                },
                control_2: AuthoredPoint2 {
                    x: 0.25 * canvas.width,
                    y: -7.0 / 24.0 * canvas.height,
                },
                end: AuthoredPoint2 {
                    x: 0.75 * canvas.width,
                    y: 5.0 / 24.0 * canvas.height,
                },
            }],
        )
        .expect("horizontal cubic validates"),
        AuthoredStructure::new(
            AuthoredStructureId(12),
            AuthoredStructureKind::OpenPath,
            vec![AuthoredCurveSegment::CubicBezier {
                start: AuthoredPoint2 {
                    x: -0.75 * canvas.width,
                    y: -0.25 * canvas.height,
                },
                control_1: AuthoredPoint2 {
                    x: -0.375 * canvas.width,
                    y: 0.3 * canvas.height,
                },
                control_2: AuthoredPoint2 {
                    x: 0.25 * canvas.width,
                    y: -0.2 * canvas.height,
                },
                end: AuthoredPoint2 {
                    x: 0.75 * canvas.width,
                    y: 0.15 * canvas.height,
                },
            }],
        )
        .expect("baseline-local horizontal cubic validates"),
    ];
    let document = Document::with_source_topology_and_authored_structures(
        base.id(),
        base.canvas().clone(),
        base.source().clone(),
        vec![PatternDefinitionBundle {
            definition,
            output_settings: vec![PatternOutputSettings {
                output_layer_id: PatternOutputLayerId(1),
                response: if paths {
                    PatternGeometryResponse::Connected(ConnectedGeometryResponse {
                        minimum_thickness: 0.15,
                        maximum_thickness: 0.65,
                    })
                } else {
                    PatternGeometryResponse::Marks(toniator_domain::MarkGeometryResponse {
                        minimum_fill: 0.25,
                        maximum_fill: 0.85,
                    })
                },
            }],
        }],
        base.pattern_settings().clone(),
        base.channel_model().expect("modeled RGB").to_owned(),
        base.channel_topology().expect("modeled RGB").clone(),
        structures,
    )
    .expect("two-cubic guide document validates");
    let mut history =
        DocumentHistory::new(DocumentSession::new(document).expect("two-cubic session validates"));
    for (channel_id, component) in [
        (ChannelId(1), SourceMappingComponent::Red),
        (ChannelId(2), SourceMappingComponent::Green),
        (ChannelId(3), SourceMappingComponent::Blue),
    ] {
        if history
            .document()
            .channel_topology()
            .expect("two-cubic RGB topology")
            .channels()
            .iter()
            .find(|channel| channel.id == channel_id)
            .expect("two-cubic RGB channel")
            .mapping
            .component
            != component
        {
            history
                .apply(&DocumentCommand::SetModeledMappingField {
                    channel_id,
                    edit: ModeledMappingFieldEdit::Component(component),
                })
                .expect("two-cubic RGB mapping applies");
        }
    }
    history
}

/// Builds the established 32-by-24 two-cubic fixture through the canvas-aware builder.
fn curved_two_stack_history(source_id: SourceReferenceId, paths: bool) -> DocumentHistory {
    curved_two_stack_history_for_canvas(
        source_id,
        CanvasSpec {
            width: 32.0,
            height: 24.0,
        },
        paths,
    )
}

/// Builds one stable curved-guide fixture variant at a caller-provided canvas
/// without changing the reusable small-fixture or product construction rules.
fn curved_guide_history_for_variant(
    id: &str,
    source_id: SourceReferenceId,
    canvas: CanvasSpec,
) -> DocumentHistory {
    match id {
        "curved-one-stack-paths" => {
            curved_one_stack_history_for_canvas(source_id, canvas, true, false)
        }
        "curved-one-stack-sites" => {
            curved_one_stack_history_for_canvas(source_id, canvas, false, false)
        }
        "curved-one-normal-offset-paths" => {
            curved_one_stack_history_for_canvas(source_id, canvas, true, true)
        }
        "curved-one-normal-offset-sites" => {
            curved_one_stack_history_for_canvas(source_id, canvas, false, true)
        }
        "curved-two-stack-paths" => curved_two_stack_history_for_canvas(source_id, canvas, true),
        "curved-two-stack-intersections" => {
            curved_two_stack_history_for_canvas(source_id, canvas, false)
        }
        _ => unreachable!("curved fixture IDs are stable"),
    }
}

/// Applies one catalog recipe independently to every ordinary RGB channel.
///
/// Each application travels through the public selected-channel history path.
/// It therefore allocates independent definitions and cannot leave green or
/// blue with the document's default circle-mark topology.
fn apply_recipe_to_rgb(
    registry: &PresetRegistry,
    history: &mut DocumentHistory,
    channels: [ChannelId; 3],
    preset_id: &str,
) {
    let initial = channels.map(|channel_id| {
        history
            .document()
            .pattern_definition_for(channel_id)
            .expect("RGB channel has a default definition")
            .clone()
    });
    for channel_id in channels {
        let result = registry
            .apply_to_selected(history, channel_id, preset_id)
            .expect("catalog recipe materializes for every RGB channel");
        assert_eq!(result.affected_channels, vec![channel_id]);
        let component = match channel_id.0 {
            1 => SourceMappingComponent::Red,
            2 => SourceMappingComponent::Green,
            3 => SourceMappingComponent::Blue,
            _ => unreachable!("artifact topology is fixed to RGB channels"),
        };
        let modulation_ids = history
            .document()
            .pattern_definition_for(channel_id)
            .expect("materialized RGB channel definition exists")
            .mechanisms
            .iter()
            .filter_map(|mechanism| match mechanism {
                PatternMechanism::SiteDensityModulation {
                    id,
                    modulation: toniator_domain::SiteDensityModulation::ArtworkWeighted { .. },
                    ..
                } => Some(*id),
                _ => None,
            })
            .collect::<Vec<_>>();
        for mechanism_id in modulation_ids {
            let base_definition = history
                .document()
                .pattern_definition_for(channel_id)
                .expect("channel definition remains current between weighted edits")
                .clone();
            history
                .apply(&DocumentCommand::EditSelectedChannelPatternDefinition {
                    channel_id,
                    base_definition,
                    edit: PatternDefinitionEdit::SetArtworkWeightMappingComponent {
                        mechanism_id,
                        component,
                    },
                })
                .expect("weighted site mapping follows its RGB channel");
        }
        assert!(
            history
                .document()
                .pattern_definition_for(channel_id)
                .expect("weighted RGB definition remains readable")
                .mechanisms
                .iter()
                .filter_map(|mechanism| match mechanism {
                    PatternMechanism::SiteDensityModulation {
                        modulation:
                            toniator_domain::SiteDensityModulation::ArtworkWeighted { mapping, .. },
                        ..
                    } => Some(mapping.component),
                    _ => None,
                })
                .all(|weighted_component| weighted_component == component),
            "every artwork-weighted family mapping follows its channel component"
        );
    }
    let output_kinds = channels.map(|channel_id| {
        history
            .document()
            .pattern_definition_for(channel_id)
            .expect("materialized RGB channel retains its definition")
            .output_layers
            .iter()
            .map(|output| match output.realization {
                PatternOutputRealization::CircularMarks { .. } => "circular_marks",
                PatternOutputRealization::MarkPrototype { .. } => "mark_prototype",
                PatternOutputRealization::GuidePaths { .. } => "guide_paths",
                PatternOutputRealization::ParametricPaths { .. } => "parametric_paths",
                PatternOutputRealization::ConnectionPaths { .. } => "connection_paths",
                PatternOutputRealization::MazeWalls { .. } => "maze_walls",
                PatternOutputRealization::Regions { .. } => "regions",
            })
            .collect::<Vec<_>>()
    });
    assert_eq!(output_kinds[0], output_kinds[1]);
    assert_eq!(output_kinds[0], output_kinds[2]);
    for (channel_id, default_definition) in channels.into_iter().zip(initial) {
        assert_ne!(
            history
                .document()
                .pattern_definition_for(channel_id)
                .expect("materialized RGB channel definition exists"),
            &default_definition,
            "every RGB channel replaces its default circle-mark definition"
        );
    }
}

/// Applies an authorized artifact-only resolution density to all three modeled
/// channels without changing a bundled recipe, product default, or request limit.
///
/// The intrinsic one-guide representative uses `AcrossX=4`; derived validation documents use
/// `AcrossX=16`. Measured slow cards use the corresponding bounded density with
/// their 25%-linear source and canvas so site and cell work reduce at both
/// relevant boundaries while the base aspect lock derives the companion axis.
fn apply_artifact_resolution_density(
    history: &mut DocumentHistory,
    channels: [ChannelId; 3],
    preset_id: &str,
) -> String {
    let density = if preset_id == "one-guide-lines" {
        4.0
    } else if DERIVED_RASTER_ARTIFACT_PRESET_IDS.contains(&preset_id)
        || DERIVED_SVG_ARTIFACT_PRESET_IDS.contains(&preset_id)
        || CURVED_GUIDE_FIXTURE_IDS.contains(&preset_id)
        || preset_id == "heterogeneous-rgb-recipes"
    {
        16.0
    } else {
        return "default document density (no artifact-only edit)".to_owned();
    };
    let canvas = history.document().canvas();
    let expected_y = density * canvas.height / canvas.width;
    for channel_id in channels {
        let edit = history
            .document()
            .set_channel_density_for_effective(
                channel_id,
                DensityEditedAxis::AcrossX,
                DensityMetric2D {
                    across_x: density,
                    across_y: density,
                    aspect_locked: true,
                },
            )
            .expect("artifact resolution density command validates");
        history
            .apply(&edit)
            .expect("artifact resolution density command applies");
        if preset_id == "one-guide-lines" {
            let definition = history
                .document()
                .pattern_definition_for(channel_id)
                .expect("one-guide channel retains its materialized definition");
            assert!(
                definition.output_layers.iter().all(|output| matches!(
                    output.realization,
                    PatternOutputRealization::GuidePaths { .. }
                )),
                "artifact-only density leaves every RGB channel as a one-guide structural path"
            );
        }
        let effective_density = history
            .document()
            .effective_channel_pattern(channel_id)
            .expect("artifact channel retains effective density")
            .density;
        assert!(
            effective_density.aspect_locked,
            "artifact resolution retains the document-owned aspect lock"
        );
        assert_eq!(effective_density.across_x, density);
        assert!(
            (effective_density.across_y - expected_y).abs() <= 1e-12,
            "artifact resolution retains the domain-derived locked Y axis"
        );
    }
    let axes = if (expected_y - density).abs() <= 1e-12 {
        format!("{density:.0}x{density:.0}")
    } else {
        format!("{density:.0}x{expected_y:.4}")
    };
    format!(
        "typed SetChannelDensityDelta through set_channel_density_for_effective with AcrossX authoritative and base aspect lock: R={axes}, G={axes}, B={axes}"
    )
}

/// Applies deterministic channel-distinct edits to every randomized authored
/// seed in one materialized RGB definition and returns audit-ready edit text.
///
/// The edits use existing typed history commands only. They deliberately run
/// after all recipe applications, preserving bundled defaults and recipe
/// identity tests outside the artifact-only document mutations.
fn apply_distinct_channel_seeds(
    history: &mut DocumentHistory,
    channel_id: ChannelId,
) -> Vec<String> {
    let definition = history
        .document()
        .pattern_definition_for(channel_id)
        .expect("materialized channel has a definition")
        .clone();
    let mut edits = Vec::new();
    for (ordinal, mechanism_id) in definition
        .mechanisms
        .iter()
        .filter_map(|mechanism| match mechanism {
            PatternMechanism::RandomSiteProcess { id, .. } => Some(*id),
            _ => None,
        })
        .enumerate()
    {
        let seed = 10_000 + channel_id.0 as u32 * 100 + ordinal as u32;
        let base_definition = history
            .document()
            .pattern_definition_for(channel_id)
            .expect("channel definition remains current between seed edits")
            .clone();
        history
            .apply(&DocumentCommand::EditSelectedChannelPatternDefinition {
                channel_id,
                base_definition,
                edit: PatternDefinitionEdit::SetRandomSeed { mechanism_id, seed },
            })
            .expect("random seed edit remains valid");
        edits.push(format!("random mechanism {}={seed}", mechanism_id.0));
    }
    for (ordinal, output_layer_id) in
        definition
            .output_layers
            .iter()
            .filter_map(|output| match output.realization {
                PatternOutputRealization::ConnectionPaths {
                    program:
                        ConnectionProgram::RandomLinks { .. }
                        | ConnectionProgram::GridSpanningTree { .. },
                    ..
                } => Some(output.id),
                _ => None,
            })
            .enumerate()
    {
        let seed = 20_000 + channel_id.0 as u32 * 100 + ordinal as u32;
        let base_definition = history
            .document()
            .pattern_definition_for(channel_id)
            .expect("channel definition remains current between seed edits")
            .clone();
        history
            .apply(&DocumentCommand::EditSelectedChannelPatternDefinition {
                channel_id,
                base_definition,
                edit: PatternDefinitionEdit::SetConnectionSeed {
                    output_layer_id,
                    seed,
                },
            })
            .expect("connection seed edit remains valid");
        edits.push(format!("connection output {}={seed}", output_layer_id.0));
    }
    for (ordinal, output_layer_id) in definition
        .output_layers
        .iter()
        .filter_map(|output| match output.realization {
            PatternOutputRealization::MazeWalls { .. } => Some(output.id),
            _ => None,
        })
        .enumerate()
    {
        let seed = 30_000 + channel_id.0 as u32 * 100 + ordinal as u32;
        let base_definition = history
            .document()
            .pattern_definition_for(channel_id)
            .expect("channel definition remains current between seed edits")
            .clone();
        history
            .apply(&DocumentCommand::EditSelectedChannelPatternDefinition {
                channel_id,
                base_definition,
                edit: PatternDefinitionEdit::SetMazeSeed {
                    output_layer_id,
                    seed,
                },
            })
            .expect("maze seed edit remains valid");
        edits.push(format!("maze output {}={seed}", output_layer_id.0));
    }
    edits
}

/// Reports every effective randomized seed and output kind for one channel.
///
/// The value is used only by artifact evidence and assertions; evaluator
/// state, cache identity, and the serialized bundled preset remain unchanged.
fn channel_definition_audit(history: &DocumentHistory, channel_id: ChannelId) -> String {
    let definition = history
        .document()
        .pattern_definition_for(channel_id)
        .expect("audited channel retains a definition");
    let mut fields = vec![format!("definition={}", definition.id.0)];
    for mechanism in &definition.mechanisms {
        if let PatternMechanism::RandomSiteProcess { id, seed, .. } = mechanism {
            fields.push(format!("random:{}={seed}", id.0));
        }
        if let PatternMechanism::SiteDensityModulation {
            id,
            modulation: toniator_domain::SiteDensityModulation::ArtworkWeighted { mapping, .. },
            ..
        } = mechanism
        {
            fields.push(format!("artwork_weight:{}:{:?}", id.0, mapping.component));
        }
    }
    for output in &definition.output_layers {
        match &output.realization {
            PatternOutputRealization::ConnectionPaths { program, .. } => {
                fields.push(format!("connection:{}:{:?}", output.id.0, program.seed()));
            }
            PatternOutputRealization::MazeWalls { program, .. } => {
                fields.push(format!("maze:{}={}", output.id.0, program.seed));
            }
            realization => fields.push(format!("output:{}:{realization:?}", output.id.0)),
        }
    }
    fields.join(", ")
}

/// Asserts that three modeled channel seed-edit lists assign distinct values
/// for each corresponding typed random, connection, or maze seed field.
fn assert_distinct_rgb_seed_edits(seed_edits: &[Vec<String>; 3]) {
    for kind in ["random mechanism", "connection output", "maze output"] {
        let values = seed_edits
            .iter()
            .flat_map(|edits| edits.iter().filter(move |edit| edit.starts_with(kind)))
            .filter_map(|edit| edit.rsplit_once('=').map(|(_, value)| value))
            .collect::<Vec<_>>();
        if !values.is_empty() {
            assert_eq!(values.len() % 3, 0, "seed audit retains every RGB value");
            for group in values.chunks_exact(3) {
                assert_ne!(group[0], group[1]);
                assert_ne!(group[0], group[2]);
                assert_ne!(group[1], group[2]);
            }
        }
    }
}

/// Records the parent-measured preflight that authorizes one catalog card's
/// intrinsic or derived evidence resolution without changing product behavior.
fn artifact_resolution_preflight_note(preset_id: &str) -> &'static str {
    match preset_id {
        "clustered-dispersion-random-links"
        | "residual-sites-along-guide"
        | "source-weighted-dispersion-voronoi"
        | "two-guide-maze" => {
            "intrinsic assigned source/canvas exceeded the 40s external timeout; final acceptance uses the 25%-linear derived raster source and 256x256 canvas"
        }
        "grid-voronoi-scale" => {
            "intrinsic assigned source/canvas passed in 36.89s, above the 30s demonstration threshold; final acceptance uses the 25%-linear derived raster source and 256x256 canvas"
        }
        "three-guide-cells-scale"
        | "three-guide-maze"
        | "triagrid-spanning-tree"
        | "two-guide-cells-uniform-offset" => {
            "intrinsic assigned source/canvas exceeded the 40s external timeout; final acceptance uses the 25%-linear derived SVG source and 225x155 canvas"
        }
        "round-spiral-line" | "round-spiral-marks" | "square-spiral-marks" => {
            "immutable full-resolution source and 1024x1024 canvas failed after 0.72s at ordinary `realization.stroke.profile_limit` (not a timeout); derived 256x256 source with 1024x1024 canvas exceeded the 40s external timeout without a diagnostic; final acceptance uses the derived raster source and 256x256 canvas"
        }
        "even-random-circles" => {
            "intrinsic assigned source/canvas passed in 21.13s, below the 30s demonstration threshold; intrinsic input remains the acceptance policy"
        }
        "triagrid-custom-shape-marks" => {
            "intrinsic assigned source/canvas passed in 23.91s, below the 30s demonstration threshold; intrinsic input remains the acceptance policy"
        }
        "one-guide-lines" => {
            "intrinsic assigned source/canvas reaches the request-wide canonical stroke budget across three RGB outputs; final acceptance uses the 25%-linear derived raster source and 256x256 canvas with artifact-only density 4x4"
        }
        "straight-grid-circles" => {
            "ordinary small mark baseline retains its intrinsic input; no slow preflight requires a derived evidence resolution"
        }
        _ => unreachable!("the current 16-card registry has an explicit resolution policy"),
    }
}

/// Assigns one immutable or derived project-wide source and document canvas to
/// a stable catalog card without changing authored recipe or evaluator intent.
///
/// Derived routes use only documented 25%-linear evidence inputs. In
/// particular, the canvas-covering spirals materialize CoverCanvas against the
/// matching 256-square canvas, rather than changing any product-time curve.
fn artifact_input(
    preset_id: &str,
    derived_inputs: Option<&[DerivedInputRecord; 2]>,
) -> ArtifactInput {
    if DERIVED_RASTER_ARTIFACT_PRESET_IDS.contains(&preset_id) {
        let derived = derived_inputs
            .expect("slow raster acceptance prepares the derived raster before its catalog loop");
        let raster = &derived[0];
        let cover_canvas = CANVAS_COVERING_SPIRAL_PRESET_IDS.contains(&preset_id);
        let density_note = if preset_id == "one-guide-lines" {
            "4x4"
        } else {
            "16x16"
        };
        return ArtifactInput {
            source_path: raster.derived_path.clone(),
            format: raster.format,
            source_dimensions: raster.dimensions,
            canvas_dimensions: (256.0, 256.0),
            manifest_note: format!(
                "derived 256x256 raster source from `{}` (source_sha256={}, derived_sha256={}, method={}); document/output canvas is 256x256{}; preflight: {}; artifact-only per-channel density is {density_note}; no recipe, default, or request-limit change.",
                raster.source_path,
                raster.source_sha256,
                raster.derived_sha256,
                raster.method,
                if cover_canvas {
                    " and CoverCanvas materializes against that canvas"
                } else {
                    ""
                },
                artifact_resolution_preflight_note(preset_id),
            ),
        };
    }
    if DERIVED_SVG_ARTIFACT_PRESET_IDS.contains(&preset_id) {
        let derived = derived_inputs
            .expect("slow SVG acceptance prepares the derived SVG before its catalog loop");
        let vector = &derived[1];
        return ArtifactInput {
            source_path: vector.derived_path.clone(),
            format: vector.format,
            source_dimensions: vector.dimensions,
            canvas_dimensions: (225.0, 155.0),
            manifest_note: format!(
                "derived 225x155 SVG source from `{}` (source_sha256={}, derived_sha256={}, method={}); document/output canvas is 225x155; preflight: {}; artifact-only density keeps AcrossX=16 under the base aspect lock, deriving AcrossY=11.0222; no recipe, default, or request-limit change.",
                vector.source_path,
                vector.source_sha256,
                vector.derived_sha256,
                vector.method,
                artifact_resolution_preflight_note(preset_id),
            ),
        };
    }
    let vector = matches!(preset_id, "triagrid-custom-shape-marks");
    if vector {
        ArtifactInput {
            source_path: PathBuf::from("../../assets/vector-sample.svg"),
            format: SourceFormatHint::Svg,
            source_dimensions: (900, 620),
            canvas_dimensions: (900.0, 620.0),
            manifest_note: format!(
                "immutable project-wide SVG source with matching intrinsic document canvas; preflight: {}; no recipe, default, or request-limit change.",
                artifact_resolution_preflight_note(preset_id)
            ),
        }
    } else {
        ArtifactInput {
            source_path: PathBuf::from("../../assets/raster-sample.png"),
            format: SourceFormatHint::Png,
            source_dimensions: (1024, 1024),
            canvas_dimensions: (1024.0, 1024.0),
            manifest_note: format!(
                "immutable project-wide raster source with matching intrinsic document canvas; preflight: {}; no recipe, default, or request-limit change.",
                artifact_resolution_preflight_note(preset_id)
            ),
        }
    }
}

/// Returns the existing connection inspection policy shared by acceptance
/// evaluations without changing any product default or catalog intent.
fn artifact_limits() -> EvaluationLimits {
    EvaluationLimits::default()
        .with_connection_path_limits(
            ConnectionPathLimits::new(1_048_576, 1_048_576, 2_097_152, 268_435_456)
                .expect("expanded test inspection policy validates"),
        )
        .expect("expanded test policy validates")
}

/// Waits for one complete-document scheduler result under a bounded wall-clock
/// guard without treating worker timing as part of canonical output identity.
fn wait_for_completion(scheduler: &EvaluationScheduler) -> EvaluationCompletion {
    let deadline = Instant::now() + Duration::from_secs(300);
    loop {
        if let Some(completion) = scheduler
            .try_receive_latest()
            .expect("artifact scheduler receives")
        {
            return completion;
        }
        assert!(Instant::now() < deadline, "artifact scheduler timed out");
        std::thread::yield_now();
    }
}

/// Submits the same authoritative snapshot twice, accepts both completions,
/// and proves the second publication reuses the scheduler cache for one card.
fn scheduler_cache_evidence(
    history: &DocumentHistory,
    source: ResolvedSource,
    limits: EvaluationLimits,
) -> String {
    let scheduler =
        EvaluationScheduler::new_with_limits(limits).expect("artifact scheduler builds");
    let first_ticket = scheduler
        .submit(EvaluationRequest::new(
            history.session().document_evaluation_snapshot(),
            source.clone(),
        ))
        .expect("artifact scheduler submits first evaluation");
    let first = wait_for_completion(&scheduler);
    assert_eq!(first.ticket(), first_ticket);
    assert!(
        scheduler
            .accept_completion(&first, history.session())
            .expect("first artifact completion accepts")
    );
    let first_cache = first
        .cache_diagnostics()
        .expect("first artifact completion has cache diagnostics")
        .aggregate;
    let replay_ticket = scheduler
        .submit(EvaluationRequest::new(
            history.session().document_evaluation_snapshot(),
            source,
        ))
        .expect("artifact scheduler submits replay");
    let replay = wait_for_completion(&scheduler);
    assert_eq!(replay.ticket(), replay_ticket);
    assert!(
        scheduler
            .accept_completion(&replay, history.session())
            .expect("replay artifact completion accepts")
    );
    let replay_cache = replay
        .cache_diagnostics()
        .expect("replay artifact completion has cache diagnostics")
        .aggregate;
    assert_eq!(replay_cache.decoded_source, CacheDisposition::Hit);
    assert_eq!(replay_cache.family, CacheDisposition::Hit);
    assert_eq!(replay_cache.realization, CacheDisposition::Hit);
    assert_eq!(replay_cache.scene, CacheDisposition::Hit);
    assert_eq!(replay_cache.raster, CacheDisposition::Hit);
    format!("first={first_cache:?}; replay={replay_cache:?}")
}

/// Returns a stable SHA-256 hex digest for an artifact or immutable input byte sequence.
fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Formats one projected control as bounded manifest text without serializing
/// arbitrary debug payloads or replacing the in-memory descriptor comparison.
fn compact_control_descriptor(descriptor: &PropertyDescriptor) -> String {
    format!(
        "field={:?}; target={:?}; kind={:?}; choices={:?}; bounds={:?}; unit={:?}; dependency={:?}; applicability={:?}; invalidation={:?}; copy_on_edit_escalates_to_family={}; structural_support={:?}; reference_constraint={:?}; choice_policy={:?}; authority={:?}; reset_capable={}",
        descriptor.field,
        descriptor.target,
        descriptor.value_kind,
        descriptor.choices,
        descriptor.bounds,
        descriptor.unit,
        descriptor.dependency,
        descriptor.applicability,
        descriptor.invalidation,
        descriptor.copy_on_edit_escalates_to_family,
        descriptor.structural_support,
        descriptor.reference_constraint,
        descriptor.choice_policy,
        descriptor.authority,
        descriptor.reset_capable,
    )
}

/// Joins the current projected controls into a bounded, reviewable manifest
/// summary while preserving each public descriptor's exact typed fields.
fn compact_active_controls(controls: &[PropertyDescriptor]) -> String {
    controls
        .iter()
        .map(compact_control_descriptor)
        .collect::<Vec<_>>()
        .join(" || ")
}

/// Returns the exact current Stage 20S top-level inventory, optionally including its manifest.
///
/// The inventory excludes recoverably quarantined historical directories and accepts no retired
/// catalog records, so both a full generator and manifest-only audit share one authority.
fn expected_stage20s_artifact_inventory(
    registry: &PresetRegistry,
    include_manifest: bool,
) -> BTreeSet<String> {
    let representatives = BTreeSet::from([
        "one-guide-lines",
        "triagrid-custom-shape-marks",
        "source-weighted-dispersion-voronoi",
        "two-guide-cells-uniform-offset",
        "round-spiral-marks",
        "three-guide-maze",
        "residual-sites-along-guide",
    ]);
    let mut expected = BTreeSet::new();
    if include_manifest {
        expected.insert("MANIFEST.md".to_owned());
    }
    for record in registry.entries() {
        let id = record.metadata.id.as_str();
        expected.insert(format!("{id}.preset.json"));
        expected.insert(format!("{id}.png"));
        expected.insert(format!("{id}.svg"));
        if representatives.contains(id) {
            expected.insert(format!("{id}-svg-raster.png"));
        }
    }
    for id in CURVED_GUIDE_FIXTURE_IDS {
        expected.insert(format!("{id}.png"));
        expected.insert(format!("{id}.svg"));
        expected.insert(format!("{id}-svg-raster.png"));
    }
    expected.insert("heterogeneous-rgb-recipes.png".to_owned());
    expected.insert("heterogeneous-rgb-recipes.svg".to_owned());
    expected.insert("heterogeneous-rgb-recipes-svg-raster.png".to_owned());
    expected.insert("derived-inputs".to_owned());
    expected
}

/// Verifies that a completed full-run directory contains exactly current
/// Stage 20S records, the required representative rasters, and its manifest.
fn assert_current_stage20s_artifact_inventory(output: &Path, registry: &PresetRegistry) {
    let expected = expected_stage20s_artifact_inventory(registry, true);
    let actual = fs::read_dir(output)
        .expect("artifact inventory directory remains readable")
        .map(|entry| {
            entry
                .expect("artifact inventory entry remains readable")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        actual, expected,
        "artifact inventory contains current records only"
    );
    let derived_expected = BTreeSet::from([
        "raster-sample-25pct.png".to_owned(),
        "vector-sample-25pct.svg".to_owned(),
    ]);
    let derived_actual = fs::read_dir(output.join("derived-inputs"))
        .expect("derived artifact inventory directory remains readable")
        .map(|entry| {
            entry
                .expect("derived artifact inventory entry remains readable")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        derived_actual, derived_expected,
        "derived artifact inventory contains only the two documented inputs"
    );
}

/// Summarizes raw premultiplied output channels without flattening alpha or
/// relying on a viewer background for artifact evidence.
fn rgba_statistics(width: u32, height: u32, pixels: &[u8]) -> String {
    let count = pixels.len() / 4;
    let mut sums = [0_u64; 4];
    let mut minima = [u8::MAX; 4];
    let mut maxima = [0_u8; 4];
    let mut nonzero_alpha = 0_usize;
    for pixel in pixels.chunks_exact(4) {
        for index in 0..4 {
            sums[index] += u64::from(pixel[index]);
            minima[index] = minima[index].min(pixel[index]);
            maxima[index] = maxima[index].max(pixel[index]);
        }
        nonzero_alpha += usize::from(pixel[3] != 0);
    }
    format!(
        "dimensions={width}x{height}; pixels={count}; rgb_mean={:.6},{:.6},{:.6}; alpha_mean={:.6}; rgb_min={},{},{}; rgb_max={},{},{}; alpha_min={}; alpha_max={}; alpha_nonzero={nonzero_alpha}",
        sums[0] as f64 / count as f64,
        sums[1] as f64 / count as f64,
        sums[2] as f64 / count as f64,
        sums[3] as f64 / count as f64,
        minima[0],
        minima[1],
        minima[2],
        maxima[0],
        maxima[1],
        maxima[2],
        minima[3],
        maxima[3],
    )
}

/// Decodes one generated PNG solely to report native RGBA statistics for the manifest.
fn png_statistics(path: &Path) -> String {
    let image = image::open(path).expect("generated PNG decodes").to_rgba8();
    rgba_statistics(image.width(), image.height(), image.as_raw())
}

/// Reads and validates the two pre-existing Stage 20S derived inputs without rewriting them.
///
/// The returned provenance records retain exactly the generator's deterministic derivation
/// methods. This audit performs hashes and dimension checks only; immutable inputs and derived
/// files remain untouched.
fn read_existing_stage20s_derived_inputs(output: &Path) -> [DerivedInputRecord; 2] {
    let derived = output.join("derived-inputs");
    let raster_source_path = Path::new("../../assets/raster-sample.png");
    let vector_source_path = Path::new("../../assets/vector-sample.svg");
    let raster_path = derived.join("raster-sample-25pct.png");
    let vector_path = derived.join("vector-sample-25pct.svg");
    assert_eq!(
        image::image_dimensions(&raster_path).expect("existing derived raster dimensions read"),
        (256, 256),
        "existing derived raster retains its documented dimensions"
    );
    let vector = fs::read_to_string(&vector_path).expect("existing derived SVG reads as UTF-8");
    assert!(
        vector.contains("width=\"225\" height=\"155\" viewBox=\"0 0 900 620\"")
            && vector.contains("<text"),
        "existing derived SVG retains its documented intrinsic size, viewBox, and live text"
    );
    [
        DerivedInputRecord {
            source_path: "assets/raster-sample.png",
            derived_path: raster_path.clone(),
            format: SourceFormatHint::Png,
            dimensions: (256, 256),
            source_sha256: sha256(&fs::read(raster_source_path).expect("immutable raster reads")),
            derived_sha256: sha256(&fs::read(raster_path).expect("existing derived raster reads")),
            method: "image::imageops::FilterType::Lanczos3 resize_exact(256,256)",
        },
        DerivedInputRecord {
            source_path: "assets/vector-sample.svg",
            derived_path: vector_path.clone(),
            format: SourceFormatHint::Svg,
            dimensions: (225, 155),
            source_sha256: sha256(&fs::read(vector_source_path).expect("immutable vector reads")),
            derived_sha256: sha256(&fs::read(vector_path).expect("existing derived SVG reads")),
            method: "root intrinsic width/height rewrite; preserved viewBox/content/live text",
        },
    ]
}

/// Returns a bounded digest record for an SVG metadata element without copying its identity text.
///
/// # Panics
///
/// Panics when a retained raw SVG is not UTF-8 or lacks the canonical metadata element required
/// by Stage 20S identity evidence.
fn svg_metadata_digest(svg: &[u8]) -> String {
    let text = std::str::from_utf8(svg).expect("raw SVG remains UTF-8");
    let start = text
        .find("<metadata>")
        .expect("raw SVG retains metadata start");
    let end = text[start..]
        .find("</metadata>")
        .map(|offset| start + offset + "</metadata>".len())
        .expect("raw SVG retains metadata end");
    let metadata = &svg[start..end];
    format!("sha256={}; bytes={}", sha256(metadata), metadata.len())
}

/// Summarizes one retained PNG/SVG evidence triplet using hashes and native alpha-aware statistics.
///
/// The summary reads existing files only and never rasterizes, evaluates, or recreates an artifact.
fn retained_artifact_summary(output: &Path, id: &str, svg_raster: bool) -> String {
    let png_path = output.join(format!("{id}.png"));
    let svg_path = output.join(format!("{id}.svg"));
    let png = fs::read(&png_path).expect("retained native PNG reads");
    let svg = fs::read(&svg_path).expect("retained raw SVG reads");
    let svg_raster_summary = if svg_raster {
        let path = output.join(format!("{id}-svg-raster.png"));
        format!(
            "svg_raster_sha256={}; svg_raster_rgba={}",
            sha256(&fs::read(&path).expect("retained SVG-raster PNG reads")),
            png_statistics(&path)
        )
    } else {
        "svg_raster=not-required".to_owned()
    };
    format!(
        "native_png_sha256={}; native_png_rgba={}; raw_svg_sha256={}; raw_svg_metadata={}; {svg_raster_summary}",
        sha256(&png),
        png_statistics(&png_path),
        sha256(&svg),
        svg_metadata_digest(&svg),
    )
}

/// Validates retained Stage 20S evidence and writes only a compact replacement manifest.
///
/// This mode intentionally does not evaluate, render, materialize, save presets, regenerate
/// derived inputs, or alter any artifact except `MANIFEST.md`. It records retained files as
/// earlier ordinary-generator output in accordance with the user's no-regeneration direction.
fn write_stage20s_manifest_from_existing(output: &Path, registry: &PresetRegistry) {
    assert_eq!(
        registry.version(),
        2,
        "current pre-release registry remains version two"
    );
    assert_eq!(
        registry.entries().len(),
        16,
        "current registry retains exactly sixteen cards"
    );
    assert!(
        registry.find("regions-plus-marks").is_none(),
        "retired temporary recipe is absent from the current registry"
    );
    let expected = expected_stage20s_artifact_inventory(registry, false);
    let actual = fs::read_dir(output)
        .expect("retained artifact directory remains readable")
        .map(|entry| {
            entry
                .expect("retained artifact inventory entry remains readable")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|name| name != "MANIFEST.md")
        .collect::<BTreeSet<_>>();
    assert_eq!(
        actual, expected,
        "retained evidence inventory excludes the retired card and contains no untracked files"
    );
    let derived_inputs = read_existing_stage20s_derived_inputs(output);
    let representatives = BTreeSet::from([
        "one-guide-lines",
        "triagrid-custom-shape-marks",
        "source-weighted-dispersion-voronoi",
        "two-guide-cells-uniform-offset",
        "round-spiral-marks",
        "three-guide-maze",
        "residual-sites-along-guide",
    ]);
    let mut records = Vec::new();
    for record in registry.entries() {
        let id = record.metadata.id.as_str();
        let preset_path = output.join(format!("{id}.preset.json"));
        let preset_bytes = fs::read(&preset_path).expect("retained preset reads");
        assert_eq!(
            load_preset(&preset_path).expect("retained preset-v3 loads"),
            *record,
            "retained preset matches the current registry exactly"
        );
        let preset_text =
            std::str::from_utf8(&preset_bytes).expect("retained preset remains UTF-8");
        let derived_data_absent = [
            "\"derived\"",
            "\"cache\"",
            "\"site_set\"",
            "\"selected_edges\"",
            "\"trails\"",
            "\"canonical_geometry\"",
        ]
        .iter()
        .all(|forbidden| !preset_text.contains(forbidden));
        assert!(
            derived_data_absent,
            "retained preset stores authored intent only"
        );
        let catalog_entry = registry
            .catalog_entries()
            .iter()
            .find(|entry| entry.preset.metadata.id == record.metadata.id)
            .expect("current record retains gallery-only requirement metadata");
        let input = artifact_input(id, Some(&derived_inputs));
        let source_bytes = fs::read(&input.source_path).expect("retained input source reads");
        records.push(format!(
            "## `{id}`\n- retained earlier ordinary-generator evidence; not regenerated in manifest-only mode\n- metadata: name=`{}`, category=`{}`, description=`{}`; required flags={:?} (gallery-only, nonserialized, nonidentity)\n- input/canvas: `{}` ({:?}, {}x{}, sha256={}); canvas={:.0}x{:.0}; policy: {}\n- preset-v3: sha256={}; exact current-registry comparison passed; derived-data absence={}\n- artifacts: {}\n",
            record.metadata.name,
            record.metadata.category,
            record.metadata.description,
            catalog_entry.required_features,
            input.source_path.display(),
            input.format,
            input.source_dimensions.0,
            input.source_dimensions.1,
            sha256(&source_bytes),
            input.canvas_dimensions.0,
            input.canvas_dimensions.1,
            input.manifest_note,
            sha256(&preset_bytes),
            derived_data_absent,
            retained_artifact_summary(output, id, representatives.contains(id)),
        ));
    }
    let derived_record = format!(
        "## Derived Inputs\n- `{}` -> `{}` ({:?} {}x{}, source_sha256={}, derived_sha256={}, method={})\n- `{}` -> `{}` ({:?} {}x{}, source_sha256={}, derived_sha256={}, method={})\n",
        derived_inputs[0].source_path,
        derived_inputs[0].derived_path.display(),
        derived_inputs[0].format,
        derived_inputs[0].dimensions.0,
        derived_inputs[0].dimensions.1,
        derived_inputs[0].source_sha256,
        derived_inputs[0].derived_sha256,
        derived_inputs[0].method,
        derived_inputs[1].source_path,
        derived_inputs[1].derived_path.display(),
        derived_inputs[1].format,
        derived_inputs[1].dimensions.0,
        derived_inputs[1].dimensions.1,
        derived_inputs[1].source_sha256,
        derived_inputs[1].derived_sha256,
        derived_inputs[1].method,
    );
    let curved_records = CURVED_GUIDE_FIXTURE_IDS
        .iter()
        .map(|id| {
            format!(
                "- `{id}`: {}\n",
                retained_artifact_summary(output, id, true)
            )
        })
        .collect::<String>();
    let heterogeneous_record = retained_artifact_summary(output, "heterogeneous-rgb-recipes", true);
    let manifest = format!(
        "# Stage 20S retained artifact manifest\n\nThis manifest-only audit validates retained files from the earlier ordinary generator phase and writes only `MANIFEST.md`; it does not evaluate, render, materialize, save, or regenerate any artifact. Registry v2 contains exactly 16 current IDs: {}. The temporary maze-debug `regions-plus-marks` card is retired without replacement; its prior files are expected to remain recoverable under `target/validation/stage20s-retired-regions-plus-marks-20260826/`, outside this current inventory.\n\nEarlier generator contract: each catalog card completed preset-v3 save/load equality, direct/reloaded PNG and raw-SVG parity, deterministic replay, and scheduler cache miss-to-hit assertions. This audit validates current preset bytes and retained artifact hashes but does not rerun that contract. All catalog evidence uses visible ordinary R/G/B channels with Red/Green/Blue mapping; seed-bearing recipes received deterministic distinct typed seeds per channel, and ArtworkWeighted placement used the matching component. The heterogeneous validation document retains red=three-guide-maze, green=round-spiral-marks, blue=source-weighted-dispersion-voronoi. Required flags remain gallery-only, nonserialized, and identity-neutral. SVG input contains live text, so Inkscape SVG-raster pixels remain subject to the installed-font caveat in `assets/README.md`; raw SVG and native PNG remain canonical retained files.\n\n{}\n## Curved Guide Demonstrations\nValidation-only documents; no preset JSON is retained.\n{}\n## heterogeneous-rgb-recipes\nValidation-only document; no preset JSON is retained. RGB mapping/seed policy is recorded above.\n- {}\n\n{}",
        registry
            .entries()
            .iter()
            .map(|record| record.metadata.id.as_str())
            .collect::<Vec<_>>()
            .join(", "),
        records.join("\n"),
        curved_records,
        heterogeneous_record,
        derived_record,
    );
    assert!(
        manifest.len() < 1_000_000,
        "retained manifest stays bounded for human review: {} bytes",
        manifest.len()
    );
    fs::write(output.join("MANIFEST.md"), manifest).expect("retained manifest writes");
}

/// Writes one validation-only document through normal evaluation, native PNG,
/// raw SVG, and Inkscape SVG rasterization, returning bounded manifest text.
fn write_validation_fixture_artifacts(
    id: &str,
    history: &DocumentHistory,
    source: ResolvedSource,
    output: &Path,
) -> String {
    let started = Instant::now();
    let result = evaluate(EvaluationRequest::new(
        history.session().document_evaluation_snapshot(),
        source,
    ))
    .expect("curved fixture evaluates for artifact writing");
    let elapsed = started.elapsed();
    let png_path = output.join(format!("{id}.png"));
    let svg_path = output.join(format!("{id}.svg"));
    let raster_path = output.join(format!("{id}-svg-raster.png"));
    let png = encode_png(result.raster()).expect("curved fixture native PNG encodes");
    let svg = write_svg(result.scene());
    fs::write(&png_path, &png).expect("curved fixture native PNG writes");
    fs::write(&svg_path, &svg).expect("curved fixture raw SVG writes");
    let status = Command::new("inkscape")
        .arg(&svg_path)
        .arg("--export-type=png")
        .arg(format!("--export-filename={}", raster_path.display()))
        .status()
        .expect("Inkscape is required for curved fixture SVG raster evidence");
    assert!(status.success(), "Inkscape rasterizes curved fixture SVG");
    let geometry = result
        .scene()
        .layers()
        .iter()
        .map(|layer| match layer.geometry() {
            GeometryOutput::CircularMarks(marks) => format!("circles:{}", marks.len()),
            GeometryOutput::CanonicalStrokes(strokes) => format!("strokes:{}", strokes.len()),
            GeometryOutput::CanonicalMarks(marks) => format!("marks:{}", marks.len()),
            GeometryOutput::CanonicalRegions(regions) => {
                format!("regions:{}", regions.regions().len())
            }
        })
        .collect::<Vec<_>>()
        .join(",");
    let mappings = history
        .document()
        .channel_topology()
        .expect("curved fixture RGB topology")
        .channels()
        .iter()
        .map(|channel| format!("{:?}", channel.mapping.component))
        .collect::<Vec<_>>()
        .join(",");
    let identities = result
        .channels()
        .iter()
        .map(|channel| {
            let family: String = channel.family_identity().into();
            let realization: String = channel.realization_identity().into();
            format!(
                "channel={:?}:family_sha256={}:realization_sha256={}",
                channel.channel_id(),
                sha256(family.as_bytes()),
                sha256(realization.as_bytes()),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "id={id}; elapsed_ms={}; mappings={mappings}; identities={identities}; scene_sha256={}; geometry={geometry}; png_sha256={}; svg_sha256={}; svg_raster_sha256={}; rgba={}",
        elapsed.as_millis(),
        sha256(result.scene().identity().scene_fingerprint().as_bytes()),
        sha256(&png),
        sha256(svg.as_bytes()),
        sha256(&fs::read(&raster_path).expect("curved fixture raster reads")),
        rgba_statistics(
            result.raster().width(),
            result.raster().height(),
            result.raster().pixels()
        ),
    )
}

/// Returns whether one stable validation-only fixture can be regenerated
/// without traversing the catalog or rewriting its aggregate MANIFEST.
fn is_selected_validation_fixture(id: &str) -> bool {
    id == "heterogeneous-rgb-recipes" || CURVED_GUIDE_FIXTURE_IDS.contains(&id)
}

/// Regenerates exactly one derived-SVG validation fixture's PNG, raw SVG, and
/// SVG-raster PNG without writing a catalog preset, MANIFEST, or inventory.
fn write_selected_validation_fixture_artifacts(
    id: &str,
    output: &Path,
    derived_inputs: &[DerivedInputRecord; 2],
) {
    assert!(
        is_selected_validation_fixture(id),
        "selected fixture ID is stable and validation-only"
    );
    let source_id = SourceReferenceId::new(format!("selected-validation-{id}"))
        .expect("selected validation source ID validates");
    let source = ResolvedSource::new(
        source_id.clone(),
        fs::read(&derived_inputs[1].derived_path).expect("selected derived SVG reads"),
        SourceFormatHint::Svg,
    )
    .expect("selected derived SVG resolves");
    if id == "heterogeneous-rgb-recipes" {
        let (mut history, channels, _) = heterogeneous_rgb_history(source_id, 225.0, 155.0);
        apply_artifact_resolution_density(&mut history, channels, id);
        let _ = write_validation_fixture_artifacts(id, &history, source, output);
        return;
    }
    let mut history = curved_guide_history_for_variant(
        id,
        source_id,
        CanvasSpec {
            width: 225.0,
            height: 155.0,
        },
    );
    apply_artifact_resolution_density(&mut history, [ChannelId(1), ChannelId(2), ChannelId(3)], id);
    let _ = write_validation_fixture_artifacts(id, &history, source, output);
}

/// Evaluates one complete modeled document through the ordinary canonical
/// engine boundary and retains PNG/SVG bytes plus public per-channel identity.
fn canonical_output(history: &DocumentHistory, source: ResolvedSource) -> CanonicalOutput {
    let result = evaluate(EvaluationRequest::new(
        history.session().document_evaluation_snapshot(),
        source,
    ))
    .unwrap();
    CanonicalOutput {
        png: encode_png(result.raster()).unwrap(),
        svg: write_svg(result.scene()),
        channels: result
            .channels()
            .iter()
            .map(|channel| ChannelCanonicalIdentity {
                channel_id: channel.channel_id(),
                family: channel.family_identity().into(),
                realization: channel.realization_identity().into(),
            })
            .collect(),
    }
}

/// Proves the round-spiral line samples a nonuniform red source into variable
/// positive curve thickness through the ordinary canonical engine pipeline.
#[test]
fn round_spiral_line_samples_nonuniform_source_as_variable_curve_thickness() {
    let source_id = SourceReferenceId::new("spiral-thickness-rgb").expect("source ID validates");
    let pixels = image::RgbaImage::from_fn(32, 32, |x, _| {
        image::Rgba([((x * 255) / 31) as u8, 17, 31, 255])
    });
    let mut source_bytes = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(pixels)
        .write_to(&mut source_bytes, image::ImageFormat::Png)
        .expect("in-memory RGB source encodes");
    let source = ResolvedSource::new(
        source_id.clone(),
        source_bytes.into_inner(),
        SourceFormatHint::Png,
    )
    .expect("in-memory RGB source resolves");
    let (mut history, channels, _) = artifact_history(source_id, 32.0, 32.0);
    let registry = PresetRegistry::bundled();
    registry
        .apply_to_selected(&mut history, channels[0], "round-spiral-line")
        .expect("round spiral line materializes into red");
    for channel_id in channels.into_iter().skip(1) {
        history
            .apply(&DocumentCommand::SetVisibility {
                channel_id,
                visible: false,
            })
            .expect("non-target RGB channel hides through an ordinary command");
    }
    let result = evaluate(EvaluationRequest::new(
        history.session().document_evaluation_snapshot(),
        source,
    ))
    .expect("round spiral line evaluates");
    let layer = result
        .scene()
        .layers()
        .iter()
        .find(|layer| layer.channel_id() == ChannelId(1))
        .expect("red spiral layer remains visible");
    let GeometryOutput::CanonicalStrokes(strokes) = layer.geometry() else {
        panic!("round spiral line remains a raw parametric structural path");
    };
    assert_eq!(strokes.len(), 1, "round spiral line publishes one raw path");
    let thicknesses = strokes[0]
        .profile
        .iter()
        .map(|sample| sample.normalized_thickness)
        .filter(|thickness| *thickness > 0.0)
        .collect::<Vec<_>>();
    assert!(
        thicknesses
            .iter()
            .all(|thickness| (0.15..=0.65).contains(thickness))
    );
    let minimum = thicknesses.iter().copied().fold(f64::INFINITY, f64::min);
    let maximum = thicknesses
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(
        minimum < maximum,
        "nonuniform source produces at least two curve thicknesses"
    );
}

/// Proves source-weighted Voronoi materializes independent RGB mappings and
/// source-driven canonical identities without falling back to shared luminance.
#[test]
fn source_weighted_voronoi_uses_independent_rgb_source_mappings() {
    let source_id = SourceReferenceId::new("weighted-voronoi-rgb").expect("source ID validates");
    let pixels = image::RgbaImage::from_fn(32, 32, |x, y| {
        image::Rgba([
            ((x * 255) / 31) as u8,
            ((y * 255) / 31) as u8,
            (((x ^ y) * 255) / 31) as u8,
            255,
        ])
    });
    let mut source_bytes = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(pixels)
        .write_to(&mut source_bytes, image::ImageFormat::Png)
        .expect("in-memory RGB source encodes");
    let source = ResolvedSource::new(
        source_id.clone(),
        source_bytes.into_inner(),
        SourceFormatHint::Png,
    )
    .expect("in-memory RGB source resolves");
    let (mut history, channels, _) = artifact_history(source_id, 32.0, 32.0);
    let registry = PresetRegistry::bundled();
    apply_recipe_to_rgb(
        &registry,
        &mut history,
        channels,
        "source-weighted-dispersion-voronoi",
    );
    let seed_edits = channels.map(|channel| apply_distinct_channel_seeds(&mut history, channel));
    assert!(
        seed_edits
            .iter()
            .all(|edits| { edits.len() == 1 && edits[0].starts_with("random mechanism ") }),
        "source-weighted Voronoi owns one typed random-mechanism seed per RGB channel"
    );
    assert_distinct_rgb_seed_edits(&seed_edits);
    for (channel_id, component) in channels.into_iter().zip([
        SourceMappingComponent::Red,
        SourceMappingComponent::Green,
        SourceMappingComponent::Blue,
    ]) {
        let channel = history
            .document()
            .channel_topology()
            .expect("RGB topology remains authoritative")
            .channels()
            .iter()
            .find(|channel| channel.id == channel_id)
            .expect("materialized RGB channel remains present");
        assert!(channel.visible);
        assert_eq!(channel.mapping.component, component);
        assert!(
            history
                .document()
                .pattern_definition_for(channel_id)
                .expect("source-weighted definition remains present")
                .mechanisms
                .iter()
                .filter_map(|mechanism| match mechanism {
                    PatternMechanism::SiteDensityModulation {
                        modulation:
                            toniator_domain::SiteDensityModulation::ArtworkWeighted { mapping, .. },
                        ..
                    } => Some(mapping.component),
                    _ => None,
                })
                .all(|weighted_component| weighted_component == component)
        );
    }
    let result = evaluate(EvaluationRequest::new(
        history.session().document_evaluation_snapshot(),
        source,
    ))
    .expect("source-weighted RGB Voronoi evaluates under ordinary limits");
    assert_eq!(result.scene().layers().len(), 3);
    assert!(result.scene().layers().iter().all(|layer| layer.visible()));
    let identities = result
        .channels()
        .iter()
        .map(|channel| (channel.family_identity(), channel.realization_identity()))
        .collect::<Vec<_>>();
    assert_ne!(identities[0], identities[1]);
    assert_ne!(identities[0], identities[2]);
    assert_ne!(identities[1], identities[2]);
}

/// Evaluates the cubic TransformStack path fixture through three visible RGB
/// layers and proves repetition publishes more than one canonical stroke.
#[test]
fn curved_one_stack_paths_evaluates_three_visible_repeated_stroke_layers() {
    let source_id = SourceReferenceId::new("curved-stack-rgb").expect("source ID validates");
    let pixels = image::RgbaImage::from_fn(32, 24, |x, y| {
        image::Rgba([((x * 255) / 31) as u8, ((y * 255) / 23) as u8, 128, 255])
    });
    let mut bytes = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(pixels)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .expect("cubic fixture source encodes");
    let source = ResolvedSource::new(source_id.clone(), bytes.into_inner(), SourceFormatHint::Png)
        .expect("cubic fixture source resolves");
    let history = curved_one_stack_history(source_id, true, false);
    let result = evaluate(EvaluationRequest::new(
        history.session().document_evaluation_snapshot(),
        source,
    ))
    .expect("cubic TransformStack fixture evaluates");
    assert_eq!(result.scene().layers().len(), 3);
    assert_eq!(
        history
            .document()
            .channel_topology()
            .expect("cubic RGB topology remains available")
            .channels()
            .iter()
            .map(|channel| channel.mapping.component)
            .collect::<Vec<_>>(),
        vec![
            SourceMappingComponent::Red,
            SourceMappingComponent::Green,
            SourceMappingComponent::Blue,
        ]
    );
    for layer in result.scene().layers() {
        assert!(layer.visible());
        let GeometryOutput::CanonicalStrokes(strokes) = layer.geometry() else {
            panic!("cubic path fixture publishes strokes");
        };
        assert!(
            strokes.len() > 1,
            "TransformStack repeats the cubic baseline"
        );
    }
}

/// Evaluates the same authored cubic TransformStack as AlongGuides circle
/// marks and proves every visible RGB channel publishes repeated marks.
#[test]
fn curved_one_stack_sites_evaluates_three_visible_repeated_mark_layers() {
    let source_id = SourceReferenceId::new("curved-stack-sites-rgb").expect("source ID validates");
    let pixels = image::RgbaImage::from_fn(32, 24, |x, y| {
        image::Rgba([((x * 255) / 31) as u8, ((y * 255) / 23) as u8, 128, 255])
    });
    let mut bytes = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(pixels)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .expect("cubic mark fixture source encodes");
    let source = ResolvedSource::new(source_id.clone(), bytes.into_inner(), SourceFormatHint::Png)
        .expect("cubic mark fixture source resolves");
    let history = curved_one_stack_history(source_id, false, false);
    let result = evaluate(EvaluationRequest::new(
        history.session().document_evaluation_snapshot(),
        source,
    ))
    .expect("cubic TransformStack mark fixture evaluates");
    assert_eq!(result.scene().layers().len(), 3);
    for layer in result.scene().layers() {
        assert!(layer.visible());
        let GeometryOutput::CanonicalMarks(marks) = layer.geometry() else {
            panic!("cubic AlongGuides fixture publishes circle marks");
        };
        assert!(
            marks.len() > 1,
            "TransformStack publishes repeated AlongGuides marks"
        );
    }
    assert_eq!(
        history
            .document()
            .channel_topology()
            .expect("cubic RGB topology")
            .channels()
            .iter()
            .map(|channel| channel.mapping.component)
            .collect::<Vec<_>>(),
        vec![
            SourceMappingComponent::Red,
            SourceMappingComponent::Green,
            SourceMappingComponent::Blue
        ]
    );
}

/// Evaluates the cubic NormalOffset path fixture and retains its authored
/// constant positive twelve-unit centerline spacing in the typed definition.
#[test]
fn curved_one_normal_offset_paths_evaluates_repeated_stroke_layers() {
    let source_id = SourceReferenceId::new("curved-offset-rgb").expect("source ID validates");
    let pixels = image::RgbaImage::from_pixel(32, 24, image::Rgba([96, 128, 160, 255]));
    let mut bytes = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(pixels)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .expect("offset fixture source encodes");
    let source = ResolvedSource::new(source_id.clone(), bytes.into_inner(), SourceFormatHint::Png)
        .expect("offset fixture source resolves");
    let history = curved_one_stack_history(source_id, true, true);
    let definition = history
        .document()
        .pattern_definition_for(ChannelId(1))
        .expect("offset definition remains current");
    let PatternMechanism::GuideDimensions { dimensions, .. } = &definition.mechanisms[0] else {
        panic!("offset fixture retains typed guide dimensions");
    };
    assert!(matches!(
        dimensions[0].repetition,
        GuideRepetition::NormalOffset {
            spacing: 12.0,
            sides: OffsetSides::Both,
            cleanup: OffsetCleanup::DissolveCrossings,
        }
    ));
    let result = evaluate(EvaluationRequest::new(
        history.session().document_evaluation_snapshot(),
        source,
    ))
    .expect("cubic NormalOffset path fixture evaluates");
    for layer in result.scene().layers() {
        let GeometryOutput::CanonicalStrokes(strokes) = layer.geometry() else {
            panic!("NormalOffset fixture publishes strokes");
        };
        assert!(
            strokes.len() > 1,
            "positive centerline offsets retain multiple strokes"
        );
    }
}

/// Evaluates the shallow cubic NormalOffset fixture as AlongGuides circle
/// marks while retaining the typed positive twelve-unit centerline spacing.
#[test]
fn curved_one_normal_offset_sites_evaluates_repeated_mark_layers() {
    let source_id = SourceReferenceId::new("curved-offset-sites-rgb").expect("source ID validates");
    let pixels = image::RgbaImage::from_pixel(32, 24, image::Rgba([96, 128, 160, 255]));
    let mut bytes = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(pixels)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .expect("offset mark fixture source encodes");
    let source = ResolvedSource::new(source_id.clone(), bytes.into_inner(), SourceFormatHint::Png)
        .expect("offset mark fixture source resolves");
    let history = curved_one_stack_history(source_id, false, true);
    let definition = history
        .document()
        .pattern_definition_for(ChannelId(1))
        .expect("offset mark definition remains current");
    let PatternMechanism::GuideDimensions { dimensions, .. } = &definition.mechanisms[0] else {
        panic!("offset mark fixture retains typed guide dimensions");
    };
    assert!(matches!(
        dimensions[0].repetition,
        GuideRepetition::NormalOffset {
            spacing: 12.0,
            sides: OffsetSides::Both,
            cleanup: OffsetCleanup::DissolveCrossings,
        }
    ));
    let result = evaluate(EvaluationRequest::new(
        history.session().document_evaluation_snapshot(),
        source,
    ))
    .expect("cubic NormalOffset mark fixture evaluates");
    assert_eq!(result.scene().layers().len(), 3);
    for layer in result.scene().layers() {
        assert!(layer.visible());
        let GeometryOutput::CanonicalMarks(marks) = layer.geometry() else {
            panic!("NormalOffset AlongGuides fixture publishes marks");
        };
        assert!(
            marks.len() > 1,
            "positive centerline offsets retain repeated marks"
        );
    }
}

/// Evaluates both authored cubic TransformStacks as repeated GuidePaths and
/// proves canonical stroke provenance retains each distinct source structure.
#[test]
fn curved_two_stack_paths_evaluates_both_cubic_structure_provenances() {
    let source_id = SourceReferenceId::new("curved-two-stack-rgb").expect("source ID validates");
    let pixels = image::RgbaImage::from_pixel(32, 24, image::Rgba([96, 128, 160, 255]));
    let mut bytes = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(pixels)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .expect("two-cubic source encodes");
    let source = ResolvedSource::new(source_id.clone(), bytes.into_inner(), SourceFormatHint::Png)
        .expect("two-cubic source resolves");
    let history = curved_two_stack_history(source_id, true);
    let definition = history
        .document()
        .pattern_definition_for(ChannelId(1))
        .expect("two-stack definition");
    let PatternMechanism::GuideDimensions { dimensions, .. } = &definition.mechanisms[0] else {
        panic!("two-stack fixture retains guide dimensions");
    };
    assert!(matches!(
        dimensions[0].repetition,
        GuideRepetition::TransformStack {
            direction_degrees: 90.0,
            ..
        }
    ));
    assert!(matches!(
        dimensions[1].repetition,
        GuideRepetition::TransformStack {
            direction_degrees: 90.0,
            ..
        }
    ));
    let result = evaluate(EvaluationRequest::new(
        history.session().document_evaluation_snapshot(),
        source,
    ))
    .expect("two-cubic TransformStack fixture evaluates");
    for layer in result.scene().layers() {
        let GeometryOutput::CanonicalStrokes(strokes) = layer.geometry() else {
            panic!("two-stack fixture publishes strokes");
        };
        assert!(strokes.len() > 2);
        let sources = strokes
            .iter()
            .filter_map(|stroke| stroke.source_structure_id)
            .collect::<Vec<_>>();
        assert!(sources.contains(&AuthoredStructureId(11)));
        assert!(sources.contains(&AuthoredStructureId(12)));
        assert!(strokes.iter().all(|stroke| matches!(
            stroke.path.segments()[0],
            toniator_patterns::CurveSegment::CubicBezier(_)
        )));
    }
}

/// Evaluates the orthogonal cubic TransformStacks through intersections as RGB
/// marks that occupy every document quadrant without changing guide authority.
#[test]
fn curved_two_stack_intersections_evaluates_repeated_mark_layers() {
    let source_id =
        SourceReferenceId::new("curved-two-intersections-rgb").expect("source ID validates");
    let canvas = CanvasSpec {
        width: 225.0,
        height: 155.0,
    };
    let pixels = image::RgbaImage::from_pixel(225, 155, image::Rgba([96, 128, 160, 255]));
    let mut bytes = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(pixels)
        .write_to(&mut bytes, image::ImageFormat::Png)
        .expect("two-cubic mark source encodes");
    let source = ResolvedSource::new(source_id.clone(), bytes.into_inner(), SourceFormatHint::Png)
        .expect("two-cubic mark source resolves");
    let mut history = curved_guide_history_for_variant(
        "curved-two-stack-intersections",
        source_id,
        canvas.clone(),
    );
    apply_artifact_resolution_density(
        &mut history,
        [ChannelId(1), ChannelId(2), ChannelId(3)],
        "curved-two-stack-intersections",
    );
    let definition = history
        .document()
        .pattern_definition_for(ChannelId(1))
        .expect("two-stack mark definition");
    assert!(matches!(
        definition.mechanisms[1],
        PatternMechanism::SelectedGuideIntersections { .. }
    ));
    let result = evaluate(EvaluationRequest::new(
        history.session().document_evaluation_snapshot(),
        source,
    ))
    .expect("two-cubic intersection mark fixture evaluates");
    assert_eq!(result.scene().layers().len(), 3);
    for layer in result.scene().layers() {
        assert!(layer.visible());
        let GeometryOutput::CanonicalMarks(marks) = layer.geometry() else {
            panic!("two-stack intersections publish marks");
        };
        assert!(
            marks.len() > 1,
            "two cubic contributors yield repeated intersections"
        );
        let centers = marks
            .iter()
            .map(|mark| match mark {
                toniator_patterns::CanonicalMark::Circle { center, .. } => *center,
                toniator_patterns::CanonicalMark::ClosedPath(path) => {
                    toniator_patterns::Point2::new(
                        (path.bounds.min.x + path.bounds.max.x) * 0.5,
                        (path.bounds.min.y + path.bounds.max.y) * 0.5,
                    )
                }
            })
            .collect::<Vec<_>>();
        assert!(centers.iter().any(|center| center.x < canvas.width * 0.25));
        assert!(centers.iter().any(|center| center.x > canvas.width * 0.75));
        assert!(centers.iter().any(|center| center.y < canvas.height * 0.25));
        assert!(centers.iter().any(|center| center.y > canvas.height * 0.75));
        for (left, top) in [(true, true), (true, false), (false, true), (false, false)] {
            assert!(centers.iter().any(|center| {
                (if left {
                    center.x < canvas.width * 0.5
                } else {
                    center.x >= canvas.width * 0.5
                }) && (if top {
                    center.y < canvas.height * 0.5
                } else {
                    center.y >= canvas.height * 0.5
                })
            }));
        }
    }
}

/// Renders all six validation-only curved-guide fixtures in memory, proving
/// deterministic PNG/SVG replay and bounded small-fixture evaluation time.
#[test]
fn curved_guide_fixture_matrix_replays_png_and_svg_within_thirty_seconds() {
    let source_id = SourceReferenceId::new("curved-matrix-rgb").expect("source ID validates");
    let pixels = image::RgbaImage::from_fn(32, 24, |x, y| {
        image::Rgba([
            ((x * 255) / 31) as u8,
            ((y * 255) / 23) as u8,
            ((x ^ y) * 10) as u8,
            255,
        ])
    });
    let mut encoded = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(pixels)
        .write_to(&mut encoded, image::ImageFormat::Png)
        .expect("curved matrix source encodes");
    let bytes = encoded.into_inner();
    for id in CURVED_GUIDE_FIXTURE_IDS {
        let paths = id.ends_with("paths");
        let history = match id {
            "curved-one-stack-paths" => curved_one_stack_history(source_id.clone(), true, false),
            "curved-one-stack-sites" => curved_one_stack_history(source_id.clone(), false, false),
            "curved-one-normal-offset-paths" => {
                curved_one_stack_history(source_id.clone(), true, true)
            }
            "curved-one-normal-offset-sites" => {
                curved_one_stack_history(source_id.clone(), false, true)
            }
            "curved-two-stack-paths" => curved_two_stack_history(source_id.clone(), true),
            "curved-two-stack-intersections" => curved_two_stack_history(source_id.clone(), false),
            _ => unreachable!("fixture matrix has stable IDs"),
        };
        let start = Instant::now();
        let source = ResolvedSource::new(source_id.clone(), bytes.clone(), SourceFormatHint::Png)
            .expect("curved matrix source resolves");
        let first = evaluate(EvaluationRequest::new(
            history.session().document_evaluation_snapshot(),
            source,
        ))
        .expect("curved matrix fixture evaluates");
        let elapsed = start.elapsed();
        assert!(
            elapsed < Duration::from_secs(30),
            "{id} elapsed={elapsed:?}"
        );
        let png = encode_png(first.raster()).expect("curved matrix PNG encodes");
        let svg = write_svg(first.scene());
        assert!(
            png.starts_with(b"\x89PNG\r\n\x1a\n"),
            "{id} emits native PNG"
        );
        for channel in 1..=3 {
            assert!(svg.contains(&format!("id=\"channel-{channel}\"")));
        }
        for layer in first.scene().layers() {
            assert!(matches!(
                (paths, layer.geometry()),
                (true, GeometryOutput::CanonicalStrokes(_))
                    | (false, GeometryOutput::CanonicalMarks(_))
            ));
        }
        let replay = evaluate(EvaluationRequest::new(
            history.session().document_evaluation_snapshot(),
            ResolvedSource::new(source_id.clone(), bytes.clone(), SourceFormatHint::Png)
                .expect("replay source resolves"),
        ))
        .expect("curved matrix replay evaluates");
        assert_eq!(
            png,
            encode_png(replay.raster()).expect("replay PNG encodes")
        );
        assert_eq!(svg, write_svg(replay.scene()));
    }
}

/// Writes derived inputs only to a process-temporary directory and verifies
/// their exact provenance plus spiral source and canvas selection policy.
#[test]
fn derived_stage20s_inputs_preserve_immutable_provenance() {
    let output =
        std::env::temp_dir().join(format!("toniator-stage20s-derived-{}", std::process::id()));
    fs::create_dir_all(&output).expect("temporary derived output creates");
    let records = write_stage20s_derived_inputs(&output);
    assert_eq!(records[0].dimensions, (256, 256));
    assert_eq!(records[1].dimensions, (225, 155));
    assert!(records.iter().all(|record| record.derived_path.is_file()));
    for preset_id in DERIVED_RASTER_ARTIFACT_PRESET_IDS {
        let input = artifact_input(preset_id, Some(&records));
        assert_eq!(input.source_path, records[0].derived_path);
        assert!(matches!(input.format, SourceFormatHint::Png));
        assert_eq!(input.source_dimensions, (256, 256));
        assert_eq!(input.canvas_dimensions, (256.0, 256.0));
        assert!(input.manifest_note.contains("derived_sha256"));
        let expected_density = if preset_id == "one-guide-lines" {
            "4x4"
        } else {
            "16x16"
        };
        assert!(input.manifest_note.contains(expected_density));
    }
    for preset_id in DERIVED_SVG_ARTIFACT_PRESET_IDS {
        let input = artifact_input(preset_id, Some(&records));
        assert_eq!(input.source_path, records[1].derived_path);
        assert!(matches!(input.format, SourceFormatHint::Svg));
        assert_eq!(input.source_dimensions, (225, 155));
        assert_eq!(input.canvas_dimensions, (225.0, 155.0));
        assert!(input.manifest_note.contains("derived_sha256"));
        assert!(input.manifest_note.contains("AcrossY=11.0222"));
    }
    for preset_id in CANVAS_COVERING_SPIRAL_PRESET_IDS {
        let input = artifact_input(preset_id, Some(&records));
        assert!(input.manifest_note.contains("0.72s"));
        assert!(
            input
                .manifest_note
                .contains("realization.stroke.profile_limit")
        );
        assert!(input.manifest_note.contains("40s external timeout"));
        assert!(
            input
                .manifest_note
                .contains("CoverCanvas materializes against that canvas")
        );
    }
    for preset_id in [
        "even-random-circles",
        "triagrid-custom-shape-marks",
        "straight-grid-circles",
    ] {
        let input = artifact_input(preset_id, Some(&records));
        assert!(input.manifest_note.contains("intrinsic"));
        assert_ne!(input.canvas_dimensions, (256.0, 256.0));
        assert_ne!(input.canvas_dimensions, (225.0, 155.0));
    }
    fs::remove_dir_all(&output).expect("process-temporary derived output cleans");
}

/// Proves selected fixture regeneration accepts only the six curved documents
/// and heterogeneous RGB demonstration, never a catalog artifact ID.
#[test]
fn selected_validation_fixture_ids_exclude_catalog_artifacts() {
    for id in CURVED_GUIDE_FIXTURE_IDS {
        assert!(is_selected_validation_fixture(id));
    }
    assert!(is_selected_validation_fixture("heterogeneous-rgb-recipes"));
    assert!(!is_selected_validation_fixture("round-spiral-marks"));
    assert!(!is_selected_validation_fixture("grid-voronoi-scale"));
}

/// Proves artifact-only density edits retain the base aspect lock and report
/// the domain-derived companion axis for square, quarter-SVG, and one-guide inputs.
#[test]
fn artifact_resolution_density_preserves_authoritative_aspect_lock() {
    let registry = PresetRegistry::bundled();
    let source_id = SourceReferenceId::new("artifact-resolution-density")
        .expect("artifact density source ID validates");
    let (mut raster_history, raster_channels, _) =
        artifact_history(source_id.clone(), 256.0, 256.0);
    apply_recipe_to_rgb(
        &registry,
        &mut raster_history,
        raster_channels,
        "grid-voronoi-scale",
    );
    assert!(
        apply_artifact_resolution_density(
            &mut raster_history,
            raster_channels,
            "grid-voronoi-scale"
        )
        .contains("R=16x16")
    );
    let (mut vector_history, vector_channels, _) =
        artifact_history(source_id.clone(), 225.0, 155.0);
    apply_recipe_to_rgb(
        &registry,
        &mut vector_history,
        vector_channels,
        "three-guide-maze",
    );
    assert!(
        apply_artifact_resolution_density(&mut vector_history, vector_channels, "three-guide-maze")
            .contains("R=16x11.0222")
    );
    let (mut one_guide_history, one_guide_channels, _) =
        artifact_history(source_id, 1024.0, 1024.0);
    apply_recipe_to_rgb(
        &registry,
        &mut one_guide_history,
        one_guide_channels,
        "one-guide-lines",
    );
    assert!(
        apply_artifact_resolution_density(
            &mut one_guide_history,
            one_guide_channels,
            "one-guide-lines"
        )
        .contains("R=4x4")
    );
    let heterogeneous_source_id = SourceReferenceId::new("heterogeneous-density")
        .expect("heterogeneous density source ID validates");
    let (mut heterogeneous_history, heterogeneous_channels, _) =
        heterogeneous_rgb_history(heterogeneous_source_id, 225.0, 155.0);
    assert!(
        apply_artifact_resolution_density(
            &mut heterogeneous_history,
            heterogeneous_channels,
            "heterogeneous-rgb-recipes"
        )
        .contains("R=16x11.0222")
    );
    for (history, channels, expected_axes) in [
        (&raster_history, raster_channels, (16.0, 16.0)),
        (
            &vector_history,
            vector_channels,
            (16.0, 16.0 * 155.0 / 225.0),
        ),
        (&one_guide_history, one_guide_channels, (4.0, 4.0)),
        (
            &heterogeneous_history,
            heterogeneous_channels,
            (16.0, 16.0 * 155.0 / 225.0),
        ),
    ] {
        for channel_id in channels {
            let density = history
                .document()
                .effective_channel_pattern(channel_id)
                .expect("artifact channel retains effective density")
                .density;
            assert!(density.aspect_locked);
            assert_eq!(density.across_x, expected_axes.0);
            assert!((density.across_y - expected_axes.1).abs() <= 1e-12);
        }
    }
}

/// Proves every curved-guide evidence history retains its typed variant and
/// RGB authority at the reduced derived-SVG canvas with aspect-locked density.
#[test]
fn curved_guide_derived_svg_evidence_histories_preserve_variants_and_rgb() {
    let canvas = CanvasSpec {
        width: 225.0,
        height: 155.0,
    };
    for id in CURVED_GUIDE_FIXTURE_IDS {
        let source_id = SourceReferenceId::new(format!("derived-curved-{id}"))
            .expect("derived curved fixture source ID validates");
        let mut history = curved_guide_history_for_variant(id, source_id, canvas.clone());
        let channels = [ChannelId(1), ChannelId(2), ChannelId(3)];
        let density_audit = apply_artifact_resolution_density(&mut history, channels, id);
        assert!(density_audit.contains("R=16x11.0222"));
        for (channel_id, component) in channels.into_iter().zip([
            SourceMappingComponent::Red,
            SourceMappingComponent::Green,
            SourceMappingComponent::Blue,
        ]) {
            let topology_channel = history
                .document()
                .channel_topology()
                .expect("derived curved fixture retains RGB topology")
                .channels()
                .iter()
                .find(|channel| channel.id == channel_id)
                .expect("derived curved fixture retains RGB channel");
            assert!(topology_channel.visible);
            assert_eq!(topology_channel.mapping.component, component);
            let definition = history
                .document()
                .pattern_definition_for(channel_id)
                .expect("derived curved fixture retains its definition");
            if id.ends_with("paths") {
                assert!(matches!(
                    definition.output_layers[0].realization,
                    PatternOutputRealization::GuidePaths { .. }
                ));
            } else {
                assert!(matches!(
                    definition.output_layers[0].realization,
                    PatternOutputRealization::MarkPrototype { .. }
                ));
            }
            if id.contains("normal-offset") {
                assert!(definition.mechanisms.iter().any(|mechanism| matches!(
                    mechanism,
                    PatternMechanism::GuideDimensions { dimensions, .. }
                        if matches!(dimensions[0].repetition, GuideRepetition::NormalOffset { spacing, sides: OffsetSides::Both, cleanup: OffsetCleanup::DissolveCrossings } if spacing == 77.5)
                )));
            }
            let density = history
                .document()
                .effective_channel_pattern(channel_id)
                .expect("derived curved fixture retains effective density")
                .density;
            assert!(density.aspect_locked);
            assert_eq!(density.across_x, 16.0);
            assert!((density.across_y - 16.0 * 155.0 / 225.0).abs() <= 1e-12);
        }
    }
}

/// Proves centered-local authored curved prototypes reach both document edges
/// after generic placement and contribute visible RGB ink in the left canvas band.
#[test]
fn curved_validation_path_prototypes_span_document_edges_after_placement() {
    let canvas = CanvasSpec {
        width: 225.0,
        height: 155.0,
    };
    let source_id = SourceReferenceId::new("curved-placement-envelope")
        .expect("curved placement source ID validates");
    let pixels = image::RgbaImage::from_fn(225, 155, |x, y| {
        image::Rgba([x as u8, y as u8, 255_u8.saturating_sub(x as u8), 255])
    });
    let mut encoded = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(pixels)
        .write_to(&mut encoded, image::ImageFormat::Png)
        .expect("curved placement source encodes");
    let bytes = encoded.into_inner();
    let mut one_stack = curved_guide_history_for_variant(
        "curved-one-stack-paths",
        source_id.clone(),
        canvas.clone(),
    );
    let channels = [ChannelId(1), ChannelId(2), ChannelId(3)];
    apply_artifact_resolution_density(&mut one_stack, channels, "curved-one-stack-paths");
    let result = evaluate(EvaluationRequest::new(
        one_stack.session().document_evaluation_snapshot(),
        ResolvedSource::new(source_id.clone(), bytes.clone(), SourceFormatHint::Png)
            .expect("curved placement source resolves"),
    ))
    .expect("centered-local one-stack paths evaluate");
    let left_band_has_visible_ink =
        result
            .raster()
            .pixels()
            .chunks_exact(4)
            .enumerate()
            .any(|(index, pixel)| {
                index % (result.raster().width() as usize) < 4
                    && pixel[3] > 0
                    && (pixel[0] > 0 || pixel[1] > 0 || pixel[2] > 0)
            });
    assert!(left_band_has_visible_ink);
    for layer in result.scene().layers() {
        let GeometryOutput::CanonicalStrokes(strokes) = layer.geometry() else {
            panic!("one-stack envelope fixture publishes canonical strokes");
        };
        let min_x = strokes
            .iter()
            .map(|stroke| {
                stroke
                    .path
                    .bounds()
                    .expect("stroke bounds are finite")
                    .min
                    .x
            })
            .fold(f64::INFINITY, f64::min);
        let max_x = strokes
            .iter()
            .map(|stroke| {
                stroke
                    .path
                    .bounds()
                    .expect("stroke bounds are finite")
                    .max
                    .x
            })
            .fold(f64::NEG_INFINITY, f64::max);
        assert!(
            min_x <= 0.0,
            "raw path envelope reaches left boundary: {min_x}"
        );
        assert!(
            max_x >= canvas.width,
            "raw path envelope reaches right boundary: {max_x}"
        );
    }
    let mut two_stack = curved_guide_history_for_variant(
        "curved-two-stack-paths",
        source_id.clone(),
        canvas.clone(),
    );
    apply_artifact_resolution_density(&mut two_stack, channels, "curved-two-stack-paths");
    let two_result = evaluate(EvaluationRequest::new(
        two_stack.session().document_evaluation_snapshot(),
        ResolvedSource::new(source_id, bytes, SourceFormatHint::Png)
            .expect("two-stack placement source resolves"),
    ))
    .expect("centered-local two-stack paths evaluate");
    for layer in two_result.scene().layers() {
        let GeometryOutput::CanonicalStrokes(strokes) = layer.geometry() else {
            panic!("two-stack envelope fixture publishes canonical strokes");
        };
        let horizontal = strokes
            .iter()
            .filter(|stroke| stroke.source_structure_id == Some(AuthoredStructureId(11)))
            .flat_map(|stroke| {
                let bounds = stroke
                    .path
                    .bounds()
                    .expect("horizontal stroke bounds are finite");
                [bounds.min.x, bounds.max.x]
            })
            .collect::<Vec<_>>();
        let vertical = strokes
            .iter()
            .filter(|stroke| stroke.source_structure_id == Some(AuthoredStructureId(12)))
            .flat_map(|stroke| {
                let bounds = stroke
                    .path
                    .bounds()
                    .expect("vertical stroke bounds are finite");
                [bounds.min.y, bounds.max.y]
            })
            .collect::<Vec<_>>();
        assert!(horizontal.iter().any(|value| *value <= 0.0));
        assert!(horizontal.iter().any(|value| *value >= canvas.width));
        assert!(vertical.iter().any(|value| *value <= 0.0));
        assert!(vertical.iter().any(|value| *value >= canvas.height));
    }
}

/// Proves every centered-local curved site fixture still realizes visible RGB marks at the reduced evidence canvas.
#[test]
fn curved_validation_site_variants_evaluate_after_centered_local_placement() {
    let canvas = CanvasSpec {
        width: 225.0,
        height: 155.0,
    };
    let source_pixels = image::RgbaImage::from_pixel(225, 155, image::Rgba([96, 144, 192, 255]));
    let mut encoded = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(source_pixels)
        .write_to(&mut encoded, image::ImageFormat::Png)
        .expect("curved site source encodes");
    let bytes = encoded.into_inner();
    for id in [
        "curved-one-stack-sites",
        "curved-one-normal-offset-sites",
        "curved-two-stack-intersections",
    ] {
        let source_id = SourceReferenceId::new(format!("centered-local-{id}"))
            .expect("curved site source ID validates");
        let mut history = curved_guide_history_for_variant(id, source_id.clone(), canvas.clone());
        apply_artifact_resolution_density(
            &mut history,
            [ChannelId(1), ChannelId(2), ChannelId(3)],
            id,
        );
        let result = evaluate(EvaluationRequest::new(
            history.session().document_evaluation_snapshot(),
            ResolvedSource::new(source_id, bytes.clone(), SourceFormatHint::Png)
                .expect("curved site source resolves"),
        ))
        .expect("centered-local curved site fixture evaluates");
        assert!(result.scene().layers().iter().all(|layer| matches!(
            layer.geometry(),
            GeometryOutput::CanonicalMarks(marks) if !marks.is_empty()
        )));
    }
}

/// Builds the validation-only heterogeneous catalog document with independent
/// RGB recipe materialization, blue artwork weighting, and typed seed edits.
fn heterogeneous_rgb_history(
    source_id: SourceReferenceId,
    width: f64,
    height: f64,
) -> (DocumentHistory, [ChannelId; 3], [Vec<String>; 3]) {
    let (mut history, channels, _) = artifact_history(source_id, width, height);
    let registry = PresetRegistry::bundled();
    for (channel, id) in [
        (channels[0], "three-guide-maze"),
        (channels[1], "round-spiral-marks"),
        (channels[2], "source-weighted-dispersion-voronoi"),
    ] {
        registry
            .apply_to_selected(&mut history, channel, id)
            .expect("heterogeneous recipe materializes");
    }
    let blue_definition = history
        .document()
        .pattern_definition_for(channels[2])
        .expect("blue definition")
        .clone();
    let weighted_id = blue_definition
        .mechanisms
        .iter()
        .find_map(|mechanism| match mechanism {
            PatternMechanism::SiteDensityModulation {
                id,
                modulation: toniator_domain::SiteDensityModulation::ArtworkWeighted { .. },
                ..
            } => Some(*id),
            _ => None,
        })
        .expect("blue recipe retains artwork-weighted modulation");
    history
        .apply(&DocumentCommand::EditSelectedChannelPatternDefinition {
            channel_id: channels[2],
            base_definition: blue_definition,
            edit: PatternDefinitionEdit::SetArtworkWeightMappingComponent {
                mechanism_id: weighted_id,
                component: SourceMappingComponent::Blue,
            },
        })
        .expect("blue artwork mapping command applies");
    let seed_edits = channels.map(|channel| apply_distinct_channel_seeds(&mut history, channel));
    (history, channels, seed_edits)
}

/// Proves the validation-only `heterogeneous-rgb-recipes` document evaluates
/// three independently materialized catalog definitions in visible RGB order.
#[test]
fn heterogeneous_rgb_recipes_replay_canonical_png_and_svg() {
    let source_id =
        SourceReferenceId::new("heterogeneous-rgb-recipes").expect("source ID validates");
    let pixels = image::RgbaImage::from_fn(32, 24, |x, y| {
        image::Rgba([
            ((x * 255) / 31) as u8,
            ((y * 255) / 23) as u8,
            ((x ^ y) * 10) as u8,
            255,
        ])
    });
    let mut encoded = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(pixels)
        .write_to(&mut encoded, image::ImageFormat::Png)
        .expect("heterogeneous source encodes");
    let bytes = encoded.into_inner();
    let (history, channels, seed_edits) = heterogeneous_rgb_history(source_id.clone(), 32.0, 24.0);
    assert!(
        seed_edits[0]
            .iter()
            .any(|edit| edit.starts_with("maze output"))
    );
    assert!(
        seed_edits[2]
            .iter()
            .any(|edit| edit.starts_with("random mechanism"))
    );
    assert_ne!(
        seed_edits[0], seed_edits[2],
        "heterogeneous typed seeds remain independent"
    );
    let kinds = channels.map(|channel| {
        history
            .document()
            .pattern_definition_for(channel)
            .expect("heterogeneous definition")
            .output_layers[0]
            .realization
            .clone()
    });
    assert!(matches!(
        kinds[0],
        PatternOutputRealization::MazeWalls { .. }
    ));
    assert!(matches!(
        kinds[1],
        PatternOutputRealization::MarkPrototype { .. }
    ));
    assert!(matches!(kinds[2], PatternOutputRealization::Regions { .. }));
    let source = ResolvedSource::new(source_id.clone(), bytes.clone(), SourceFormatHint::Png)
        .expect("heterogeneous source resolves");
    let first = evaluate(EvaluationRequest::new(
        history.session().document_evaluation_snapshot(),
        source,
    ))
    .expect("heterogeneous RGB fixture evaluates");
    assert_eq!(first.scene().layers().len(), 3);
    assert!(matches!(
        first.scene().layers()[0].geometry(),
        GeometryOutput::CanonicalStrokes(_)
    ));
    assert!(matches!(
        first.scene().layers()[1].geometry(),
        GeometryOutput::CanonicalMarks(_)
    ));
    assert!(matches!(
        first.scene().layers()[2].geometry(),
        GeometryOutput::CanonicalRegions(_)
    ));
    let png = encode_png(first.raster()).expect("heterogeneous PNG encodes");
    let svg = write_svg(first.scene());
    let replay = evaluate(EvaluationRequest::new(
        history.session().document_evaluation_snapshot(),
        ResolvedSource::new(source_id, bytes, SourceFormatHint::Png)
            .expect("heterogeneous replay source resolves"),
    ))
    .expect("heterogeneous RGB replay evaluates");
    assert_eq!(
        png,
        encode_png(replay.raster()).expect("heterogeneous replay PNG encodes")
    );
    assert_eq!(svg, write_svg(replay.scene()));
    let identities = first
        .channels()
        .iter()
        .map(|channel| (channel.family_identity(), channel.realization_identity()))
        .collect::<Vec<_>>();
    assert_ne!(identities[0], identities[1]);
    assert_ne!(identities[0], identities[2]);
    assert_ne!(identities[1], identities[2]);
}

/// Clones one authoritative document and hides every non-target channel using
/// ordinary history commands. The resulting document remains a modeled RGB
/// document, so its canonical output isolates the target channel without a
/// test-only evaluator shortcut.
fn isolated_channel_history(history: &DocumentHistory, target: ChannelId) -> DocumentHistory {
    let session = DocumentSession::new(history.document().clone()).unwrap();
    let mut isolated = DocumentHistory::new(session);
    let ids = isolated
        .document()
        .channel_topology()
        .unwrap()
        .channels()
        .iter()
        .map(|channel| channel.id)
        .collect::<Vec<_>>();
    for channel_id in ids.into_iter().filter(|channel_id| *channel_id != target) {
        isolated
            .apply(&DocumentCommand::SetVisibility {
                channel_id,
                visible: false,
            })
            .unwrap();
    }
    isolated
}

/// Returns the public canonical identity for one authoritative channel and
/// panics only when the engine fails to preserve the document topology it just
/// evaluated in this test fixture.
fn channel_identity(output: &CanonicalOutput, channel_id: ChannelId) -> &ChannelCanonicalIdentity {
    output
        .channels
        .iter()
        .find(|channel| channel.channel_id == channel_id)
        .expect("evaluated modeled document retains every authoritative channel")
}

/// Returns the canonical geometry retained for one channel in an ordinary
/// complete-document scene. It proves per-channel output stability without
/// treating aggregate SVG metadata (which includes red) as green/blue output.
fn channel_geometry(
    output: &toniator_engine::EvaluationResult,
    channel_id: ChannelId,
) -> &GeometryOutput {
    output
        .scene()
        .layers()
        .iter()
        .find(|layer| layer.channel_id() == channel_id)
        .expect("evaluated modeled scene retains every authoritative channel")
        .geometry()
}

/// Evaluates a complete modeled document without serializing it, so tests can
/// compare one unaffected channel's canonical geometry directly after another
/// channel's typed definition edit.
fn evaluated_document(
    history: &DocumentHistory,
    source: ResolvedSource,
) -> toniator_engine::EvaluationResult {
    evaluate(EvaluationRequest::new(
        history.session().document_evaluation_snapshot(),
        source,
    ))
    .unwrap()
}

/// Builds RGB document state in which red owns even-random, green owns the
/// visibly non-default straight-grid recipe, and blue retains its original
/// default definition. Each selected application is history-backed and must
/// disclose exactly its selected channel.
fn independent_rgb_history(
    source_id: SourceReferenceId,
    width: f64,
    height: f64,
) -> (DocumentHistory, [ChannelId; 3]) {
    let registry = PresetRegistry::bundled();
    let mut history = history(source_id, width, height);
    let channels = [ChannelId(1), ChannelId(2), ChannelId(3)];
    let blue_definition = history
        .document()
        .pattern_definition_for(channels[2])
        .unwrap()
        .id;
    let red = registry
        .apply_to_selected(&mut history, channels[0], "even-random-circles")
        .unwrap();
    let green = registry
        .apply_to_selected(&mut history, channels[1], "straight-grid-circles")
        .unwrap();
    assert_eq!(red.affected_channels, vec![channels[0]]);
    assert_eq!(green.affected_channels, vec![channels[1]]);
    let definition_ids = channels.map(|channel_id| {
        history
            .document()
            .pattern_definition_for(channel_id)
            .unwrap()
            .id
    });
    assert_ne!(definition_ids[0], definition_ids[1]);
    assert_ne!(definition_ids[0], definition_ids[2]);
    assert_ne!(definition_ids[1], definition_ids[2]);
    assert_eq!(definition_ids[2], blue_definition);
    assert_eq!(
        history
            .document()
            .channel_topology()
            .unwrap()
            .channels()
            .iter()
            .map(|channel| channel.id)
            .collect::<Vec<_>>(),
        channels,
        "the RGB document retains canonical deterministic channel order"
    );
    (history, channels)
}

/// Builds one ordinary catalog workload and immutable source for performance validation.
fn performance_fixture(
    preset_id: &str,
    width: f64,
    height: f64,
) -> (DocumentHistory, ResolvedSource) {
    let source_id = SourceReferenceId::new(format!("stage20-profile-{preset_id}"))
        .expect("profile source ID validates");
    let source_bytes =
        fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/raster-sample.png"))
            .expect("immutable raster source reads");
    let source = ResolvedSource::new(source_id.clone(), source_bytes, SourceFormatHint::Png)
        .expect("immutable raster source resolves");
    let mut history = history(source_id, width, height);
    apply_recipe_to_rgb(
        &PresetRegistry::bundled(),
        &mut history,
        [ChannelId(1), ChannelId(2), ChannelId(3)],
        preset_id,
    );
    (history, source)
}

/// Proves one-worker and multi-worker evaluation preserve exact semantic output and work counts.
#[test]
fn parallel_evaluation_matches_single_worker_reference() {
    let (history, source) = performance_fixture("straight-grid-circles", 320.0, 240.0);
    let request = || {
        EvaluationRequest::new(
            history.session().document_evaluation_snapshot(),
            source.clone(),
        )
    };
    let one = rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build()
        .expect("one-worker pool builds")
        .install(|| {
            evaluate_profiled_with_limits(request(), EvaluationLimits::default())
                .expect("one-worker evaluation completes")
        });
    let many = rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .expect("four-worker pool builds")
        .install(|| {
            evaluate_profiled_with_limits(request(), EvaluationLimits::default())
                .expect("four-worker evaluation completes")
        });
    assert_eq!(one.result, many.result);
    assert_eq!(one.diagnostics, many.diagnostics);
    let stable = |profile: &toniator_engine::EvaluationPerformanceMetrics| {
        profile
            .records
            .iter()
            .map(|record| {
                (
                    record.stage,
                    record.channel_id,
                    record.output_layer_id,
                    record.cache,
                    record.workloads.clone(),
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(stable(&one.performance), stable(&many.performance));
    assert_eq!(one.performance.configured_worker_count, 1);
    assert_eq!(many.performance.configured_worker_count, 4);
    assert_eq!(one.performance.observed_worker_count, 1);
    assert!(
        many.performance.observed_worker_count > 1,
        "parallel work must execute cancellation-polled work on multiple Rayon workers"
    );
    assert!(many.performance.worker_registration_count > 1);
}

/// Prints release-mode architectural metrics for one caller-selected ordinary catalog workload.
#[test]
#[ignore = "run explicitly in release mode for Stage 20 performance evidence"]
fn stage20_closeout_release_profile() {
    let preset_id = std::env::var("STAGE20_PROFILE_PRESET_ID")
        .unwrap_or_else(|_| "straight-grid-circles".to_owned());
    let size = std::env::var("STAGE20_PROFILE_SIZE")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(512);
    let (history, source) = performance_fixture(&preset_id, f64::from(size), f64::from(size));
    let request = || {
        EvaluationRequest::new(
            history.session().document_evaluation_snapshot(),
            source.clone(),
        )
    };
    let mut cache = EvaluationProfileCache::default();
    let profile =
        evaluate_profiled_cached_with_limits(request(), EvaluationLimits::default(), &mut cache)
            .expect("cold profile workload evaluates");
    let png_started = Instant::now();
    let png = encode_png(profile.result.raster()).expect("profile raster encodes");
    let png_elapsed = png_started.elapsed();
    let svg_started = Instant::now();
    let svg = write_svg(profile.result.scene());
    let svg_elapsed = svg_started.elapsed();
    println!(
        "stage20_profile run=cold preset={preset_id} size={size} configured_workers={} observed_workers={} worker_registrations={}",
        profile.performance.configured_worker_count,
        profile.performance.observed_worker_count,
        profile.performance.worker_registration_count,
    );
    println!(
        "stage20_export preset={preset_id} size={size} png_elapsed_us={} png_bytes={} svg_elapsed_us={} svg_bytes={}",
        png_elapsed.as_micros(),
        png.len(),
        svg_elapsed.as_micros(),
        svg.len(),
    );
    for record in profile.performance.records {
        println!(
            "stage={:?} channel={:?} output={:?} cache={:?} execution={:?} elapsed_us={} workloads={:?}",
            record.stage,
            record.channel_id,
            record.output_layer_id,
            record.cache,
            record.execution,
            record.elapsed.as_micros(),
            record.workloads
        );
    }
    let warm =
        evaluate_profiled_cached_with_limits(request(), EvaluationLimits::default(), &mut cache)
            .expect("warm profile workload evaluates");
    println!(
        "stage20_profile run=warm preset={preset_id} size={size} configured_workers={} observed_workers={} worker_registrations={}",
        warm.performance.configured_worker_count,
        warm.performance.observed_worker_count,
        warm.performance.worker_registration_count,
    );
    for record in warm.performance.records {
        println!(
            "stage={:?} channel={:?} output={:?} cache={:?} execution={:?} elapsed_us={} workloads={:?}",
            record.stage,
            record.channel_id,
            record.output_layer_id,
            record.cache,
            record.execution,
            record.elapsed.as_micros(),
            record.workloads
        );
    }
}

/// Applies one typed random-seed edit to red through `DocumentHistory`; the
/// command uses the public existing mechanism ID and leaves independently
/// owned green/blue definitions outside the command's affected scope.
fn edit_red_seed(history: &mut DocumentHistory, red: ChannelId) {
    let base_definition = history
        .document()
        .pattern_definition_for(red)
        .unwrap()
        .clone();
    let random_id = base_definition
        .mechanisms
        .iter()
        .find_map(|mechanism| match mechanism {
            PatternMechanism::RandomSiteProcess { id, .. } => Some(*id),
            _ => None,
        })
        .expect("even-random preset retains its typed random mechanism");
    let result = history
        .apply(&DocumentCommand::EditSelectedChannelPatternDefinition {
            channel_id: red,
            base_definition,
            edit: PatternDefinitionEdit::SetRandomSeed {
                mechanism_id: random_id,
                seed: 91,
            },
        })
        .unwrap();
    assert_eq!(result.affected_channels, vec![red]);
}

/// Saves, reloads, materializes, evaluates, and renders every bundled preset
/// through three independent visible ordinary RGB channels.
///
/// The generated validation evidence uses only current recipes, ordinary
/// preset-v3 persistence, the canonical engine/render boundary, and existing
/// request limits. `STAGE20S_FIXTURE_ID` selects exactly one validation-only
/// curved or heterogeneous document without rewriting catalog artifacts,
/// MANIFEST, or inventory. `STAGE20S_MANIFEST_EXISTING=1` instead audits
/// retained evidence and writes only a compact MANIFEST without evaluation or
/// artifact regeneration. The test does not claim visual acceptance.
#[test]
fn bundled_presets_reload_and_preserve_canonical_output_parity() {
    let registry = PresetRegistry::bundled();
    let selected = std::env::var("STAGE20S_PRESET_ID").ok();
    let selected_fixture = std::env::var("STAGE20S_FIXTURE_ID").ok();
    let manifest_existing = match std::env::var("STAGE20S_MANIFEST_EXISTING") {
        Ok(value) => {
            assert_eq!(
                value, "1",
                "existing-evidence manifest mode accepts only STAGE20S_MANIFEST_EXISTING=1"
            );
            true
        }
        Err(std::env::VarError::NotPresent) => false,
        Err(error) => panic!("existing-evidence manifest selector is readable: {error}"),
    };
    assert!(
        !(manifest_existing && (selected.is_some() || selected_fixture.is_some()))
            && (selected.is_none() || selected_fixture.is_none()),
        "existing manifest mode, one selected catalog preset, or one validation fixture may run at a time"
    );
    let output = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/validation/stage20s");
    if manifest_existing {
        assert!(
            output.is_dir(),
            "existing-evidence manifest mode requires parent-prepared Stage 20S artifacts"
        );
        write_stage20s_manifest_from_existing(&output, &registry);
        return;
    }
    fs::create_dir_all(&output).expect("stage20s validation directory exists");
    let full_catalog_run = selected.is_none() && selected_fixture.is_none();
    let derived_inputs = if full_catalog_run
        || selected_fixture.is_some()
        || selected.as_deref().is_some_and(|preset_id| {
            DERIVED_RASTER_ARTIFACT_PRESET_IDS.contains(&preset_id)
                || DERIVED_SVG_ARTIFACT_PRESET_IDS.contains(&preset_id)
        }) {
        Some(write_stage20s_derived_inputs(&output))
    } else {
        None
    };
    let mut manifest_records = Vec::new();
    for record in registry.entries() {
        let preset_id = record.metadata.id.as_str();
        if selected_fixture.is_some() || selected.as_deref().is_some_and(|id| id != preset_id) {
            continue;
        }
        let input = artifact_input(preset_id, derived_inputs.as_ref());
        let record = registry.find(preset_id).unwrap();
        let catalog_entry = registry
            .catalog_entries()
            .iter()
            .find(|entry| entry.preset.metadata.id == record.metadata.id)
            .expect("bundled record retains catalog entry");
        let preset_path = output.join(format!("{preset_id}.preset.json"));
        save_preset(&preset_path, record).unwrap();
        let reloaded = load_preset(&preset_path).unwrap();
        assert_eq!(reloaded, *record);
        let source_id = SourceReferenceId::new(format!("preset-{preset_id}")).unwrap();
        let source_bytes = fs::read(&input.source_path).expect("assigned artifact source reads");
        let source_hash = sha256(&source_bytes);
        let source = ResolvedSource::new(source_id.clone(), source_bytes, input.format)
            .expect("assigned source resolves");
        let (mut original_history, channels, topology_paint) = artifact_history(
            source_id.clone(),
            input.canvas_dimensions.0,
            input.canvas_dimensions.1,
        );
        let (mut reloaded_history, reloaded_channels, reloaded_topology_paint) = artifact_history(
            source_id,
            input.canvas_dimensions.0,
            input.canvas_dimensions.1,
        );
        assert_eq!(channels, reloaded_channels);
        assert_eq!(topology_paint, reloaded_topology_paint);
        apply_recipe_to_rgb(&registry, &mut original_history, channels, preset_id);
        let reloaded_registry = PresetRegistry::new(registry.version(), vec![reloaded]).unwrap();
        apply_recipe_to_rgb(
            &reloaded_registry,
            &mut reloaded_history,
            reloaded_channels,
            preset_id,
        );
        let density_edit =
            apply_artifact_resolution_density(&mut original_history, channels, preset_id);
        let reloaded_density_edit =
            apply_artifact_resolution_density(&mut reloaded_history, reloaded_channels, preset_id);
        assert_eq!(density_edit, reloaded_density_edit);
        let original_seed_edits = channels
            .map(|channel_id| apply_distinct_channel_seeds(&mut original_history, channel_id));
        let reloaded_seed_edits = reloaded_channels
            .map(|channel_id| apply_distinct_channel_seeds(&mut reloaded_history, channel_id));
        assert_eq!(original_seed_edits, reloaded_seed_edits);
        assert_distinct_rgb_seed_edits(&original_seed_edits);
        let projection = original_history
            .document()
            .pattern_capabilities(PatternCapabilityScope::Channel(channels[0]))
            .expect("materialized recipe projects active capabilities");
        let request = || {
            EvaluationRequest::new(
                original_history.session().document_evaluation_snapshot(),
                source.clone(),
            )
        };
        let limits = artifact_limits();
        let original = evaluate_with_limits(request(), limits)
            .unwrap_or_else(|error| panic!("{preset_id} original evaluation failed: {error:?}"));
        let reloaded_request = EvaluationRequest::new(
            reloaded_history.session().document_evaluation_snapshot(),
            source.clone(),
        );
        let reloaded_output = evaluate_with_limits(reloaded_request, limits)
            .unwrap_or_else(|error| panic!("{preset_id} reloaded evaluation failed: {error:?}"));
        let original_png = encode_png(original.raster()).unwrap();
        let reloaded_png = encode_png(reloaded_output.raster()).unwrap();
        let original_svg = write_svg(original.scene());
        assert_eq!(original_png, reloaded_png);
        assert_eq!(original_svg, write_svg(reloaded_output.scene()));
        assert_eq!(
            original
                .scene()
                .layers()
                .iter()
                .map(|layer| layer.channel_id())
                .collect::<Vec<_>>(),
            channels,
            "canonical scene retains all modeled RGB channels in order"
        );
        assert!(
            original
                .scene()
                .layers()
                .iter()
                .all(|layer| layer.visible())
        );
        for (channel_id, expected_color) in
            channels
                .into_iter()
                .zip([(1.0, 0.0, 0.0), (0.0, 1.0, 0.0), (0.0, 0.0, 1.0)])
        {
            let layer = original
                .scene()
                .layers()
                .iter()
                .find(|layer| layer.channel_id() == channel_id)
                .expect("visible RGB channel retains a scene layer");
            assert_eq!(
                (layer.color().red, layer.color().green, layer.color().blue),
                expected_color,
                "artifact channel retains ordinary modeled RGB paint"
            );
            assert!(
                original_svg.contains(&format!("id=\"channel-{}\"", channel_id.0)),
                "raw SVG retains the visible modeled channel group"
            );
        }
        let scheduler_cache = scheduler_cache_evidence(&original_history, source, limits);
        fs::write(output.join(format!("{preset_id}.png")), original_png)
            .expect("native png artifact writes");
        let svg_path = output.join(format!("{preset_id}.svg"));
        fs::write(&svg_path, original_svg).expect("raw svg artifact writes");
        let svg_raster = matches!(
            preset_id,
            "one-guide-lines"
                | "triagrid-custom-shape-marks"
                | "source-weighted-dispersion-voronoi"
                | "two-guide-cells-uniform-offset"
                | "round-spiral-marks"
                | "three-guide-maze"
                | "residual-sites-along-guide"
        );
        let svg_raster_record = if svg_raster {
            let svg_raster_path = output.join(format!("{preset_id}-svg-raster.png"));
            let status = Command::new("inkscape")
                .arg(&svg_path)
                .arg("--export-type=png")
                .arg(format!("--export-filename={}", svg_raster_path.display()))
                .status()
                .expect("Inkscape is required for Stage 20S SVG evidence");
            assert!(status.success(), "Inkscape rasterizes Stage 20S raw SVG");
            format!(
                "hash={}; {}",
                sha256(&fs::read(&svg_raster_path).expect("SVG-raster artifact reads")),
                png_statistics(&svg_raster_path)
            )
        } else {
            "not required for this representative set".into()
        };
        let png_path = output.join(format!("{preset_id}.png"));
        let preset_text = fs::read_to_string(&preset_path).expect("serialized preset reads");
        let derived_data_absent = [
            "\"derived\"",
            "\"cache\"",
            "\"site_set\"",
            "\"selected_edges\"",
            "\"trails\"",
            "\"canonical_geometry\"",
        ]
        .iter()
        .all(|forbidden| !preset_text.contains(forbidden));
        assert!(
            derived_data_absent,
            "preset stores authored recipe intent only"
        );
        let channel_identities = original
            .channels()
            .iter()
            .map(|channel| {
                format!(
                    "channel={:?}; family_sha256={}; output_sha256={}",
                    channel.channel_id(),
                    sha256(channel.family_identity().as_bytes()),
                    sha256(channel.realization_identity().as_bytes())
                )
            })
            .collect::<Vec<_>>()
            .join(" | ");
        let channel_audits = channels
            .into_iter()
            .zip(original_seed_edits)
            .map(|(channel_id, seed_edits)| {
                format!(
                    "channel={}; {}; typed edits=[{}]",
                    channel_id.0,
                    channel_definition_audit(&original_history, channel_id),
                    seed_edits.join("; ")
                )
            })
            .collect::<Vec<_>>()
            .join(" | ");
        manifest_records.push(format!(
            "## `{preset_id}`\n\
             - metadata: name=`{}`, category=`{}`, description=`{}`; required flags={:?} (gallery-only, nonserialized, nonidentity)\n\
             - projected flags={:?}; active controls=[{}]\n\
             - topology/paint: {topology_paint}; all R/G/B channels are visible with ordinary solid channel paint; source: `{}` ({:?}, {}x{}, sha256:{source_hash}); canvas: {:.0}x{:.0}; input policy: {}\n\
             - per-channel definitions and deterministic seed edits: {channel_audits}\n\
             - artifact-only density edits: {density_edit}\n\
             - request limits: {}; reconstruction: preset-v3 save/load exact; direct/reloaded PNG and raw SVG exact\n\
             - identities: {channel_identities}; scene_sha256={}\n\
             - scheduler cache: {scheduler_cache}\n\
             - artifacts: preset sha256:{}; native PNG sha256:{}; raw SVG sha256:{}\n\
             - native PNG RGBA: {}\n\
             - SVG raster: {svg_raster_record}\n\
             - derived-data absence: {derived_data_absent} (no cached/canonical/site-set/selected-edge/trail payload)\n",
            record.metadata.name,
            record.metadata.category,
            record.metadata.description,
            catalog_entry.required_features,
            projection.features,
            compact_active_controls(&projection.active_controls),
            input.source_path.display(),
            input.format,
            input.source_dimensions.0,
            input.source_dimensions.1,
            input.canvas_dimensions.0,
            input.canvas_dimensions.1,
            input.manifest_note,
            "default plus existing connection inspection bounds",
            sha256(original.scene().identity().scene_fingerprint().as_bytes()),
            sha256(preset_text.as_bytes()),
            sha256(&fs::read(&png_path).expect("native PNG artifact reads")),
            sha256(fs::read_to_string(&svg_path).expect("raw SVG artifact reads").as_bytes()),
            rgba_statistics(
                original.raster().width(),
                original.raster().height(),
                original.raster().pixels(),
            ),
        ));
    }
    if full_catalog_run {
        assert_eq!(manifest_records.len(), registry.entries().len());
        let derived_inputs = derived_inputs
            .as_ref()
            .expect("full catalog run prepared derived inputs before its catalog loop");
        manifest_records.push(format!(
            "## Derived Inputs\n- `{}` -> `{}` ({:?} {}x{}, source_sha256={}, derived_sha256={}, method={})\n- `{}` -> `{}` ({:?} {}x{}, source_sha256={}, derived_sha256={}, method={})\n",
            derived_inputs[0].source_path, derived_inputs[0].derived_path.display(), derived_inputs[0].format, derived_inputs[0].dimensions.0, derived_inputs[0].dimensions.1, derived_inputs[0].source_sha256, derived_inputs[0].derived_sha256, derived_inputs[0].method,
            derived_inputs[1].source_path, derived_inputs[1].derived_path.display(), derived_inputs[1].format, derived_inputs[1].dimensions.0, derived_inputs[1].dimensions.1, derived_inputs[1].source_sha256, derived_inputs[1].derived_sha256, derived_inputs[1].method,
        ));
        let heterogeneous_source_id = SourceReferenceId::new("heterogeneous-rgb-artifact")
            .expect("heterogeneous artifact source ID validates");
        let (mut heterogeneous_history, heterogeneous_channels, heterogeneous_seeds) =
            heterogeneous_rgb_history(heterogeneous_source_id.clone(), 225.0, 155.0);
        let heterogeneous_density = apply_artifact_resolution_density(
            &mut heterogeneous_history,
            heterogeneous_channels,
            "heterogeneous-rgb-recipes",
        );
        let heterogeneous_kinds = heterogeneous_channels.map(|channel| {
            heterogeneous_history
                .document()
                .pattern_definition_for(channel)
                .expect("heterogeneous artifact definition remains current")
                .output_layers[0]
                .realization
                .clone()
        });
        assert!(matches!(
            heterogeneous_kinds[0],
            PatternOutputRealization::MazeWalls { .. }
        ));
        assert!(matches!(
            heterogeneous_kinds[1],
            PatternOutputRealization::MarkPrototype { .. }
        ));
        assert!(matches!(
            heterogeneous_kinds[2],
            PatternOutputRealization::Regions { .. }
        ));
        let heterogeneous_source = ResolvedSource::new(
            heterogeneous_source_id,
            fs::read(&derived_inputs[1].derived_path).expect("derived heterogeneous SVG reads"),
            SourceFormatHint::Svg,
        )
        .expect("derived heterogeneous SVG resolves");
        let heterogeneous_record = write_validation_fixture_artifacts(
            "heterogeneous-rgb-recipes",
            &heterogeneous_history,
            heterogeneous_source,
            &output,
        );
        assert!(
            !output
                .join("heterogeneous-rgb-recipes.preset.json")
                .exists(),
            "heterogeneous validation document never writes a preset JSON"
        );
        manifest_records.push(format!(
            "## heterogeneous-rgb-recipes\n- validation-only document; no preset JSON is written\n- input policy: derived `assets/vector-sample.svg` provenance source_sha256={}, derived_sha256={}, method={}; 25%-linear 225x155 source/canvas; artifact-only density: {heterogeneous_density}\n- channels: red=three-guide-maze; green=round-spiral-marks; blue=source-weighted-dispersion-voronoi\n- mappings: R/G/B; blue artwork weighting=Blue; seeds={:?}\n- {}\n",
            derived_inputs[1].source_sha256,
            derived_inputs[1].derived_sha256,
            derived_inputs[1].method,
            heterogeneous_seeds,
            heterogeneous_record,
        ));
        let curved_bytes = fs::read(&derived_inputs[1].derived_path)
            .expect("derived curved fixture vector source reads");
        manifest_records.push(format!(
            "## Curved Guide Demonstrations\n- validation-only documents; no preset JSON is written\n- input policy: immutable `assets/vector-sample.svg` source_sha256={}, derived SVG sha256={}, method={}; user-authorized 25%-linear 225x155 source/canvas. Full-size 900x620 `curved-one-stack-paths` preflight failed at `realization.stroke.profile_limit` after total test elapsed 155.73s; no curved file was written.\n",
            derived_inputs[1].source_sha256,
            derived_inputs[1].derived_sha256,
            derived_inputs[1].method,
        ));
        for id in CURVED_GUIDE_FIXTURE_IDS {
            let curved_source_id = SourceReferenceId::new(format!("curved-artifact-{id}"))
                .expect("curved fixture source ID validates");
            let mut history = curved_guide_history_for_variant(
                id,
                curved_source_id.clone(),
                CanvasSpec {
                    width: 225.0,
                    height: 155.0,
                },
            );
            let curved_density = apply_artifact_resolution_density(
                &mut history,
                [ChannelId(1), ChannelId(2), ChannelId(3)],
                id,
            );
            let source = ResolvedSource::new(
                curved_source_id,
                curved_bytes.clone(),
                SourceFormatHint::Svg,
            )
            .expect("curved fixture vector source resolves");
            manifest_records.push(format!(
                "- `{id}`: artifact-only density: {curved_density}; {}\n",
                write_validation_fixture_artifacts(id, &history, source, &output),
            ));
        }
        let manifest = format!(
            "# Stage 20S generated artifact manifest\n\nAll records are generated through the ordinary recipe, preset-v3, engine, PNG, raw SVG, deterministic replay, and scheduler-cache boundaries. Each artifact applies its selected recipe independently to visible ordinary red, green, and blue channels, then records deterministic typed per-channel seed edits where the recipe owns seeds. SVG-raster PNGs use Inkscape. The SVG input contains live text, so SVG-raster pixels remain subject to the installed-font caveat in `assets/README.md`; raw SVG and native PNG remain the canonical artifacts. Required flags are gallery-only/nonserialized/nonidentity.\n\n{}",
            manifest_records.join("\n")
        );
        assert!(
            manifest.len() < 1_000_000,
            "manifest stays bounded for human review: {} bytes",
            manifest.len()
        );
        fs::write(output.join("MANIFEST.md"), manifest).expect("manifest writes");
        assert_current_stage20s_artifact_inventory(&output, &registry);
    } else if let Some(id) = selected_fixture.as_deref() {
        let derived_inputs = derived_inputs
            .as_ref()
            .expect("selected validation fixture prepares derived SVG inputs");
        write_selected_validation_fixture_artifacts(id, &output, derived_inputs);
    }
}

/// Proves that RGB preset applications allocate three independent document
/// definitions and that a later typed red seed edit changes only red's
/// canonical output/identity. It compares isolated green/blue PNG bytes plus
/// complete-document geometry and per-channel identity; isolated SVG bytes are
/// intentionally excluded because hidden document-wide identity metadata still
/// changes when red changes.
#[test]
fn independent_rgb_presets_preserve_unaffected_channel_canonical_outputs() {
    let cases = [
        (
            "rgb-raster-1024",
            "raster-sample.png",
            "../../assets/raster-sample.png",
            SourceFormatHint::Png,
            1024.0,
            1024.0,
        ),
        (
            "rgb-vector-900x620",
            "vector-sample.svg",
            "../../assets/vector-sample.svg",
            SourceFormatHint::Svg,
            900.0,
            620.0,
        ),
    ];
    for (artifact_name, _source_name, source_path, format, width, height) in cases {
        let source_id = SourceReferenceId::new(format!("independent-{artifact_name}")).unwrap();
        let source =
            ResolvedSource::new(source_id.clone(), fs::read(source_path).unwrap(), format).unwrap();
        let (mut history, [red, green, blue]) = independent_rgb_history(source_id, width, height);
        let green_definition_before = history
            .document()
            .pattern_definition_for(green)
            .unwrap()
            .clone();
        let blue_definition_before = history
            .document()
            .pattern_definition_for(blue)
            .unwrap()
            .clone();
        let before_result = evaluated_document(&history, source.clone());
        let before = CanonicalOutput {
            png: encode_png(before_result.raster()).unwrap(),
            svg: write_svg(before_result.scene()),
            channels: before_result
                .channels()
                .iter()
                .map(|channel| ChannelCanonicalIdentity {
                    channel_id: channel.channel_id(),
                    family: channel.family_identity().into(),
                    realization: channel.realization_identity().into(),
                })
                .collect(),
        };
        let red_before = canonical_output(&isolated_channel_history(&history, red), source.clone());
        let green_before =
            canonical_output(&isolated_channel_history(&history, green), source.clone());
        let blue_before =
            canonical_output(&isolated_channel_history(&history, blue), source.clone());

        edit_red_seed(&mut history, red);

        let after_result = evaluated_document(&history, source.clone());
        let after = CanonicalOutput {
            png: encode_png(after_result.raster()).unwrap(),
            svg: write_svg(after_result.scene()),
            channels: after_result
                .channels()
                .iter()
                .map(|channel| ChannelCanonicalIdentity {
                    channel_id: channel.channel_id(),
                    family: channel.family_identity().into(),
                    realization: channel.realization_identity().into(),
                })
                .collect(),
        };
        let red_after = canonical_output(&isolated_channel_history(&history, red), source.clone());
        let green_after =
            canonical_output(&isolated_channel_history(&history, green), source.clone());
        let blue_after = canonical_output(&isolated_channel_history(&history, blue), source);
        assert_eq!(
            history.document().pattern_definition_for(green).unwrap(),
            &green_definition_before,
            "red's selected typed edit cannot mutate green's independent definition"
        );
        assert_eq!(
            history.document().pattern_definition_for(blue).unwrap(),
            &blue_definition_before,
            "red's selected typed edit cannot mutate blue's default definition"
        );
        assert_ne!(before.png, after.png);
        assert_ne!(before.svg, after.svg);
        assert_ne!(red_before.png, red_after.png);
        assert_ne!(red_before.svg, red_after.svg);
        assert_eq!(green_before.png, green_after.png);
        assert_eq!(blue_before.png, blue_after.png);
        assert_eq!(
            channel_geometry(&before_result, green),
            channel_geometry(&after_result, green)
        );
        assert_eq!(
            channel_geometry(&before_result, blue),
            channel_geometry(&after_result, blue)
        );
        assert_ne!(
            channel_identity(&before, red),
            channel_identity(&after, red),
            "red's public family/realization identity changes with its seed"
        );
        assert_eq!(
            channel_identity(&before, green),
            channel_identity(&after, green)
        );
        assert_eq!(
            channel_identity(&before, blue),
            channel_identity(&after, blue)
        );
        let green_geometry_equal =
            channel_geometry(&before_result, green) == channel_geometry(&after_result, green);
        let blue_geometry_equal =
            channel_geometry(&before_result, blue) == channel_geometry(&after_result, blue);
        assert!(green_geometry_equal);
        assert!(blue_geometry_equal);
    }
}
