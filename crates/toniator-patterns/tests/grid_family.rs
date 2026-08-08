use std::collections::BTreeSet;

use toniator_domain::{CanvasSpec, DensityMetric2D};
use toniator_geometry::{SiteId, Vector2};
use toniator_patterns::{
    ANTIALIAS_MARGIN, GridInspectRequest, directional_spacing, evaluate_straight_grid,
};

fn request(rotation_degrees: f64, translation_x: f64, translation_y: f64) -> GridInspectRequest {
    GridInspectRequest {
        canvas: CanvasSpec {
            width: 900.0,
            height: 600.0,
        },
        density: DensityMetric2D {
            across_x: 90.0,
            across_y: 60.0,
            aspect_locked: true,
        },
        rotation_degrees,
        translation_x,
        translation_y,
        guard_steps: 2,
        support_radius: 4.5,
        max_family_candidates: 1_048_576,
    }
}

#[test]
fn resolves_reference_spacing_and_inclusive_guide_ranges() {
    let output = evaluate_straight_grid(&request(0.0, 0.0, 0.0)).expect("valid family");

    assert_eq!(output.coverage[0].spacing, 10.0);
    assert_eq!(output.coverage[1].spacing, 10.0);
    assert_eq!(output.coverage[0].first_index, -3);
    assert_eq!(output.coverage[0].last_index, 93);
    assert_eq!(output.coverage[1].first_index, -3);
    assert_eq!(output.coverage[1].last_index, 63);
    assert_eq!(output.guides.len(), 164);
    assert_eq!(output.antialias_margin, ANTIALIAS_MARGIN);
}

#[test]
fn directional_frequency_uses_the_authored_axis_density() {
    let mut anisotropic = request(0.0, 0.0, 0.0);
    anisotropic.density = DensityMetric2D {
        across_x: 180.0,
        across_y: 60.0,
        aspect_locked: false,
    };
    let output = evaluate_straight_grid(&anisotropic).expect("valid family");

    assert_eq!(output.coverage[0].spacing, 5.0);
    assert_eq!(output.coverage[1].spacing, 10.0);
    let diagonal = directional_spacing(
        &anisotropic.canvas,
        &anisotropic.density,
        Vector2::new(2_f64.sqrt().recip(), 2_f64.sqrt().recip()),
    )
    .expect("valid diagonal frequency");
    assert!((diagonal - 6.324_555_320_336_759).abs() < 1e-12);
}

#[test]
fn rotations_translations_and_phase_are_deterministic() {
    for rotation in [0.0, 17.0, 45.0, 89.5, 137.0] {
        for (x, y) in [(0.0, 0.0), (3.25, -4.5), (-6.75, 8.25), (23.25, -24.5)] {
            let first = evaluate_straight_grid(&request(rotation, x, y)).expect("valid family");
            let second = evaluate_straight_grid(&request(rotation, x, y)).expect("valid family");
            assert_eq!(first, second, "rotation={rotation}, translation=({x}, {y})");
            assert!(first.has_only_finite_geometry());
            assert!(first.coverage.iter().all(|coverage| {
                coverage.normalized_phase >= 0.0 && coverage.normalized_phase < coverage.spacing
            }));
        }
    }

    let zero = evaluate_straight_grid(&request(0.0, 0.0, 0.0)).expect("valid family");
    let multi_period = evaluate_straight_grid(&request(0.0, 20.0, -30.0)).expect("valid family");
    assert_eq!(
        zero.coverage[0].normalized_phase,
        multi_period.coverage[0].normalized_phase
    );
    assert_eq!(
        zero.coverage[1].normalized_phase,
        multi_period.coverage[1].normalized_phase
    );
    assert_ne!(zero.family_fingerprint, multi_period.family_fingerprint);
}

