use toniator_domain::PathStrokeStyle;
use toniator_geometry::{
    CanonicalStroke, CurvePath, GuideInstanceId, PathClosure, PathLocation, Point2,
    StrokeProfileSample, VariableWidthOutlineLimits, VariableWidthPathSample,
    build_variable_width_outline_cancellable,
};

/// Builds one bounded reusable outline from exact finite width samples.
fn outline(
    path: &CurvePath,
    widths: &[(usize, f64, f64)],
) -> toniator_geometry::CanonicalFilledOutline {
    build_variable_width_outline_cancellable(
        path,
        &widths
            .iter()
            .map(|(segment, parameter, width)| VariableWidthPathSample {
                location: PathLocation::new(*segment, *parameter).expect("location"),
                width: *width,
            })
            .collect::<Vec<_>>(),
        PathStrokeStyle::default(),
        1.0 / 8.0,
        VariableWidthOutlineLimits::new(128).expect("limit"),
        &|| false,
    )
    .expect("outline")
}

/// Proves canonical strokes retain off-canvas centerlines and independent nonzero outline bounds.
#[test]
fn canonical_stroke_retains_ordered_profile_without_canvas_clipping() {
    let path = CurvePath::line(Point2::new(-5.0, 4.0), Point2::new(15.0, 4.0))
        .expect("finite open line builds");
    let profile = vec![
        StrokeProfileSample {
            location: PathLocation::new(0, 0.0).expect("location"),
            center: Point2::new(-5.0, 4.0),
            normalized_thickness: 0.2,
            width: 2.0,
        },
        StrokeProfileSample {
            location: PathLocation::new(0, 1.0).expect("location"),
            center: Point2::new(15.0, 4.0),
            normalized_thickness: 0.4,
            width: 4.0,
        },
    ];
    let stroke = CanonicalStroke::new(
        GuideInstanceId {
            dimension_id: 3,
            index: -1,
        },
        None,
        path.clone(),
        10.0,
        PathStrokeStyle::default(),
        profile.clone(),
        outline(&path, &[(0, 0.0, 2.0), (0, 1.0, 4.0)]),
    )
    .expect("canonical stroke validates");
    assert_eq!(stroke.path, path);
    assert_eq!(stroke.profile, profile);
    assert!(stroke.outline.bounds.expect("positive outline").min.x < -5.0);
}

/// Proves the reusable builder handles zero widths, taper runs, closed winding, and global bounds.
#[test]
fn reusable_outline_preserves_zero_runs_closed_winding_and_limits() {
    let path = CurvePath::line(Point2::new(0.0, 0.0), Point2::new(8.0, 0.0)).expect("line");
    assert!(
        outline(&path, &[(0, 0.0, 0.0), (0, 1.0, 0.0)])
            .contours
            .is_empty()
    );
    let taper = outline(&path, &[(0, 0.0, 0.0), (0, 0.5, 3.0), (0, 1.0, 0.0)]);
    assert!(!taper.contours.is_empty());
    assert!(taper.contours.iter().all(contour_is_closed));
    let disc = outline(&path, &[(0, 0.5, 3.0)]);
    assert_eq!(disc.contours[0].segments.len(), 4);
    assert!(contour_is_closed(&disc.contours[0]));
    let ring = CurvePath::polyline(
        vec![
            Point2::new(0.0, 0.0),
            Point2::new(4.0, 0.0),
            Point2::new(4.0, 4.0),
        ],
        PathClosure::Closed,
    )
    .expect("ring");
    let closed = outline(&ring, &[(0, 0.0, 2.0), (1, 0.0, 2.0), (2, 0.0, 2.0)]);
    assert_eq!(closed.contours.len(), 2);
    assert!(
        contour_signed_area(&closed.contours[0]) * contour_signed_area(&closed.contours[1]) < 0.0
    );
    let split_closed = outline(&ring, &[(0, 0.0, 2.0), (1, 0.0, 0.0), (2, 0.0, 2.0)]);
    assert_eq!(split_closed.contours.len(), 1);
    assert!(contour_is_closed(&split_closed.contours[0]));
    let two_runs = outline(
        &ring,
        &[
            (0, 0.0, 2.0),
            (0, 1.0, 0.0),
            (1, 0.0, 0.0),
            (1, 1.0, 2.0),
            (2, 0.0, 0.0),
        ],
    );
    assert_eq!(two_runs.contours.len(), 2);
    assert!(two_runs.contours.iter().all(contour_is_closed));
    let error = build_variable_width_outline_cancellable(
        &path,
        &[VariableWidthPathSample {
            location: PathLocation::new(0, 0.0).expect("location"),
            width: 2.0,
        }],
        PathStrokeStyle::default(),
        1.0 / 8.0,
        VariableWidthOutlineLimits::new(1).expect("limit"),
        &|| false,
    )
    .expect_err("disc needs four cubic quarter arcs");
    assert_eq!(error.path(), "curve.outline.segment_limit");
}

