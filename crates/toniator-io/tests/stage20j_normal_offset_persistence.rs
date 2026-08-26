use std::{fs, path::Path};

use toniator_domain::{
    AuthoredCurveSegment, AuthoredPoint2, AuthoredStructure, AuthoredStructureId,
    AuthoredStructureKind, CanvasSpec, ConnectedGeometryResponse, CoveragePolicy, Document,
    DocumentId, GeneralizedSiteProduct, GuideDimension, GuideDimensionId, GuidePrototype,
    GuideRepetition, MarkOrientation, OffsetCleanup, OffsetSides, PathStrokeStyle,
    PatternDefinition, PatternDefinitionBundle, PatternDefinitionId, PatternGeometryResponse,
    PatternMechanismId, PatternOutputLayer, PatternOutputLayerId, PatternOutputSettings,
    SourceReference, SourceReferenceId,
};
use toniator_io::{EmbeddedSource, EmbeddedSourceFormat, SourceBundle, load, save};

/// Builds one current-v4 document whose persisted generic guide owns normal-offset intent.
fn normal_offset_document(
    source_id: SourceReferenceId,
    width: f64,
    height: f64,
    spacing: f64,
    cubic: bool,
    sides: OffsetSides,
) -> Document {
    let base = Document::new_default_document(
        CanvasSpec { width, height },
        SourceReference::Assigned(source_id),
    )
    .expect("base document validates");
    let mut definition = PatternDefinition::generalized_guides(
        PatternDefinitionId(91),
        "normal offset",
        PatternMechanismId(92),
        PatternMechanismId(93),
        PatternOutputLayerId(94),
        vec![GuideDimension {
            id: GuideDimensionId(95),
            baseline_angle_degrees: 0.0,
            phase: 0.0,
            prototype: GuidePrototype::AuthoredOpenPath {
                structure_id: AuthoredStructureId(96),
            },
            repetition: GuideRepetition::NormalOffset {
                spacing,
                sides,
                cleanup: OffsetCleanup::DissolveCrossings,
            },
        }],
        GeneralizedSiteProduct::AlongGuides {
            dimensions: vec![GuideDimensionId(95)],
            interval_multiplier: 1.0,
            phase: 0.0,
        },
        MarkOrientation::Fixed,
        CoveragePolicy {
            guard_steps: 1,
            additional_margin: 0.0,
        },
    );
    definition.output_layers = vec![PatternOutputLayer::GuidePaths {
        id: PatternOutputLayerId(94),
        guide_mechanism_id: PatternMechanismId(92),
        style: PathStrokeStyle::default(),
    }];
    let mut settings = base.pattern_settings().clone();
    settings.definition_id = definition.id;
    let bundle = PatternDefinitionBundle {
        output_settings: vec![PatternOutputSettings {
            output_layer_id: PatternOutputLayerId(94),
            response: PatternGeometryResponse::Connected(ConnectedGeometryResponse {
                minimum_thickness: 0.02,
                maximum_thickness: 0.05,
            }),
        }],
        definition,
    };
    Document::with_source_topology_and_authored_structures(
        DocumentId(91),
        base.canvas().clone(),
        base.source().clone(),
        vec![bundle],
        settings,
        base.channel_model().expect("modeled document").to_owned(),
        base.channel_topology().expect("modeled document").clone(),
        vec![
            AuthoredStructure::new(
                AuthoredStructureId(96),
                AuthoredStructureKind::OpenPath,
                vec![if cubic {
                    AuthoredCurveSegment::CubicBezier {
                        start: AuthoredPoint2 {
                            x: width * 0.0625,
                            y: height * 0.5,
                        },
                        control_1: AuthoredPoint2 {
                            x: width * 0.30,
                            y: height * 0.20,
                        },
                        control_2: AuthoredPoint2 {
                            x: width * 0.70,
                            y: height * 0.20,
                        },
                        end: AuthoredPoint2 {
                            x: width * 0.9375,
                            y: height * 0.5,
                        },
                    }
                } else {
                    AuthoredCurveSegment::Line {
                        start: AuthoredPoint2 {
                            x: width * 0.0625,
                            y: height * 0.5,
                        },
                        end: AuthoredPoint2 {
                            x: width * 0.9375,
                            y: height * 0.5,
                        },
                    }
                }],
            )
            .expect("authored guide validates"),
        ],
    )
    .expect("normal-offset document validates")
}

