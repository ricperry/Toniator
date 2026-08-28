use toniator_domain::{
    CanvasSpec, ChannelAppearance, ChannelId, ChannelPatternInstance, ChannelPatternLayoutDelta,
    ChannelSourceMapping, ChannelState, ColorValue, CoveragePolicy, DensityMetric2D, Document,
    DocumentId, DocumentPatternSettings, DocumentSession, MarkGeometryResponse, PatternDefinition,
    PatternDefinitionBundle, PatternDefinitionId, PatternGeometryResponse, PatternMechanismId,
    PatternOutputLayerId, PatternOutputSettings, SourceComponent, SourcePlacement, SourceReference,
    SourceReferenceId,
};
use toniator_engine::{
    ChannelDiagnosticRequest, EvaluationLimits, GeometryOutput, ResolvedSource, SourceFormatHint,
    evaluate_channel_diagnostic, evaluate_channel_diagnostic_with_limits, write_svg,
};

const CHANNEL_ID: ChannelId = ChannelId(1);

/// Builds current document-owned pattern settings whose resolved values match
/// the diagnostic fixture without storing an obsolete per-channel layout.
fn pattern_settings(rotation_degrees: f64) -> DocumentPatternSettings {
    DocumentPatternSettings {
        definition_id: PatternDefinitionId(1),
        density: DensityMetric2D {
            density: 5_400.0_f64.sqrt(),
            aspect: 1.0,
        },
        pattern_rotation_degrees: rotation_degrees,
        shape_rotation_degrees: 0.0,
    }
}

/// Builds a channel that inherits every family and realization setting while
/// retaining only channel-owned appearance and source mapping intent.
fn channel(
    component: SourceComponent,
    translation_x: f64,
    translation_y: f64,
    appearance: ChannelAppearance,
) -> ChannelState {
    ChannelState {
        id: CHANNEL_ID,
        pattern_instance: ChannelPatternInstance {
            definition_override: None,
            layout_delta: ChannelPatternLayoutDelta {
                density: None,
                rotation_degrees: None,
                translation_x,
                translation_y,
            },
            shape_rotation_delta_degrees: None,
            output_response_deltas: Vec::new(),
        },
        appearance,
        source_mapping: ChannelSourceMapping {
            component,
            placement: SourcePlacement::StretchToCanvas,
        },
    }
}

/// Builds the retained legacy diagnostic request used by render identity regression witnesses.
fn request(bytes: Vec<u8>, format: SourceFormatHint) -> ChannelDiagnosticRequest {
    let source_id = SourceReferenceId::new("baseline-source").unwrap();
    let document = Document::with_source(
        DocumentId(1),
        CanvasSpec {
            width: 900.0,
            height: 600.0,
        },
        SourceReference::Assigned(source_id.clone()),
        vec![mark_bundle()],
        pattern_settings(17.0),
        vec![channel(
            SourceComponent::Luminance,
            3.25,
            -4.5,
            ChannelAppearance {
                visible: true,
                color: ColorValue {
                    red: 0.0,
                    green: toniator_engine::srgb_to_linear(183.0 / 255.0),
                    blue: 1.0,
                    alpha: 1.0,
                },
                opacity: 0.72,
            },
        )],
    )
    .unwrap();
    let session = DocumentSession::new(document).unwrap();
    ChannelDiagnosticRequest::new(
        session.evaluation_snapshot(CHANNEL_ID).unwrap(),
        ResolvedSource::new(source_id, bytes, format).unwrap(),
    )
}

/// Builds the current typed structural definition and its document-owned mark response.
fn mark_bundle() -> PatternDefinitionBundle {
    PatternDefinitionBundle {
        definition: PatternDefinition::supported_straight_grid(
            PatternDefinitionId(1),
            "straight-grid",
            PatternMechanismId(1),
            PatternMechanismId(2),
            PatternOutputLayerId(1),
            CoveragePolicy {
                guard_steps: 2,
                additional_margin: 4.5,
            },
        ),
        output_settings: vec![PatternOutputSettings {
            output_layer_id: PatternOutputLayerId(1),
            response: PatternGeometryResponse::Marks(MarkGeometryResponse {
                minimum_fill: 0.2,
                maximum_fill: 0.9,
            }),
        }],
    }
}

