//! Focused current native-AA checked Boolean behavior; no historical geometry fixtures.

use i_overlay::{
    core::{
        checked::{CheckedOverlayControl, CheckedOverlayError},
        fill_rule::FillRule,
        overlay::ShapeType,
        overlay_rule::OverlayRule,
        solver::{Precision, Solver},
    },
    float::overlay::{FloatOverlay, OverlayOptions},
    i_float::{adapter::FloatPointAdapter, float::rect::FloatRect},
};
use std::{
    cell::Cell,
    io::{Cursor, Read},
};

type Shapes = Vec<Vec<Vec<[f64; 2]>>>;

/// Reads an exact current 170-edge reduced maze witness captured from the normalized native union.
///
/// # Panics
/// Panics if the immutable focused fixture is truncated or has the wrong stream signature.
fn maze() -> (FloatRect<f64>, Shapes) {
    let mut reader = Cursor::new(include_bytes!("fixtures/ton018-maze-union.bin"));
    let mut word = || {
        let mut b = [0; 8];
        reader.read_exact(&mut b).unwrap();
        b
    };
    assert_eq!(word(), *b"AREAUNI1");
    let bounds = [
        f64::from_le_bytes(word()),
        f64::from_le_bytes(word()),
        f64::from_le_bytes(word()),
        f64::from_le_bytes(word()),
    ];
    let count = u64::from_le_bytes(word()) as usize;
    let mut shapes = Vec::new();
    for _ in 0..count {
        let contours = u64::from_le_bytes(word()) as usize;
        let mut shape = Vec::new();
        for _ in 0..contours {
            let points = u64::from_le_bytes(word()) as usize;
            let mut contour = Vec::new();
            for _ in 0..points {
                contour.push([f64::from_le_bytes(word()), f64::from_le_bytes(word())]);
            }
            shape.push(contour);
        }
        shapes.push(shape);
    }
    assert_eq!(shapes.len(), 5);
    assert_eq!(shapes.iter().flatten().map(Vec::len).sum::<usize>(), 170);
    (
        FloatRect::new(bounds[0], bounds[1], bounds[2], bounds[3]).unwrap(),
        shapes,
    )
}

/// Builds the current fixed-scale native adapter with output cleanup disabled.
///
/// # Panics
/// Panics if the finite fixture's checked adapter cannot be constructed.
fn overlay(rect: FloatRect<f64>, shapes: &Shapes, solver: Solver) -> FloatOverlay<[f64; 2], i64> {
    let adapter = FloatPointAdapter::try_with_scale_conservative(rect, 1e12).unwrap();
    let mut options = OverlayOptions::<f64, i64>::default();
    options.clean_result = false;
    options.min_output_area = 0.0;
    FloatOverlay::new_custom(
        adapter,
        options,
        Solver {
            precision: Precision::ABSOLUTE,
            ..solver
        },
        0,
    )
    .unsafe_add_source(shapes, ShapeType::Subject)
}

/// Proves all mature strategy selections recover the measured rounding-loop witness with bounded retries.
///
/// # Panics
/// Panics if retries do not finish at radius four or discard the accepted mutation certificate.
#[test]
fn reduced_maze_retries_original_geometry_and_converges() {
    let (rect, shapes) = maze();
    for solver in [Solver::AUTO, Solver::TREE, Solver::FRAG] {
        let mut overlay = overlay(rect, &shapes, solver);
        let allowance = toniator_render::RasterizationLimits::default().max_boolean_operations();
        let mut control = CheckedOverlayControl::new(allowance, &|| false);
        let (output, certificate) = overlay
            .overlay_checked(OverlayRule::Subject, FillRule::NonZero, &mut control)
            .unwrap();
        assert_eq!(output.len(), 1);
        assert_eq!(control.attempts(), 3);
        assert!(control.split_rounds() <= 48);
        assert!(certificate > 0);
        assert!(control.operations() < allowance);
        eprintln!(
            "strategy={:?} attempts={} rounds={} operations={} certificate={} finite_allowance={}",
            solver.strategy,
            control.attempts(),
            control.split_rounds(),
            control.operations(),
            certificate,
            allowance
        );
    }
}

/// Proves ordinary radius-one dispatch/order is retained by an accepted first attempt.
///
/// # Panics
/// Panics if checked output differs bytewise in f64 coordinates or changes shape ordering.
#[test]
fn healthy_radius_one_preserves_exact_output() {
    let rect = FloatRect::new(0., 10., 0., 10.).unwrap();
    let shapes = vec![
        vec![vec![[1., 1.], [8., 2.], [7., 8.], [2., 7.]]],
        vec![vec![[3., 0.], [9., 3.], [6., 9.]]],
    ];
    for solver in [Solver::AUTO, Solver::TREE, Solver::FRAG] {
        let expected =
            overlay(rect, &shapes, solver).overlay(OverlayRule::Subject, FillRule::NonZero);
        let mut control = CheckedOverlayControl::new(4_000_000, &|| false);
        let (actual, _) = overlay(rect, &shapes, solver)
            .overlay_checked(OverlayRule::Subject, FillRule::NonZero, &mut control)
            .unwrap();
        assert_eq!(actual, expected);
        assert_eq!(control.attempts(), 1);
    }
}

/// Proves finite round exhaustion and Boolean work exhaustion return errors without shapes.
///
/// # Panics
/// Panics if a reduced cap is ignored or failed attempts replenish shared work.
#[test]
fn checked_caps_and_all_attempt_work_are_explicit() {
    let (rect, shapes) = maze();
    let mut capped = CheckedOverlayControl::new(4_000_000, &|| false).with_max_split_rounds(1);
    assert_eq!(
        overlay(rect, &shapes, Solver::AUTO)
            .overlay_checked(OverlayRule::Subject, FillRule::NonZero, &mut capped)
            .unwrap_err(),
        CheckedOverlayError::NonConvergent
    );
    assert_eq!(capped.attempts(), 3);
    assert_eq!(capped.split_rounds(), 3);
    let mut exhausted = CheckedOverlayControl::new(4_000, &|| false);
    assert_eq!(
        overlay(rect, &shapes, Solver::AUTO)
            .overlay_checked(OverlayRule::Subject, FillRule::NonZero, &mut exhausted)
            .unwrap_err(),
        CheckedOverlayError::WorkLimit
    );
    assert!(exhausted.operations() > 0);
    assert!(exhausted.operations() <= 4_000);
}

/// Proves cancellation becomes observable during dense candidate enumeration rather than only around overlay.
///
/// # Panics
/// Panics if cancellation is ignored until geometry or output publication finishes.
#[test]
fn checked_candidate_loops_poll_mid_operation() {
    let shapes: Shapes = (0..200)
        .map(|i| {
            let y = i as f64 / 1000.;
            vec![vec![[0., y], [10., y + 1.], [10., y + 5.], [0., y + 4.]]]
        })
        .collect();
    let rect = FloatRect::new(0., 10., 0., 10.).unwrap();
    let polls = Cell::new(0);
    let cancelled = || {
        polls.set(polls.get() + 1);
        polls.get() >= 8
    };
    let mut control = CheckedOverlayControl::new(1_000_000, &cancelled);
    assert_eq!(
        overlay(rect, &shapes, Solver::LIST)
            .overlay_checked(OverlayRule::Subject, FillRule::NonZero, &mut control)
            .unwrap_err(),
        CheckedOverlayError::Cancelled
    );
    assert_eq!(polls.get(), 8);
    assert!(control.operations() >= 1024);
    assert_eq!(control.attempts(), 1);
}
