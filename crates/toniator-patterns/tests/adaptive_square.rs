use toniator_domain::*;
use toniator_geometry::{CanonicalMark, CurveSegment};
use toniator_patterns::*;
use toniator_sampling::SourceField;

/// Materializes the public recipe and its full guard-inclusive native grid at 16px pitch.
/// # Panics
/// Panics if fixed document, recipe or structural evaluation fails.
fn fixture() -> (Document, PatternPipelinePlan, TypedFamilyOutput) {
    let original = Document::new_default_document(
        CanvasSpec {
            width: 64.0,
            height: 64.0,
        },
        SourceReference::Unassigned,
    )
    .unwrap();
    let document = original
        .apply_command(&DocumentCommand::ReplaceDocumentPatternDefinitionRecipe {
            base: original.pattern_settings().clone(),
            base_definition: original.pattern_definition_bundles()[0].definition.clone(),
            recipe: adaptive_square_marks_recipe(),
        })
        .unwrap()
        .0;
    let plan = resolve_document_pattern_pipeline_for_channel(
        &document,
        document.pattern_definition_for(ChannelId(1)).unwrap(),
        SourceWeighting::canonical(SourceMappingComponent::Alpha),
    )
    .unwrap();
    let request = GridInspectRequest {
        canvas: document.canvas().clone(),
        density: DensityMetric2D {
            density: 16.0,
            aspect: 1.0,
        }
        .resolve(document.canvas())
        .unwrap(),
        rotation_degrees: 0.0,
        translation_x: 0.0,
        translation_y: 0.0,
        guard_steps: 2,
        support_radius: 32.0,
        max_family_candidates: 100000,
    };
    let family = evaluate_typed_family_product_with_source_cancellable(
        &plan.family,
        &request,
        None,
        &|| false,
    )
    .unwrap();
    (document, plan, family)
}

/// Realizes an actual mapped source through the same compact canonical mark producer as exports.
/// # Panics
/// Panics on sampling, footprint bounds or canonical construction failure.
fn realize(
    source: &SourceField,
    paint: bool,
    input: GeometryResponseInput<'_>,
) -> CanonicalMarkRealization {
    let (document, plan, family) = fixture();
    realize_typed_canonical_marks_with_response_cancellable(
        &document,
        &family,
        &plan,
        source,
        document.canvas(),
        CanonicalMarkRequest {
            mapping: SourceMapping::canonical(SourceMappingComponent::Alpha),
            sampled_paint: paint,
            response: MarkResponse {
                minimum_fill: 0.0,
                maximum_fill: 1.0,
                rotation_offset_degrees: 0.0,
            },
            max_transformed_curve_segment_instances: 100000,
        },
        input,
        &|| false,
    )
    .unwrap()
    .output
}

/// Verifies reference pose in actual artwork coordinates, plus exact full squares and emptiness.
/// # Panics
/// Panics if guide tangent coordinates rotate the intended reference pose or inflate node counts.
#[test]
fn adaptive_native_reference_pose_and_compact_endpoints() {
    let source = SourceField::from_straight_rgba8(1, 1, vec![255, 0, 0, 255]).unwrap();
    let half = realize(&source, false, GeometryResponseInput::RawProbe(0.5));
    let (document, _, family) = fixture();
    let _ = document;
    let site = family
        .site_set()
        .sites()
        .iter()
        .find(|site| site.position.x == 32.0 && site.position.y == 32.0)
        .unwrap();
    let mark = half
        .marks
        .iter()
        .find(|mark| matches!(mark,CanonicalMark::ClosedPath(path) if path.source_site_id==site.id))
        .unwrap();
    let CanonicalMark::ClosedPath(path) = mark else {
        panic!("triangle")
    };
    let points = path
        .path
        .segments()
        .iter()
        .map(|segment| match segment {
            CurveSegment::Line(line) => line.start(),
            _ => panic!("line triangle"),
        })
        .collect::<Vec<_>>();
    assert_eq!(points.len(), 3);
    assert!(points[0].x < 32.0 && points[0].y < 32.0);
    assert!(points[1].x < 32.0 && points[1].y > 32.0);
    assert!(points[2].x > 32.0 && points[2].y > 32.0);
    assert!(
        realize(&source, false, GeometryResponseInput::RawProbe(0.0))
            .marks
            .is_empty()
    );
    assert!(realize(&source,false,GeometryResponseInput::RawProbe(1.0)).marks.iter().all(|mark|matches!(mark,CanonicalMark::ClosedPath(path) if path.path.segments().len()==4)));
    assert!(
        realize(
            &source,
            false,
            GeometryResponseInput::RawProbe(std::f64::consts::FRAC_PI_4)
        )
        .marks
        .iter()
        .all(|mark| matches!(mark, CanonicalMark::Circle { .. }))
    );
}

/// Keeps opaque corner ink when the cell center is transparent and averages associated RGB once.
/// # Panics
/// Panics if center-only sampling suppresses the mark or hidden transparent RGB contaminates paint.
#[test]
fn adaptive_source_color_uses_chosen_footprint_not_cell_center() {
    let mut pixels = vec![0; 64 * 64 * 4];
    for y in 0..64 {
        for x in 0..64 {
            let i = (y * 64 + x) * 4;
            pixels[i + 2] = 255;
            if (24..32).contains(&x) && (24..32).contains(&y) {
                pixels[i] = 255;
                pixels[i + 2] = 0;
                pixels[i + 3] = 255;
            }
        }
    }
    let source = SourceField::from_straight_rgba8(64, 64, pixels).unwrap();
    let output = realize(&source, true, GeometryResponseInput::Identity);
    assert!(!output.marks.is_empty());
    for paint in output.paints.unwrap() {
        assert!(paint.red > 0.999);
        assert!(paint.blue < 1e-12);
        assert_eq!(paint.alpha, 1.0);
    }
}

/// Proves indexed publication is identical under serial and concurrent execution.
/// # Panics
/// Panics if source scores, poses or canonical ordering vary with Rayon scheduling.
#[test]
fn adaptive_parallel_schedule_is_deterministic() {
    let source = SourceField::from_straight_rgba8(
        4,
        4,
        (0..16).flat_map(|i| [255, 0, 0, (i * 17) as u8]).collect(),
    )
    .unwrap();
    let one = rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build()
        .unwrap()
        .install(|| realize(&source, true, GeometryResponseInput::Identity));
    let four = rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .unwrap()
        .install(|| realize(&source, true, GeometryResponseInput::Identity));
    assert_eq!(one, four);
}
