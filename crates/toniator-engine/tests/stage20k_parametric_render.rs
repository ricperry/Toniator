use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};

use toniator_domain::{
    CanvasSpec, ConnectedGeometryResponse, CoveragePolicy, CurveWinding, Document, DocumentCommand,
    DocumentHistory, DocumentId, DocumentSession, GuideRepetition, MarkGeometryResponse,
    MarkOrientation, MarkPrototype, ParametricCurve, PatternDefinition, PatternDefinitionBundle,
    PatternDefinitionEdit, PatternDefinitionId, PatternFamily, PatternGeometryResponse,
    PatternMechanism, PatternMechanismId, PatternModulation, PatternOutputLayer,
    PatternOutputLayerId, PatternOutputRealization, PatternOutputSettings, SourceReference,
    SourceReferenceId, SpiralCurve, SpiralShape,
};
use toniator_engine::{
    CacheDisposition, EvaluationCompletion, EvaluationLimits, EvaluationRequest,
    EvaluationScheduler, ResolvedSource, SourceFormatHint, evaluate_with_limits,
};
use toniator_render::{encode_png, write_svg};

const ARTBOARD_SPIRAL_TURNS: f64 = 5.0;

/// Carries the bounded spiral values chosen by one render or cache fixture.
#[derive(Clone, Copy)]
struct SpiralFixtureSettings {
    turns: f64,
    radial_spacing: f64,
    site_interval: f64,
}

impl SpiralFixtureSettings {
    /// Builds a five-turn fixture whose round or square cadence spans the complete artboard.
    fn artboard(width: f64, height: f64, shape: SpiralShape) -> Self {
        let radial_spacing = match shape {
            SpiralShape::Round => {
                let corner_radius = width.hypot(height) * 0.5;
                corner_radius * 1.05 / (ARTBOARD_SPIRAL_TURNS - 1.0)
            }
            SpiralShape::Square => width.min(height) * 0.25,
        };
        Self {
            turns: ARTBOARD_SPIRAL_TURNS,
            radial_spacing,
            site_interval: radial_spacing * 0.25,
        }
    }

    /// Builds the small one-turn fixture used only by cache-identity scheduling checks.
    const fn cache() -> Self {
        Self {
            turns: 1.0,
            radial_spacing: 96.0,
            site_interval: 72.0,
        }
    }
}

