//! Current default generic-guide family capacity and publication controls.
use std::sync::atomic::{AtomicBool, Ordering};
use toniator_domain::*;
use toniator_patterns::*;

/// Materializes an ordinary current generic recipe with one segmented path reused at0°/90°.
/// # Panics
/// Panics if the finite authored path or ID-free document command is rejected.
fn document(count: usize) -> Document {
    let base = Document::new_default_document(
        CanvasSpec {
            width: 100.0,
            height: 100.0,
        },
        SourceReference::Unassigned,
    )
    .unwrap();
    let segments = (0..count)
        .map(|i| AuthoredCurveSegment::Line {
            start: AuthoredPoint2 {
                x: -1.0 + 2.0 * i as f64 / count as f64,
                y: 0.0,
            },
            end: AuthoredPoint2 {
                x: -1.0 + 2.0 * (i + 1) as f64 / count as f64,
                y: 0.0,
            },
        })
        .collect();
    let resource = AuthoredStructureDraft::new(AuthoredStructureKind::OpenPath, segments).unwrap();
    let recipe = PatternDefinitionRecipe::marks(PatternStructureRecipe::AuthoredResources {
        resources: vec![resource],
        definition: Box::new(PatternStructureRecipe::GenericGuides {
            name: "segmented crossing".into(),
            coverage: CoveragePolicy {
                guard_steps: 1,
                additional_margin: 0.0,
            },
            dimensions: [0.0, 90.0]
                .into_iter()
                .map(|angle| GenericGuideDimensionDraft {
                    baseline_angle_degrees: angle,
                    phase: 0.0,
                    prototype: GenericGuidePrototypeDraft::AuthoredOpenPathReference {
                        resource_index: 0,
                    },
                    repetition: GuideRepetition::Single,
                })
                .collect(),
            product: GeneralizedSiteProductDraft::Intersections {
                dimension_indices: vec![0, 1],
                merge_epsilon: 1e-7,
            },
            orientation: MarkOrientationDraft::Fixed,
        }),
    });
    let settings = base.pattern_settings().clone();
    let definition = base
        .pattern_definition_bundles()
        .iter()
        .find(|b| b.definition.id == settings.definition_id)
        .unwrap()
        .definition
        .clone();
    let mut history = DocumentHistory::new(DocumentSession::new(base).unwrap());
    history
        .apply(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base: settings,
            base_definition: definition,
            recipe,
        })
        .unwrap();
    history.document().clone()
}

/// Resolves current document authority into the ordinary shared family producer request.
/// # Panics
/// Panics if current canvas/density or recipe capability resolution fails.
fn inputs(document: &Document, limit: usize) -> (FamilyCapability, GridInspectRequest) {
    let definition = &document
        .pattern_definition_bundles()
        .iter()
        .find(|b| b.definition.id == document.pattern_settings().definition_id)
        .unwrap()
        .definition;
    (
        resolve_document_pattern_pipeline(document, definition)
            .unwrap()
            .family,
        GridInspectRequest {
            canvas: document.canvas().clone(),
            density: document
                .pattern_settings()
                .density
                .resolve(document.canvas())
                .unwrap(),
            rotation_degrees: 0.0,
            translation_x: 0.0,
            translation_y: 0.0,
            guard_steps: 1,
            support_radius: 1.0,
            max_family_candidates: limit,
        },
    )
}

/// Retains every authored/extended segment while producing the single correct default family site.
/// # Panics
/// Panics if the old pair ceiling survives or source/provenance/topology is simplified.
#[test]
fn ordinary_segmented_guides_render_one_site_with_full_source_paths() {
    let document = document(513);
    let (family, request) = inputs(&document, usize::MAX);
    let output = evaluate_typed_family_product_with_curve_limits_progress_cancellable(
        &family,
        &request,
        None,
        CurveWorkLimits::default(),
        &|| false,
        &|_, _| {},
    )
    .unwrap();
    assert_eq!(output.site_set().sites().len(), 1);
    assert_eq!(output.structural_path_set().unwrap().paths().len(), 2);
    assert!(
        output
            .structural_path_set()
            .unwrap()
            .paths()
            .iter()
            .all(|guide| guide.path.segments().len() == 515)
    );
    let site = &output.site_set().sites()[0];
    assert!((site.position.x - 50.0).abs() < 1e-10 && (site.position.y - 50.0).abs() < 1e-10);
    assert!(
        matches!(&site.provenance,FamilySiteProvenance::CurveGuideIntersection{contributors} if contributors.len()==2)
    );
    let (_, finite) = inputs(&document, 1_000);
    assert_eq!(
        evaluate_typed_family_product_with_curve_limits_progress_cancellable(
            &family,
            &finite,
            None,
            CurveWorkLimits::default(),
            &|| false,
            &|_, _| {}
        )
        .unwrap_err()
        .path(),
        "coverage.curved_guides.pairwise_limit"
    );
}

/// Cancels after contact collection and before site merging/publication through real progress callbacks.
/// # Panics
/// Panics if obsolete family output escapes or ordinary recovery changes the accepted result.
#[test]
fn solved_family_contacts_cancel_before_site_publication() {
    let document = document(513);
    let (family, request) = inputs(&document, usize::MAX);
    let cancelled = AtomicBool::new(false);
    let result = evaluate_typed_family_product_with_curve_limits_progress_cancellable(
        &family,
        &request,
        None,
        CurveWorkLimits::default(),
        &|| cancelled.load(Ordering::Relaxed),
        &|completed, total| {
            if completed * 1000 / total >= 700 {
                cancelled.store(true, Ordering::Relaxed);
            }
        },
    );
    assert_eq!(result.unwrap_err().path(), "evaluation.cancelled");
    assert!(cancelled.load(Ordering::Relaxed));
    assert_eq!(
        evaluate_typed_family_product_with_curve_limits_progress_cancellable(
            &family,
            &request,
            None,
            CurveWorkLimits::default(),
            &|| false,
            &|_, _| {}
        )
        .unwrap()
        .site_set()
        .sites()
        .len(),
        1
    );
}