#[test]
fn sites_have_stable_ids_ordering_provenance_and_fingerprint() {
    let output = evaluate_straight_grid(&request(17.0, 3.25, -4.5)).expect("valid family");
    let ids: Vec<_> = output.sites.iter().map(|site| site.id).collect();
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    assert_eq!(ids, sorted);
    assert!(output.sites.iter().all(|site| {
        site.id.first_dimension_id == site.provenance.contributors[0].dimension_id
            && site.id.first_index == site.provenance.contributors[0].index
            && site.id.second_dimension_id == site.provenance.contributors[1].dimension_id
            && site.id.second_index == site.provenance.contributors[1].index
    }));
    assert!(output.family_fingerprint.starts_with("fnv1a64:"));
}

#[test]
fn support_envelope_is_complete_and_bounded_across_rotation_translation_and_anisotropy() {
    let mut anisotropic = request(45.0, -6.75, 8.25);
    anisotropic.density = DensityMetric2D {
        across_x: 180.0,
        across_y: 40.0,
        aspect_locked: false,
    };
    for input in [
        request(0.0, 0.0, 0.0),
        request(17.0, 3.25, -4.5),
        request(45.0, 23.25, -24.5),
        request(89.5, -6.75, 8.25),
        anisotropic,
    ] {
        let output = evaluate_straight_grid(&input).expect("valid family");
        let emitted: BTreeSet<_> = output.sites.iter().map(|site| site.id).collect();
        let spacing_x = input.canvas.width / input.density.across_x;
        let spacing_y = input.canvas.height / input.density.across_y;
        let envelope = planning_envelope(&input, spacing_x, spacing_y);

        assert!(output.sites.iter().all(|site| {
            distance_to_canvas(
                site.position.x,
                site.position.y,
                input.canvas.width,
                input.canvas.height,
            ) <= envelope + 1e-10
        }));
        for first_index in -220_i64..=220 {
            for second_index in -220_i64..=220 {
                let (x, y) =
                    candidate_position(&input, spacing_x, spacing_y, first_index, second_index);
                if distance_to_canvas(x, y, input.canvas.width, input.canvas.height)
                    <= envelope + 1e-10
                {
                    assert!(
                        emitted.contains(&SiteId {
                            first_dimension_id: 1,
                            first_index,
                            second_dimension_id: 2,
                            second_index,
                        }),
                        "missing envelope site ({first_index}, {second_index})"
                    );
                }
            }
        }
    }
}

#[test]
fn rotated_coverage_rectangle_omits_all_four_dead_cartesian_corners() {
    let input = request(17.0, 3.25, -4.5);
    let output = evaluate_straight_grid(&input).unwrap();
    let ids: BTreeSet<_> = output.sites.iter().map(|site| site.id).collect();
    let spacing_x = output.coverage[0].spacing;
    let spacing_y = output.coverage[1].spacing;
    let envelope = planning_envelope(&input, spacing_x, spacing_y);
    for first_index in [
        output.coverage[0].first_index,
        output.coverage[0].last_index,
    ] {
        for second_index in [
            output.coverage[1].first_index,
            output.coverage[1].last_index,
        ] {
            let (x, y) =
                candidate_position(&input, spacing_x, spacing_y, first_index, second_index);
            assert!(
                distance_to_canvas(x, y, input.canvas.width, input.canvas.height) > envelope,
                "coverage corner ({first_index}, {second_index}) must lie outside the rounded envelope"
            );
            assert!(
                !ids.contains(&SiteId {
                    first_dimension_id: 1,
                    first_index,
                    second_dimension_id: 2,
                    second_index,
                }),
                "coverage corner ({first_index}, {second_index}) must not be published"
            );
        }
    }
}

#[test]
fn support_envelope_retains_required_edge_sites_and_scopes_them_as_guards() {
    let output = evaluate_straight_grid(&request(0.0, 0.0, 0.0)).unwrap();
    let retained = output
        .sites
        .iter()
        .find(|site| {
            site.id
                == SiteId {
                    first_dimension_id: 1,
                    first_index: -2,
                    second_dimension_id: 2,
                    second_index: 30,
                }
        })
        .unwrap();
    assert_eq!(retained.scope, toniator_patterns::SiteScope::Guard);
    assert!(!output.sites.iter().any(|site| site.id
        == SiteId {
            first_dimension_id: 1,
            first_index: -3,
            second_dimension_id: 2,
            second_index: 30,
        }));
}