/// Builds one current modeled document whose base definition produces either raw paths or equal-arc sites.
fn document(
    width: f64,
    height: f64,
    source_id: SourceReferenceId,
    shape: SpiralShape,
    sites: bool,
    fixture: SpiralFixtureSettings,
) -> Document {
    let base = Document::new_default_document(
        CanvasSpec { width, height },
        SourceReference::Assigned(source_id),
    )
    .expect("default document");
    let definition_id = PatternDefinitionId(810);
    let curve_id = PatternMechanismId(811);
    let site_id = PatternMechanismId(812);
    let definition = PatternDefinition {
        id: definition_id,
        name: "stage20k spiral".into(),
        family: PatternFamily::ParametricCurve {
            curve_mechanism_id: curve_id,
            site_mechanism_id: sites.then_some(site_id),
        },
        mechanisms: {
            let mut mechanisms = vec![PatternMechanism::ParametricCurveSource {
                id: curve_id,
                curve: ParametricCurve::Spiral(SpiralCurve {
                    shape,
                    turns: fixture.turns,
                    radial_spacing: fixture.radial_spacing,
                    phase_degrees: 0.0,
                    winding: CurveWinding::CounterClockwise,
                }),
                repetition: GuideRepetition::Single,
            }];
            if sites {
                mechanisms.push(PatternMechanism::AlongParametricCurveSites {
                    id: site_id,
                    curve_mechanism_id: curve_id,
                    interval: fixture.site_interval,
                    phase: 0.0,
                });
            }
            mechanisms
        },
        output_layers: if sites {
            vec![PatternOutputLayer::all(
                PatternOutputLayerId(813),
                PatternOutputRealization::MarkPrototype {
                    site_mechanism_id: site_id,
                    prototype: MarkPrototype::Circle,
                    orientation: MarkOrientation::Fixed,
                },
            )]
        } else {
            vec![PatternOutputLayer::all(
                PatternOutputLayerId(813),
                PatternOutputRealization::ParametricPaths {
                    curve_mechanism_id: curve_id,
                    style: toniator_domain::PathStrokeStyle::default(),
                },
            )]
        },
        modulation: PatternModulation,
        coverage: CoveragePolicy {
            guard_steps: 1,
            additional_margin: 0.0,
        },
    };
    let mut settings = base.pattern_settings().clone();
    settings.definition_id = definition_id;
    let response = if sites {
        PatternGeometryResponse::Marks(MarkGeometryResponse {
            minimum_fill: 0.02,
            maximum_fill: 0.28,
        })
    } else {
        PatternGeometryResponse::Connected(ConnectedGeometryResponse {
            minimum_thickness: 0.04,
            maximum_thickness: 0.28,
            bias: 0.0,
        })
    };
    Document::with_source_topology_and_authored_structures(
        DocumentId(809),
        base.canvas().clone(),
        base.source().clone(),
        vec![PatternDefinitionBundle {
            definition,
            output_settings: vec![PatternOutputSettings {
                output_layer_id: PatternOutputLayerId(813),
                response,
            }],
        }],
        settings,
        base.channel_model().expect("model"),
        base.channel_topology().expect("topology").clone(),
        Vec::new(),
    )
    .expect("parametric document")
}

/// Evaluates one immutable artwork and writes canonical PNG/SVG evidence without mutating inputs.
fn render_artwork(
    input_name: &str,
    width: f64,
    height: f64,
    hint: SourceFormatHint,
    shape: SpiralShape,
    sites: bool,
    stem: &str,
) {
    let source_id = SourceReferenceId::new(format!("stage20k-{stem}")).expect("source ID");
    let input = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets")
        .join(input_name);
    let bytes = fs::read(input).expect("immutable artwork reads");
    let session = DocumentSession::new(document(
        width,
        height,
        source_id.clone(),
        shape,
        sites,
        SpiralFixtureSettings::artboard(width, height, shape),
    ))
    .expect("document session");
    let output = evaluate_with_limits(
        EvaluationRequest::new(
            session.document_evaluation_snapshot(),
            ResolvedSource::new(source_id, bytes, hint).expect("resolved source"),
        ),
        EvaluationLimits::new(1_048_576).expect("limits"),
    )
    .expect("authoritative evaluation");
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/validation/stage-20k");
    fs::create_dir_all(&directory).expect("validation directory");
    fs::write(
        directory.join(format!("{stem}.png")),
        encode_png(output.raster()).expect("PNG encodes"),
    )
    .expect("PNG writes");
    fs::write(
        directory.join(format!("{stem}.svg")),
        write_svg(output.scene()),
    )
    .expect("SVG writes");
}

/// Proves the evidence preset carries a full final turn beyond both immutable artwork bounds.
#[test]
fn evidence_spiral_settings_cover_both_immutable_artboards() {
    for (width, height) in [(1024.0_f64, 1024.0_f64), (900.0_f64, 620.0_f64)] {
        let corner_radius = width.hypot(height) * 0.5;
        let round = SpiralFixtureSettings::artboard(width, height, SpiralShape::Round);
        let penultimate_radius = (round.turns - 1.0) * round.radial_spacing;
        assert!(
            penultimate_radius >= corner_radius,
            "the penultimate evidence turn must enclose every artboard corner"
        );
        let square = SpiralFixtureSettings::artboard(width, height, SpiralShape::Square);
        assert!(
            2.0 * square.radial_spacing >= width.min(height) * 0.5,
            "a complete square evidence turn must reach the shorter artboard edges"
        );
    }
}