/// Verifies current document authority yields exact deterministic circle diagnostics for both
/// immutable source inputs, including source-sensitive realization and scene identities.
#[test]
fn document_derived_evaluation_has_current_deterministic_circle_geometry() {
    for (path, format, decoded, family, realization, scene) in [
        (
            "../../assets/raster-sample.png",
            SourceFormatHint::Png,
            "sha256:2840ac64a71451469ed2b90b797b284f57ceace284cd986a46e66b6ef82b6ee8",
            "fnv1a64:28e1d1b546dd8156:nominal-cell-basis:fnv1a64:58c64d923bc54a00",
            "fnv1a64:9ff83748f59c55ad",
            "fnv1a64:593c8be10bba1676",
        ),
        (
            "../../assets/vector-sample.svg",
            SourceFormatHint::Svg,
            "sha256:cf28b0ab640991969d9a5936be85dfd552867125950362495c69b1ab99f94fb7",
            "fnv1a64:28e1d1b546dd8156:nominal-cell-basis:fnv1a64:58c64d923bc54a00",
            "fnv1a64:9186a6071d7e1b8b",
            "fnv1a64:545d6661187e377b",
        ),
    ] {
        let result =
            evaluate_channel_diagnostic(request(std::fs::read(path).unwrap(), format)).unwrap();
        assert_eq!(result.scene().identity().family_fingerprint(), family);
        assert_eq!(
            result.scene().identity().realization_fingerprint(),
            realization
        );
        assert_eq!(result.scene().identity().scene_fingerprint(), scene);
        assert_eq!(result.source_identity().decoded_pixel_hash, decoded);
        let GeometryOutput::CircularMarks(marks) = result.scene().layers()[0].geometry() else {
            panic!("fixture must retain the circle diagnostic adapter");
        };
        assert_eq!(marks.len(), 6_830);
        let guard_count = marks
            .iter()
            .filter(|mark| mark.scope == toniator_engine::SiteScope::Guard)
            .count();
        assert_eq!(guard_count, 1_428);
        assert!(
            marks
                .windows(2)
                .all(|pair| pair[0].source_site_id != pair[1].source_site_id)
        );
        assert_eq!(
            (result.raster().width(), result.raster().height()),
            (900, 600)
        );
        assert!(write_svg(result.scene()).contains("<circle "));
    }
}

/// Confirms source identity mismatch fails before decoding or geometry work.
#[test]
fn source_mismatch_is_rejected_before_decode_or_geometry() {
    let source_id = SourceReferenceId::new("snapshot-source").unwrap();
    let other_id = SourceReferenceId::new("different-source").unwrap();
    let document = Document::with_source(
        DocumentId(1),
        CanvasSpec {
            width: 900.0,
            height: 600.0,
        },
        SourceReference::Assigned(source_id),
        vec![mark_bundle()],
        pattern_settings(0.0),
        vec![channel(
            SourceComponent::Luminance,
            0.0,
            0.0,
            ChannelAppearance {
                visible: true,
                color: ColorValue {
                    red: 0.0,
                    green: 0.0,
                    blue: 0.0,
                    alpha: 1.0,
                },
                opacity: 1.0,
            },
        )],
    )
    .unwrap();
    let session = DocumentSession::new(document).unwrap();
    let error = evaluate_channel_diagnostic(ChannelDiagnosticRequest::new(
        session.evaluation_snapshot(CHANNEL_ID).unwrap(),
        ResolvedSource::new(other_id, vec![1_u8], SourceFormatHint::Png).unwrap(),
    ))
    .expect_err("mismatched reference fails before invalid PNG decode");
    assert_eq!(error.path(), "evaluation.source_reference");
}

/// Confirms an unassigned authoritative source rejects a resolved payload.
#[test]
fn unassigned_source_reference_fails_at_the_authoritative_boundary() {
    let document = Document::new(
        DocumentId(1),
        CanvasSpec {
            width: 900.0,
            height: 600.0,
        },
        vec![mark_bundle()],
        pattern_settings(0.0),
        vec![channel(
            SourceComponent::Alpha,
            0.0,
            0.0,
            ChannelAppearance {
                visible: true,
                color: ColorValue {
                    red: 0.0,
                    green: 0.0,
                    blue: 0.0,
                    alpha: 1.0,
                },
                opacity: 1.0,
            },
        )],
    )
    .unwrap();
    let session = DocumentSession::new(document).unwrap();
    let error = evaluate_channel_diagnostic(ChannelDiagnosticRequest::new(
        session.evaluation_snapshot(CHANNEL_ID).unwrap(),
        ResolvedSource::new(
            SourceReferenceId::new("resolved").unwrap(),
            vec![1_u8],
            SourceFormatHint::Png,
        )
        .unwrap(),
    ))
    .expect_err("unassigned source fails before decode");
    assert_eq!(error.path(), "evaluation.source_reference");
}

/// Confirms candidate limits reject before oversized family allocation.
#[test]
fn default_and_custom_candidate_limits_fail_before_oversized_family_allocation() {
    assert_eq!(
        EvaluationLimits::default().max_family_candidates(),
        1_048_576
    );
    assert_eq!(
        EvaluationLimits::new(0)
            .expect_err("zero is invalid")
            .path(),
        "coverage.candidate_limit"
    );
    let bytes = std::fs::read("../../assets/raster-sample.png").unwrap();
    let error = evaluate_channel_diagnostic_with_limits(
        request(bytes.clone(), SourceFormatHint::Png),
        EvaluationLimits::new(1).unwrap(),
    )
    .expect_err("one candidate cannot cover the requested grid");
    assert_eq!(error.path(), "coverage.candidate_limit");
    assert!(
        evaluate_channel_diagnostic_with_limits(
            request(bytes, SourceFormatHint::Png),
            EvaluationLimits::new(100_000).unwrap(),
        )
        .is_ok()
    );
}
