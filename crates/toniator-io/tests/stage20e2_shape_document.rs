use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use toniator_domain::{
    AuthoredCurveSegment, AuthoredPoint2, AuthoredStructure, AuthoredStructureId,
    AuthoredStructureKind, CanvasSpec, CoveragePolicy, Document, GeneralizedSiteProduct,
    GuideDimensionId, MarkOrientation, MarkPrototype, PatternDefinition, PatternDefinitionId,
    PatternMechanismId, PatternOutputLayerId, PatternOutputRealization, SourceReference,
    SourceReferenceId, StraightGuideDimension, StraightGuideRepetition,
};
use toniator_io::{
    DOCUMENT_SCHEMA_VERSION, EmbeddedSource, EmbeddedSourceFormat, SourceBundle, load, save,
};

/// Allocates one process-local persistence path without changing repository state.
fn temporary(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "toniator-stage20e2-{name}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

/// Builds one current modeled document whose typed output references a self-intersecting shape.
fn shape_document() -> (Document, SourceBundle) {
    let source_id = SourceReferenceId::new("stage20e2-source").unwrap();
    let base = Document::new_default_document(
        CanvasSpec {
            width: 1024.0,
            height: 1024.0,
        },
        SourceReference::Assigned(source_id.clone()),
    )
    .unwrap();
    let mut definition = PatternDefinition::generalized_straight_guides(
        PatternDefinitionId(1),
        "persisted shape",
        PatternMechanismId(1),
        PatternMechanismId(2),
        PatternOutputLayerId(1),
        vec![
            StraightGuideDimension {
                id: GuideDimensionId(1),
                baseline_angle_degrees: 0.0,
                phase: 0.0,
                repetition: StraightGuideRepetition {
                    spacing_multiplier: 1.0,
                },
            },
            StraightGuideDimension {
                id: GuideDimensionId(2),
                baseline_angle_degrees: 90.0,
                phase: 0.0,
                repetition: StraightGuideRepetition {
                    spacing_multiplier: 1.0,
                },
            },
        ],
        GeneralizedSiteProduct::Intersections {
            dimensions: vec![GuideDimensionId(1), GuideDimensionId(2)],
            merge_epsilon: 0.0,
        },
        MarkOrientation::GuideNormal {
            dimension_id: GuideDimensionId(2),
        },
        CoveragePolicy {
            guard_steps: 1,
            additional_margin: 4.5,
        },
    );
    let PatternOutputRealization::MarkPrototype { prototype, .. } =
        &mut definition.output_layers[0].realization
    else {
        panic!("generalized guides own a typed output")
    };
    *prototype = MarkPrototype::AuthoredClosedShape {
        structure_id: AuthoredStructureId(7),
    };
    let points = [
        AuthoredPoint2 { x: -2.0, y: -2.0 },
        AuthoredPoint2 { x: 2.0, y: 2.0 },
        AuthoredPoint2 { x: -2.0, y: 2.0 },
        AuthoredPoint2 { x: 2.0, y: -2.0 },
    ];
    let shape = AuthoredStructure::new(
        AuthoredStructureId(7),
        AuthoredStructureKind::ClosedShape,
        (0..points.len())
            .map(|index| AuthoredCurveSegment::Line {
                start: points[index],
                end: points[(index + 1) % points.len()],
            })
            .collect(),
    )
    .unwrap();
    let document = Document::with_source_topology_and_authored_structures(
        base.id(),
        base.canvas().clone(),
        base.source().clone(),
        vec![{
            let mut bundle = base.pattern_definition_bundles()[0].clone();
            bundle.definition = definition;
            bundle
        }],
        base.pattern_settings().clone(),
        base.channel_model().unwrap(),
        base.channel_topology().unwrap().clone(),
        vec![shape],
    )
    .unwrap();
    let sources = SourceBundle::new([EmbeddedSource::new(
        source_id,
        EmbeddedSourceFormat::Png,
        fs::read("../../assets/raster-sample.png").unwrap(),
        Some("raster-sample.png".into()),
    )
    .unwrap()])
    .unwrap();
    (document, sources)
}

/// Reads the canonical document JSON member without invoking a compatibility decoder.
fn document_json(path: &PathBuf) -> String {
    let file = fs::File::open(path).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    let mut document = archive.by_name("document.json").unwrap();
    let mut bytes = Vec::new();
    std::io::Read::read_to_end(&mut document, &mut bytes).unwrap();
    String::from_utf8(bytes).unwrap()
}

/// Proves document-v5 persists the typed stable reference and exact closed shape deterministically,
/// then reconstructs the same validated document without a migration report or runtime geometry.
#[test]
fn authored_shape_document_v5_round_trip_is_deterministic_and_additive() {
    assert_eq!(DOCUMENT_SCHEMA_VERSION, 5);
    let (document, sources) = shape_document();
    let first = temporary("shape-first.toniator");
    let second = temporary("shape-second.toniator");
    save(&first, &document, &sources).unwrap();
    save(&second, &document, &sources).unwrap();
    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
    let json = document_json(&first);
    assert!(json.contains("\"authored_closed_shape\":{\"structure_id\":7}"));
    assert!(json.contains("\"kind\":\"closed_shape\""));
    assert!(!json.contains("canonical_marks"));
    let loaded = load(&first).unwrap();
    assert_eq!(loaded.document(), &document);
    fs::remove_file(first).unwrap();
    fs::remove_file(second).unwrap();
}
