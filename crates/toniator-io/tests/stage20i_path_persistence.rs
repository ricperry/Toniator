use std::{
    fs,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use toniator_domain::{
    CanvasSpec, ConnectedGeometryResponse, CoveragePolicy, Document, DocumentId,
    GeneralizedSiteProduct, GuideDimensionId, MarkOrientation, PathStrokeStyle, PatternDefinition,
    PatternDefinitionBundle, PatternDefinitionId, PatternGeometryResponse, PatternMechanismId,
    PatternOutputLayer, PatternOutputLayerId, PatternOutputRealization, PatternOutputSettings,
    SourceReference, SourceReferenceId, StraightGuideDimension, StraightGuideRepetition,
};
use toniator_io::{EmbeddedSource, EmbeddedSourceFormat, SourceBundle, load, save};

/// Builds one persisted-v5 connected guide-path document against supplied immutable source bytes.
fn stroke_document(
    source_id: SourceReferenceId,
    width: f64,
    height: f64,
    resolution: f64,
    guard_steps: u32,
) -> Document {
    let base = Document::new_default_document(
        CanvasSpec { width, height },
        SourceReference::Assigned(source_id),
    )
    .expect("default document validates");
    let guide = PatternMechanismId(81);
    let mut definition = PatternDefinition::generalized_straight_guides(
        PatternDefinitionId(80),
        "Stage 20I path witness",
        guide,
        PatternMechanismId(82),
        PatternOutputLayerId(83),
        vec![StraightGuideDimension {
            id: GuideDimensionId(84),
            baseline_angle_degrees: 0.0,
            phase: 0.0,
            repetition: StraightGuideRepetition {
                spacing_multiplier: 1.0,
            },
        }],
        GeneralizedSiteProduct::AlongGuides {
            dimensions: vec![GuideDimensionId(84)],
            interval_multiplier: 1.0,
            phase: 0.0,
        },
        MarkOrientation::Fixed,
        CoveragePolicy {
            guard_steps,
            additional_margin: 0.0,
        },
    );
    definition.output_layers = vec![PatternOutputLayer::all(
        PatternOutputLayerId(83),
        PatternOutputRealization::GuidePaths {
            guide_mechanism_id: guide,
            style: PathStrokeStyle::default(),
        },
    )];
    let mut settings = base.pattern_settings().clone();
    settings.definition_id = definition.id;
    settings.density.across_x = resolution;
    settings.density.across_y = resolution;
    let bundle = PatternDefinitionBundle {
        output_settings: vec![PatternOutputSettings {
            output_layer_id: PatternOutputLayerId(83),
            response: PatternGeometryResponse::Connected(ConnectedGeometryResponse {
                minimum_thickness: 0.05,
                maximum_thickness: 0.8,
            }),
        }],
        definition,
    };
    Document::with_source_topology_and_authored_structures(
        DocumentId(820),
        base.canvas().clone(),
        base.source().clone(),
        vec![bundle],
        settings,
        base.channel_model().expect("modeled").to_owned(),
        base.channel_topology().expect("modeled").clone(),
        Vec::new(),
    )
    .expect("connected document validates")
}

/// Saves and reopens exact v5 guide-path witnesses for subsequent intrinsic CLI render evidence.
#[test]
fn save_reopen_connected_path_documents_for_immutable_sources() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../");
    let output = root.join("target/validation/stage-20i");
    fs::create_dir_all(&output).expect("derived validation directory creates");
    for (name, format, bytes, width, height) in [
        (
            "path-raster.toniator",
            EmbeddedSourceFormat::Png,
            fs::read(root.join("assets/raster-sample.png")).expect("immutable PNG reads"),
            1024.0,
            1024.0,
        ),
        (
            "path-vector.toniator",
            EmbeddedSourceFormat::Svg,
            fs::read(root.join("assets/vector-sample.svg")).expect("immutable SVG reads"),
            900.0,
            620.0,
        ),
    ] {
        let id = SourceReferenceId::new("stage20i-source").expect("valid id");
        let document = stroke_document(id.clone(), width, height, 32.0, 2);
        let bundle = SourceBundle::new([
            EmbeddedSource::new(id, format, bytes, Some(name.into())).expect("source valid")
        ])
        .expect("bundle valid");
        let path = output.join(name);
        save(&path, &document, &bundle).expect("v5 save succeeds");
        let reopened = load(&path).expect("v5 reopen succeeds");
        assert!(matches!(
            reopened.document().pattern_definition_bundles()[0].output_settings[0].response,
            PatternGeometryResponse::Connected(_)
        ));
        assert!(matches!(
            reopened.document().pattern_definition_bundles()[0]
                .definition
                .output_layers
                .as_slice(),
            [PatternOutputLayer {
                realization: PatternOutputRealization::GuidePaths { .. },
                ..
            }]
        ));
    }
    let low_resolution = root.join("target/validation/stage-20i/lowres");
    fs::create_dir_all(&low_resolution).expect("low-res validation directory creates");
    for (name, format, bytes, width, height) in [
        (
            "path-raster-lowres.toniator",
            EmbeddedSourceFormat::Png,
            fs::read(root.join("assets/raster-sample.png")).expect("immutable PNG reads"),
            64.0,
            64.0,
        ),
        (
            "path-vector-lowres.toniator",
            EmbeddedSourceFormat::Svg,
            fs::read(root.join("assets/vector-sample.svg")).expect("immutable SVG reads"),
            90.0,
            62.0,
        ),
    ] {
        let id = SourceReferenceId::new("stage20i-lowres-source").expect("valid id");
        let document = stroke_document(id.clone(), width, height, 2.0, 0);
        let bundle = SourceBundle::new([
            EmbeddedSource::new(id, format, bytes, Some(name.into())).expect("source valid")
        ])
        .expect("bundle valid");
        let path = low_resolution.join(name);
        save(&path, &document, &bundle).expect("low-res v5 save succeeds");
        assert!(load(&path).is_ok(), "low-res v5 document reopens");
    }
}

