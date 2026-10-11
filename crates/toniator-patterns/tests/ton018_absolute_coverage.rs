use toniator_patterns::{CoverageCurve, CoverageKnot, GeometryResponseInput, PresetRegistry};

/// Keeps desired coverage in absolute units through low tones, saturation, and raw probes.
///
/// # Panics
/// Panics if a requested fraction is normalized to the reachable span or applied twice.
#[test]
fn desired_coverage_inverts_absolute_fraction() {
    let curve = CoverageCurve::new(vec![
        CoverageKnot {
            response: 0.0,
            coverage: 0.0,
        },
        CoverageKnot {
            response: 1.0,
            coverage: 0.8,
        },
    ])
    .unwrap();
    let input = GeometryResponseInput::AbsoluteCoverage {
        curve: &curve,
        minimum: 0.0,
        maximum: 1.0,
    };
    for (tone, raw) in [
        (0.0, 0.0),
        (0.000001, 0.0000025),
        (0.25, 0.625),
        (0.5, 1.25),
        (0.75, 1.875),
        (1.0, 2.0),
    ] {
        assert!((input.resolve_raw_size(tone, 0.0, 1.0).unwrap() - raw).abs() < 1e-12);
    }
    let equal = GeometryResponseInput::AbsoluteCoverage {
        curve: &curve,
        minimum: 0.5,
        maximum: 0.5,
    };
    assert_eq!(equal.resolve_raw_size(0.0, 0.0, 1.0).unwrap(), 1.25);
    assert_eq!(equal.resolve_raw_size(1.0, 0.0, 1.0).unwrap(), 1.25);
    assert_eq!(
        GeometryResponseInput::RawProbe(1.75)
            .resolve_raw_size(0.0, 0.0, 1.0)
            .unwrap(),
        1.75
    );
}

/// Checks the retained Clustered raw companion targets half the canvas, not half its endpoint span.
///
/// # Panics
/// Panics if production response treats desired .5 as a normalized fraction of C(1) or C(2).
#[test]
fn clustered_half_tone_targets_half_coverage() {
    let registry = PresetRegistry::bundled();
    let saved = registry
        .entries()
        .iter()
        .find(|record| record.metadata.id == "clustered-dispersion-random-links")
        .unwrap()
        .saved_calibration
        .as_ref()
        .unwrap();
    let curve = CoverageCurve::new(
        saved
            .raw()
            .iter()
            .map(|point| CoverageKnot {
                response: point.input / 2.0,
                coverage: point.coverage,
            })
            .collect(),
    )
    .unwrap();
    let raw = GeometryResponseInput::AbsoluteCoverage {
        curve: &curve,
        minimum: 0.0,
        maximum: 1.0,
    }
    .resolve_raw_size(0.5, 0.0, 1.0)
    .unwrap();
    let knots = saved.raw();
    let upper = knots.partition_point(|point| point.input < raw);
    let coverage = if knots[upper].input == raw {
        knots[upper].coverage
    } else {
        let left = &knots[upper - 1];
        let right = &knots[upper];
        left.coverage
            + (raw - left.input) / (right.input - left.input) * (right.coverage - left.coverage)
    };
    assert!(
        (coverage - 0.5).abs() < 1e-12,
        "raw={raw}, coverage={coverage}"
    );
    assert!(raw > 0.5 && raw < 1.0);
}
