use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use toniator_domain::{
    CanvasSpec, CoveragePolicy, CurveWinding, Document, DocumentId, GuideRepetition,
    MarkGeometryResponse, MarkOrientation, MarkPrototype, ParametricCurve, PatternDefinition,
    PatternDefinitionBundle, PatternDefinitionId, PatternFamily, PatternGeometryResponse,
    PatternMechanism, PatternMechanismId, PatternModulation, PatternOutputLayer,
    PatternOutputLayerId, PatternOutputRealization, PatternOutputSettings, SourceReference,
    SourceReferenceId, SpiralCurve, SpiralShape,
};
use toniator_io::{EmbeddedSource, EmbeddedSourceFormat, SourceBundle, load, save};

/// Returns one collision-resistant temporary current-format archive location.
fn temporary() -> PathBuf {
    std::env::temp_dir().join(format!(
        "toniator-stage20k-{}.toniator",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time follows epoch")
            .as_nanos(),
    ))
}

/// Persists analytic spiral intent deterministically without serializing derived paths, sites, or effective state.
#[test]
fn v5_round_trips_parametric_intent_deterministically_without_derived_geometry() {
    let source_id = SourceReferenceId::new("stage20k-source").expect("source id");
    let base = Document::new_default_document(
        CanvasSpec {
            width: 320.0,
            height: 240.0,
        },
        SourceReference::Assigned(source_id.clone()),
    )
    .expect("default document");
    let definition_id = PatternDefinitionId(701);
    let curve_id = PatternMechanismId(702);
    let site_id = PatternMechanismId(703);
    let definition = PatternDefinition {
        id: definition_id,
        name: "round spiral".into(),
        family: PatternFamily::ParametricCurve {
            curve_mechanism_id: curve_id,
            site_mechanism_id: Some(site_id),
        },
        mechanisms: vec![
            PatternMechanism::ParametricCurveSource {
                id: curve_id,
                curve: ParametricCurve::Spiral(SpiralCurve {
                    shape: SpiralShape::Round,
                    turns: 4.0,
                    radial_spacing: 24.0,
                    phase_degrees: 15.0,
                    winding: CurveWinding::CounterClockwise,
                }),
                repetition: GuideRepetition::Single,
            },
            PatternMechanism::AlongParametricCurveSites {
                id: site_id,
                curve_mechanism_id: curve_id,
                interval: 12.0,
                phase: 0.25,
            },
        ],
        output_layers: vec![PatternOutputLayer::all(
            PatternOutputLayerId(704),
            PatternOutputRealization::MarkPrototype {
                site_mechanism_id: site_id,
                prototype: MarkPrototype::Circle,
                orientation: MarkOrientation::Fixed,
            },
        )],
        modulation: PatternModulation,
        coverage: CoveragePolicy {
            guard_steps: 1,
            additional_margin: 0.0,
        },
    };
    let mut settings = base.pattern_settings().clone();
    settings.definition_id = definition_id;
    let document = Document::with_source_and_topology(
        DocumentId(700),
        base.canvas().clone(),
        base.source().clone(),
        vec![PatternDefinitionBundle {
            definition,
            output_settings: vec![PatternOutputSettings {
                output_layer_id: PatternOutputLayerId(704),
                response: PatternGeometryResponse::Marks(MarkGeometryResponse {
                    minimum_fill: 0.0,
                    maximum_fill: 1.0,
                }),
            }],
        }],
        settings,
        base.channel_model().expect("modeled channel model"),
        base.channel_topology().expect("modeled topology").clone(),
    )
    .expect("parametric document");
    let bundle = SourceBundle::new([EmbeddedSource::new(
        source_id,
        EmbeddedSourceFormat::Svg,
        b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>".to_vec(),
        None,
    )
    .expect("source")])
    .expect("bundle");
    let path = temporary();
    let second = temporary();
    save(&path, &document, &bundle).expect("save");
    let bytes = fs::read(&path).expect("archive reads");
    let reopened = load(&path).expect("load");
    save(&second, reopened.document(), reopened.sources()).expect("reopened save");
    assert_eq!(bytes, fs::read(&second).expect("reopened archive reads"));
    fs::remove_file(path).expect("temporary archive removes");
    fs::remove_file(second).expect("reopened temporary archive removes");
    assert_eq!(reopened.document(), &document);
    assert!(!String::from_utf8_lossy(&bytes).contains("curve_path"));
    assert!(!String::from_utf8_lossy(&bytes).contains("family_site"));
    assert!(!String::from_utf8_lossy(&bytes).contains("effective_channel_pattern"));
}