/// Waits for one current scheduler completion without assuming worker timing.
fn wait_for_completion(scheduler: &EvaluationScheduler) -> EvaluationCompletion {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Some(completion) = scheduler
            .try_receive_latest()
            .expect("scheduler receive succeeds")
        {
            return completion;
        }
        assert!(Instant::now() < deadline, "parametric evaluation timed out");
        std::thread::yield_now();
    }
}

/// Proves parametric family and transform-stack intent participate in the document family cache.
#[test]
fn parametric_transform_stack_edits_miss_authoritative_family_cache_entries() {
    let source_id = SourceReferenceId::new("stage20k-cache").expect("source ID");
    let document = document(
        160.0,
        120.0,
        source_id.clone(),
        SpiralShape::Round,
        true,
        SpiralFixtureSettings::cache(),
    );
    let mut history = DocumentHistory::new(DocumentSession::new(document).expect("session"));
    let source =
        fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/raster-sample.png"))
            .expect("immutable raster source reads");
    let scheduler =
        EvaluationScheduler::new_with_limits(EvaluationLimits::new(1_048_576).expect("limits"))
            .expect("scheduler starts");
    let submit = |history: &DocumentHistory| {
        let ticket = scheduler
            .submit(EvaluationRequest::new(
                history.session().document_evaluation_snapshot(),
                ResolvedSource::new(source_id.clone(), source.clone(), SourceFormatHint::Png)
                    .expect("resolved source"),
            ))
            .expect("submit succeeds");
        let completion = wait_for_completion(&scheduler);
        assert_eq!(completion.ticket(), ticket);
        assert!(
            completion.result().is_some(),
            "evaluation succeeds: {:?}",
            completion.error()
        );
        assert!(
            scheduler
                .accept_completion(&completion, history.session())
                .expect("completion accepts")
        );
        completion
    };
    submit(&history);
    let repeated = submit(&history);
    assert_eq!(
        repeated
            .cache_diagnostics()
            .expect("diagnostics")
            .aggregate
            .family,
        CacheDisposition::Hit
    );
    let base_definition = history
        .document()
        .pattern_definition_bundles()
        .iter()
        .find(|bundle| bundle.definition.id == PatternDefinitionId(810))
        .expect("parametric definition")
        .definition
        .clone();
    history
        .apply(&DocumentCommand::EditSharedPatternDefinition {
            definition_id: PatternDefinitionId(810),
            base_definition,
            edit: PatternDefinitionEdit::SetParametricRepetition {
                mechanism_id: PatternMechanismId(811),
                repetition: GuideRepetition::TransformStack {
                    direction_degrees: 0.0,
                    spacing_multiplier: 1.25,
                },
            },
        })
        .expect("transform stack edit applies");
    let changed = submit(&history);
    assert!(
        changed
            .cache_diagnostics()
            .expect("diagnostics")
            .channels
            .iter()
            .all(|channel| channel.family == CacheDisposition::Miss)
    );
    scheduler.shutdown().expect("scheduler stops");
}

/// Exercises raw connected paths and equal-arc sites for each shape against both immutable artworks.
#[test]
fn renders_all_parametric_products_for_both_immutable_artworks() {
    for (input, width, height, hint, source) in [
        (
            "raster-sample.png",
            1024.0,
            1024.0,
            SourceFormatHint::Png,
            "raster",
        ),
        (
            "vector-sample.svg",
            900.0,
            620.0,
            SourceFormatHint::Svg,
            "vector",
        ),
    ] {
        for (shape, shape_name) in [
            (SpiralShape::Round, "round"),
            (SpiralShape::Square, "square"),
        ] {
            for (sites, product) in [(true, "sites"), (false, "path")] {
                render_artwork(
                    input,
                    width,
                    height,
                    hint,
                    shape,
                    sites,
                    &format!("{shape_name}-{product}-{source}"),
                );
            }
        }
    }
}
