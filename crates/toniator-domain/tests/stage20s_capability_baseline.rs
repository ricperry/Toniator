use toniator_domain::{
    CanvasSpec, CoveragePolicy, CurveRepetition, CurveWinding, Document, DocumentCommand,
    ParametricCurve, ParametricCurveSiteDraft, PatternCapabilityFlag, PatternCapabilityScope,
    PatternDefinitionRecipe, PatternMechanism, PatternStructureRecipe, PropertyFieldId,
    PropertyTarget, SourceReference, SpiralCoveragePolicy, SpiralCurve, SpiralShape,
};

/// Projects a materialized parametric recipe without introducing a second control authority.
#[test]
fn parametric_recipe_projects_canonical_flags_and_scope_filtered_controls() {
    let document = Document::new_default_document(
        CanvasSpec {
            width: 120.0,
            height: 80.0,
        },
        SourceReference::Unassigned,
    )
    .expect("default document validates");
    let base = document.pattern_settings().clone();
    let base_definition = document.pattern_definition_bundles()[0].definition.clone();
    let recipe = PatternDefinitionRecipe::marks(PatternStructureRecipe::ParametricCurve {
        name: "Stage 20S parametric fixture".into(),
        coverage: CoveragePolicy {
            guard_steps: 2,
            additional_margin: 0.0,
        },
        curve: ParametricCurve::Spiral(SpiralCurve {
            shape: SpiralShape::Round,
            turns: 5.0,
            radial_spacing: 16.0,
            phase_degrees: 0.0,
            winding: CurveWinding::Clockwise,
        }),
        spiral_coverage: toniator_domain::SpiralCoveragePolicy::Fixed,
        repetition: CurveRepetition::Single,
        sites: Some(ParametricCurveSiteDraft {
            interval: 16.0,
            phase: 0.0,
        }),
    });
    let (candidate, _) = document
        .apply_command(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base,
            base_definition,
            recipe,
        })
        .expect("recipe materializes atomically");
    let base_projection = candidate
        .pattern_capabilities(PatternCapabilityScope::DocumentBase)
        .expect("base projection resolves");
    assert!(base_projection.supports_all(&[
        PatternCapabilityFlag::Parametric,
        PatternCapabilityFlag::AlongCurveSites,
        PatternCapabilityFlag::Marks,
    ]));
    assert!(
        base_projection
            .active_controls
            .iter()
            .all(|descriptor| descriptor.field != PropertyFieldId::SourceReference)
    );
    let channel_projection = candidate
        .pattern_capabilities(PatternCapabilityScope::Channel(toniator_domain::ChannelId(
            1,
        )))
        .expect("channel projection resolves effective definition");
    assert!(channel_projection.active_controls.iter().all(|descriptor| {
        !matches!(
            (descriptor.target, descriptor.field),
            (
                PropertyTarget::Channel(_),
                PropertyFieldId::TranslationX | PropertyFieldId::TranslationY
            )
        )
    }));
}

/// Materializes recipe-only `CoverCanvas` intent into finite turns that reach
/// every centered canvas corner while fixed recipe intent remains available.
#[test]
fn cover_canvas_spiral_recipe_derives_fixed_turns_for_both_baseline_canvases() {
    for canvas in [
        CanvasSpec {
            width: 1024.0,
            height: 1024.0,
        },
        CanvasSpec {
            width: 900.0,
            height: 620.0,
        },
    ] {
        let document = Document::new_default_document(canvas.clone(), SourceReference::Unassigned)
            .expect("baseline canvas document validates");
        let recipe = PatternDefinitionRecipe::marks(PatternStructureRecipe::ParametricCurve {
            name: "Cover canvas spiral".into(),
            coverage: CoveragePolicy {
                guard_steps: 2,
                additional_margin: 0.0,
            },
            curve: ParametricCurve::Spiral(SpiralCurve {
                shape: SpiralShape::Round,
                turns: 1.0,
                radial_spacing: 16.0,
                phase_degrees: 0.0,
                winding: CurveWinding::Clockwise,
            }),
            spiral_coverage: SpiralCoveragePolicy::CoverCanvas,
            repetition: CurveRepetition::Single,
            sites: Some(ParametricCurveSiteDraft {
                interval: 16.0,
                phase: 0.0,
            }),
        });
        let (candidate, _) = document
            .apply_command(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
                base: document.pattern_settings().clone(),
                base_definition: document.pattern_definition_bundles()[0].definition.clone(),
                recipe,
            })
            .expect("cover canvas recipe materializes atomically");
        let curve = candidate
            .pattern_definition_for(toniator_domain::ChannelId(1))
            .expect("materialized channel resolves its recipe definition")
            .mechanisms
            .iter()
            .find_map(|mechanism| match mechanism {
                PatternMechanism::ParametricCurveSource { curve, .. } => Some(curve),
                _ => None,
            })
            .expect("materialized definition retains a finite curve source");
        let ParametricCurve::Spiral(spiral) = curve;
        let corner_radius = (canvas.width * 0.5).hypot(canvas.height * 0.5);
        assert_eq!(spiral.turns, (corner_radius / 16.0).ceil() + 1.0);
        assert!(
            (spiral.turns - 1.0) * spiral.radial_spacing >= corner_radius,
            "the terminal complete revolution starts beyond every corner"
        );
    }
}