/// Proves deterministic v5 persistence preserves explicit guide-path style and omits derived canonical stroke state.
#[test]
fn connected_path_v5_save_is_deterministic_and_never_serializes_derived_strokes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../");
    let id = SourceReferenceId::new("stage20i-deterministic").expect("valid id");
    let document = stroke_document(id.clone(), 64.0, 48.0, 32.0, 2);
    let bytes = fs::read(root.join("assets/raster-sample.png")).expect("source reads");
    let sources = SourceBundle::new([EmbeddedSource::new(
        id,
        EmbeddedSourceFormat::Png,
        bytes,
        Some("raster.png".into()),
    )
    .expect("source")])
    .expect("bundle");
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("epoch")
        .as_nanos();
    let first = root.join(format!(
        "target/validation/stage-20i/deterministic-{stamp}-a.toniator"
    ));
    let second = root.join(format!(
        "target/validation/stage-20i/deterministic-{stamp}-b.toniator"
    ));
    save(&first, &document, &sources).expect("first v5 save");
    save(&second, &document, &sources).expect("second v5 save");
    assert_eq!(
        fs::read(&first).expect("first bytes"),
        fs::read(&second).expect("second bytes")
    );
    let reopened = load(&first).expect("reopen");
    assert!(
        matches!(reopened.document().pattern_definition_bundles()[0].definition.output_layers.as_slice(), [PatternOutputLayer { realization: PatternOutputRealization::GuidePaths { style, .. }, .. }] if *style == PathStrokeStyle::default())
    );
    let archive = fs::read(&first).expect("container bytes");
    assert!(!String::from_utf8_lossy(&archive).contains("canonical_stroke"));
    assert!(!String::from_utf8_lossy(&archive).contains("profile"));
}