/// Confirms one derived contour connects every cubic through its exact final closure.
fn contour_is_closed(contour: &toniator_geometry::CanonicalOutlineContour) -> bool {
    !contour.segments.is_empty()
        && contour
            .segments
            .windows(2)
            .all(|pair| pair[0].end() == pair[1].start())
        && contour.segments.last().expect("nonempty").end() == contour.segments[0].start()
}

/// Returns an orientation witness from the contour's retained segment starts.
fn contour_signed_area(contour: &toniator_geometry::CanonicalOutlineContour) -> f64 {
    let points = contour
        .segments
        .iter()
        .map(|segment| segment.start())
        .collect::<Vec<_>>();
    points
        .iter()
        .copied()
        .chain(points.first().copied())
        .collect::<Vec<_>>()
        .windows(2)
        .map(|pair| pair[0].x * pair[1].y - pair[1].x * pair[0].y)
        .sum::<f64>()
        * 0.5
}

/// Proves unordered or nonfinite samples reject before a reusable outline becomes visible.
#[test]
fn reusable_outline_rejects_unordered_or_nonfinite_samples() {
    let path = CurvePath::line(Point2::new(0.0, 0.0), Point2::new(4.0, 0.0)).expect("line");
    let error = build_variable_width_outline_cancellable(
        &path,
        &[
            VariableWidthPathSample {
                location: PathLocation::new(0, 1.0).expect("location"),
                width: 1.0,
            },
            VariableWidthPathSample {
                location: PathLocation::new(0, 0.0).expect("location"),
                width: f64::NAN,
            },
        ],
        PathStrokeStyle::default(),
        1.0 / 8.0,
        VariableWidthOutlineLimits::new(16).expect("limit"),
        &|| false,
    )
    .expect_err("invalid samples reject");
    assert_eq!(error.path(), "curve.outline.width");
}

/// Proves simplification uses authored path location rather than the count of adaptive samples.
/// Simplifies by exact authored location and emits a round join only when its boundary widths match.
#[test]
fn reusable_outline_simplifies_by_location_and_rounds_the_outer_corner() {
    let path = CurvePath::line(Point2::new(0.0, 0.0), Point2::new(100.0, 0.0)).expect("line");
    let nonuniform = outline(&path, &[(0, 0.0, 2.0), (0, 0.01, 2.02), (0, 1.0, 4.0)]);
    assert_eq!(nonuniform.contours[0].segments.len(), 6);
    let corner = CurvePath::polyline(
        vec![
            Point2::new(0.0, 0.0),
            Point2::new(4.0, 0.0),
            Point2::new(4.0, 4.0),
        ],
        PathClosure::Open,
    )
    .expect("corner");
    let outlined = outline(
        &corner,
        &[(0, 0.0, 2.0), (0, 1.0, 2.0), (1, 0.0, 2.0), (1, 1.0, 2.0)],
    );
    let join = outlined.contours[0]
        .segments
        .iter()
        .find(|segment| {
            segment.start() == Point2::new(4.0, 1.0) && segment.end() == Point2::new(3.0, 0.0)
        })
        .expect("outer round join retains its exact rail endpoints");
    let toniator_geometry::CurveSegment::CubicBezier(join) = join else {
        panic!("round join stays cubic");
    };
    assert!(join.control_1().x < 4.0 && join.control_1().y >= 1.0);
    assert!(join.control_2().x <= 3.0 && join.control_2().y > 0.0);
    assert!(contour_is_closed(&outlined.contours[0]));
    let unequal = outline(
        &corner,
        &[(0, 0.0, 2.0), (0, 1.0, 2.0), (1, 0.0, 3.0), (1, 1.0, 3.0)],
    );
    let fallback = unequal.contours[0]
        .segments
        .iter()
        .find(|segment| {
            segment.start() == Point2::new(4.0, 1.0) && segment.end() == Point2::new(2.5, 0.0)
        })
        .expect("unequal-radius boundary retains a deterministic rail");
    let toniator_geometry::CurveSegment::CubicBezier(fallback) = fallback else {
        panic!("rail remains cubic storage");
    };
    assert!(fallback.control_1().y < 1.0 && fallback.control_2().y < 1.0);
}
