use toniator_domain::{
    CanvasSpec, ChannelAppearance, ChannelId, ChannelPatternLayout, ChannelSourceMapping,
    ChannelState, ColorValue, CoveragePolicy, DensityMetric2D, Document, DocumentId,
    DocumentSession, MarkGeometryResponse, PatternDefinition, PatternDefinitionId,
    PatternMechanismId, PatternOutputLayerId, SourceComponent, SourcePlacement, SourceReference,
    SourceReferenceId,
};
use toniator_engine::{
    ChannelDiagnosticRequest, EvaluationLimits, GeometryOutput, ResolvedSource, SourceFormatHint,
    evaluate_channel_diagnostic, evaluate_channel_diagnostic_with_limits, write_svg,
};

const CHANNEL_ID: ChannelId = ChannelId(1);

fn request(bytes: Vec<u8>, format: SourceFormatHint) -> ChannelDiagnosticRequest {
    let source_id = SourceReferenceId::new("baseline-source").unwrap();
    let document = Document::with_source(
        DocumentId(1),
        CanvasSpec {
            width: 900.0,
            height: 600.0,
        },
        SourceReference::Assigned(source_id.clone()),
        vec![PatternDefinition::supported_straight_grid(
            PatternDefinitionId(1),
            "straight-grid",
            PatternMechanismId(1),
            PatternMechanismId(2),
            PatternOutputLayerId(1),
            CoveragePolicy {
                guard_steps: 2,
                maximum_support_radius: 4.5,
            },
        )],
        vec![ChannelState {
            id: CHANNEL_ID,
            pattern_definition_id: PatternDefinitionId(1),
            layout: ChannelPatternLayout {
                density: DensityMetric2D {
                    across_x: 90.0,
                    across_y: 60.0,
                    aspect_locked: true,
                },
                rotation_degrees: 17.0,
                translation_x: 3.25,
                translation_y: -4.5,
            },
            appearance: ChannelAppearance {
                visible: true,
                color: ColorValue {
                    red: 0.0,
                    green: toniator_engine::srgb_to_linear(183.0 / 255.0),
                    blue: 1.0,
                    alpha: 1.0,
                },
                opacity: 0.72,
            },
            mark_geometry_response: MarkGeometryResponse {
                minimum_size: 2.0,
                maximum_size: 9.0,
            },
            source_mapping: ChannelSourceMapping {
                component: SourceComponent::Luminance,
                placement: SourcePlacement::StretchToCanvas,
            },
        }],
    )
    .unwrap();
    let session = DocumentSession::new(document).unwrap();
    ChannelDiagnosticRequest::new(
        session.evaluation_snapshot(CHANNEL_ID).unwrap(),
        ResolvedSource::new(source_id, bytes, format).unwrap(),
    )
}

#[test]
fn document_derived_evaluation_matches_accepted_stage_five_identities_and_geometry() {
    for (path, format, realization, scene, decoded) in [
        (
            "../../assets/raster-sample.png",
            SourceFormatHint::Png,
            "fnv1a64:9db8b05a88a2c727",
            "fnv1a64:79d8d6ed11625502",
            "sha256:2840ac64a71451469ed2b90b797b284f57ceace284cd986a46e66b6ef82b6ee8",
        ),
        (
            "../../assets/vector-sample.svg",
            SourceFormatHint::Svg,
            "fnv1a64:d59cc5d53352afd5",
            "fnv1a64:c78b9c3d56c8d8cd",
            "sha256:cf28b0ab640991969d9a5936be85dfd552867125950362495c69b1ab99f94fb7",
        ),
    ] {
        let result =
            evaluate_channel_diagnostic(request(std::fs::read(path).unwrap(), format)).unwrap();
        assert_eq!(
            result.scene().identity().family_fingerprint(),
            "fnv1a64:87a8b213740ed5b9"
        );
        assert_eq!(
            result.scene().identity().realization_fingerprint(),
            realization
        );
        assert_eq!(result.scene().identity().scene_fingerprint(), scene);
        assert_eq!(result.source_identity().decoded_pixel_hash, decoded);
        let GeometryOutput::CircularMarks(marks) = result.scene().layers()[0].geometry();
        assert_eq!(marks.len(), 6_185);
        assert_eq!(
            marks
                .iter()
                .filter(|mark| mark.scope == toniator_engine::SiteScope::Guard)
                .count(),
            783
        );
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
        vec![PatternDefinition::supported_straight_grid(
            PatternDefinitionId(1),
            "straight-grid",
            PatternMechanismId(1),
            PatternMechanismId(2),
            PatternOutputLayerId(1),
            CoveragePolicy {
                guard_steps: 2,
                maximum_support_radius: 4.5,
            },
        )],
        vec![ChannelState {
            id: CHANNEL_ID,
            pattern_definition_id: PatternDefinitionId(1),
            layout: ChannelPatternLayout {
                density: DensityMetric2D {
                    across_x: 90.0,
                    across_y: 60.0,
                    aspect_locked: true,
                },
                rotation_degrees: 0.0,
                translation_x: 0.0,
                translation_y: 0.0,
            },
            appearance: ChannelAppearance {
                visible: true,
                color: ColorValue {
                    red: 0.0,
                    green: 0.0,
                    blue: 0.0,
                    alpha: 1.0,
                },
                opacity: 1.0,
            },
            mark_geometry_response: MarkGeometryResponse {
                minimum_size: 2.0,
                maximum_size: 9.0,
            },
            source_mapping: ChannelSourceMapping {
                component: SourceComponent::Luminance,
                placement: SourcePlacement::StretchToCanvas,
            },
        }],
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

#[test]
fn unassigned_source_reference_fails_at_the_authoritative_boundary() {
    let document = Document::new(
        DocumentId(1),
        CanvasSpec {
            width: 900.0,
            height: 600.0,
        },
        vec![PatternDefinition::supported_straight_grid(
            PatternDefinitionId(1),
            "straight-grid",
            PatternMechanismId(1),
            PatternMechanismId(2),
            PatternOutputLayerId(1),
            CoveragePolicy {
                guard_steps: 2,
                maximum_support_radius: 4.5,
            },
        )],
        vec![ChannelState {
            id: CHANNEL_ID,
            pattern_definition_id: PatternDefinitionId(1),
            layout: ChannelPatternLayout {
                density: DensityMetric2D {
                    across_x: 90.0,
                    across_y: 60.0,
                    aspect_locked: true,
                },
                rotation_degrees: 0.0,
                translation_x: 0.0,
                translation_y: 0.0,
            },
            appearance: ChannelAppearance {
                visible: true,
                color: ColorValue {
                    red: 0.0,
                    green: 0.0,
                    blue: 0.0,
                    alpha: 1.0,
                },
                opacity: 1.0,
            },
            mark_geometry_response: MarkGeometryResponse {
                minimum_size: 2.0,
                maximum_size: 9.0,
            },
            source_mapping: ChannelSourceMapping {
                component: SourceComponent::Alpha,
                placement: SourcePlacement::StretchToCanvas,
            },
        }],
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