/// Proves current-v4 persistence retains authored normal-offset intent and no derived centerlines.
#[test]
fn normal_offset_v4_round_trip_is_deterministic_and_derived_state_free() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../");
    let output = root.join("target/validation/stage-20j");
    fs::create_dir_all(&output).expect("derived validation directory creates");
    let source_id = SourceReferenceId::new("stage20j-source").expect("source ID validates");
    let document = normal_offset_document(
        source_id.clone(),
        128.0,
        96.0,
        96.0,
        false,
        OffsetSides::Both,
    );
    let sources = SourceBundle::new([EmbeddedSource::new(
        source_id,
        EmbeddedSourceFormat::Svg,
        b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1\" height=\"1\"/>".to_vec(),
        Some("source.svg".into()),
    )
    .expect("embedded source validates")])
    .expect("source bundle validates");
    let first = output.join("normal-offset-a.toniator");
    let second = output.join("normal-offset-b.toniator");
    save(&first, &document, &sources).expect("first current-v4 save succeeds");
    let reopened = load(&first).expect("current-v4 load succeeds");
    save(&second, reopened.document(), reopened.sources()).expect("second save succeeds");
    assert_eq!(
        fs::read(&first).expect("first bytes"),
        fs::read(&second).expect("second bytes")
    );
    assert!(matches!(
        reopened.document().pattern_definition_bundles()[0].definition.mechanisms[0],
        toniator_domain::PatternMechanism::GuideDimensions { ref dimensions, .. }
            if matches!(dimensions[0].repetition,
                GuideRepetition::NormalOffset {
                    spacing,
                    sides: OffsetSides::Both,
                    cleanup: OffsetCleanup::DissolveCrossings,
        } if spacing.to_bits() == 96.0_f64.to_bits())
    ));
    let archive = fs::read(first).expect("container bytes");
    assert!(!String::from_utf8_lossy(&archive).contains("offset_path"));
}

/// Writes v4 native-artwork offset documents for the authoritative CLI render witnesses.
#[test]
fn writes_native_offset_documents_for_both_immutable_sources() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../");
    let output = root.join("target/validation/stage-20j");
    fs::create_dir_all(&output).expect("derived validation directory creates");
    for (name, source_name, format, width, height) in [
        (
            "normal-offset-raster.toniator",
            "raster-sample.png",
            EmbeddedSourceFormat::Png,
            1024.0,
            1024.0,
        ),
        (
            "normal-offset-vector.toniator",
            "vector-sample.svg",
            EmbeddedSourceFormat::Svg,
            900.0,
            620.0,
        ),
    ] {
        let source_id =
            SourceReferenceId::new(format!("stage20j-{source_name}")).expect("source ID validates");
        let document = normal_offset_document(
            source_id.clone(),
            width,
            height,
            96.0,
            false,
            OffsetSides::Both,
        );
        let bytes =
            fs::read(root.join("assets").join(source_name)).expect("immutable source reads");
        let sources = SourceBundle::new([EmbeddedSource::new(
            source_id,
            format,
            bytes,
            Some(source_name.into()),
        )
        .expect("embedded source validates")])
        .expect("source bundle validates");
        let path = output.join(name);
        save(&path, &document, &sources).expect("native v4 source document saves");
        assert!(load(&path).is_ok(), "native v4 source document reopens");
    }
}

/// Writes and byte-round-trips the intent-only cubic diagnostic whose source anchors the ladder.
#[test]
fn writes_cubic_offset_diagnostic_document() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../");
    let output = root.join("target/validation/stage-20j");
    fs::create_dir_all(&output).expect("derived validation directory creates");
    let source_id = SourceReferenceId::new("stage20j-cubic-source").expect("source ID validates");
    let document = normal_offset_document(
        source_id.clone(),
        320.0,
        320.0,
        12.0,
        true,
        OffsetSides::Both,
    );
    let sources = SourceBundle::new([EmbeddedSource::new(
        source_id,
        EmbeddedSourceFormat::Svg,
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="320" height="320" viewBox="0 0 320 320"><rect width="320" height="320" fill="#164e63"/><path d="M20 160 C96 64 224 64 300 160" fill="none" stroke="#fb7185" stroke-width="10"/></svg>"##.to_vec(),
        Some("cubic-centerline-diagnostic.svg".into()),
    )
    .expect("embedded source validates")])
    .expect("source bundle validates");
    let path = output.join("normal-offset-cubic.toniator");
    save(&path, &document, &sources).expect("cubic diagnostic saves");
    let original = fs::read(&path).expect("cubic diagnostic bytes");
    let reopened = load(&path).expect("cubic diagnostic reopens");
    assert!(matches!(
        reopened.document().pattern_definition_bundles()[0].definition.mechanisms[0],
        toniator_domain::PatternMechanism::GuideDimensions { ref dimensions, .. }
            if matches!(dimensions[0].repetition,
                GuideRepetition::NormalOffset {
                    spacing,
                    sides: OffsetSides::Both,
                    cleanup: OffsetCleanup::DissolveCrossings,
        } if spacing.to_bits() == 12.0_f64.to_bits())
    ));
    save(&path, reopened.document(), reopened.sources())
        .expect("reopened cubic diagnostic saves deterministically");
    assert_eq!(
        fs::read(&path).expect("reopened cubic diagnostic bytes"),
        original,
        "derived cusp cleanup never enters intent-only persistence"
    );
}