#[test]
fn rejects_invalid_or_nonfinite_values_before_generation() {
    let mut zero_width = request(0.0, 0.0, 0.0);
    zero_width.canvas.width = 0.0;
    assert_eq!(
        evaluate_straight_grid(&zero_width)
            .expect_err("invalid")
            .path(),
        "canvas.width"
    );

    let mut cases = vec![
        (
            request(0.0, 0.0, 0.0),
            "channel.pattern.layout.density.across_x",
        ),
        (
            request(0.0, 0.0, 0.0),
            "channel.pattern.layout.density.across_y",
        ),
        (
            request(0.0, 0.0, 0.0),
            "channel.pattern.layout.rotation_degrees",
        ),
        (
            request(0.0, 0.0, 0.0),
            "channel.pattern.layout.translation_x",
        ),
        (request(0.0, 0.0, 0.0), "coverage.support_radius"),
    ];
    cases[0].0.density.across_x = f64::NAN;
    cases[1].0.density.across_y = 0.0;
    cases[2].0.rotation_degrees = f64::INFINITY;
    cases[3].0.translation_x = f64::NAN;
    cases[4].0.support_radius = -0.1;
    for (invalid, path) in cases {
        assert_eq!(
            evaluate_straight_grid(&invalid)
                .expect_err("invalid")
                .path(),
            path
        );
    }
}

#[test]
fn candidate_limits_reject_before_family_allocation_and_range_arithmetic_is_checked() {
    let mut default_limited = request(17.0, 3.25, -4.5);
    default_limited.density = DensityMetric2D {
        across_x: 5_000.0,
        across_y: 5_000.0,
        aspect_locked: true,
    };
    assert_eq!(
        evaluate_straight_grid(&default_limited)
            .expect_err("the default policy rejects the multi-million candidate product")
            .path(),
        "coverage.candidate_limit"
    );

    let mut limited = request(17.0, 3.25, -4.5);
    limited.max_family_candidates = 1;
    assert_eq!(
        evaluate_straight_grid(&limited)
            .expect_err("the Cartesian guide range exceeds one candidate")
            .path(),
        "coverage.candidate_limit"
    );

    let mut zero = request(0.0, 0.0, 0.0);
    zero.max_family_candidates = 0;
    assert_eq!(
        evaluate_straight_grid(&zero)
            .expect_err("zero is not a policy")
            .path(),
        "coverage.candidate_limit"
    );
}

fn distance_to_canvas(x: f64, y: f64, width: f64, height: f64) -> f64 {
    let dx = if x < 0.0 {
        -x
    } else if x > width {
        x - width
    } else {
        0.0
    };
    let dy = if y < 0.0 {
        -y
    } else if y > height {
        y - height
    } else {
        0.0
    };
    dx.hypot(dy)
}

fn planning_envelope(input: &GridInspectRequest, spacing_x: f64, spacing_y: f64) -> f64 {
    input.support_radius
        + ANTIALIAS_MARGIN
        + f64::from(input.guard_steps) * spacing_x.max(spacing_y)
}

fn candidate_position(
    input: &GridInspectRequest,
    spacing_x: f64,
    spacing_y: f64,
    first_index: i64,
    second_index: i64,
) -> (f64, f64) {
    let radians = input.rotation_degrees.to_radians();
    let (cosine, sine) = (radians.cos(), radians.sin());
    let local_x = first_index as f64 * spacing_x - input.canvas.width / 2.0;
    let local_y = second_index as f64 * spacing_y - input.canvas.height / 2.0;
    (
        input.canvas.width / 2.0 + cosine * local_x - sine * local_y + input.translation_x,
        input.canvas.height / 2.0 + sine * local_x + cosine * local_y + input.translation_y,
    )
}
