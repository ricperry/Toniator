//! Editable full-cell silhouettes stay compact through the common native SVG clipping route.
use toniator_domain::{
    CanvasSpec, ChannelId, ColorValue, PatternMechanismId, PatternOutputLayerId,
};
use toniator_geometry::*;
use toniator_render::*;

/// Builds real compact canonical segment/sector/circle footprints at a requested cell placement.
/// # Panics
/// Panics if finite current canonical construction or scene validation fails.
fn scene(kind: usize, x: f64, y: f64, angle: f64) -> RenderScene {
    let id = FamilySiteId {
        mechanism_id: PatternMechanismId(1),
        ordinal: 0,
    };
    let provenance = FamilySiteProvenance::Random {
        candidate_ordinal: 0,
        accepted_ordinal: 0,
        exclusion_neighbor_ordinal: None,
    };
    let center = Point2::new(x, y);
    let mark = if kind == 2 {
        CanonicalMark::Circle {
            source_site_id: id,
            center,
            radius: 16.0,
            scope: SiteScope::Guard,
            provenance,
            fill_rule: CanonicalFillRule::EvenOdd,
        }
    } else {
        let p = |x: f64, y: f64| {
            let a = angle.to_radians();
            let (x, y) = ((x - 0.5) * 32.0, (y - 0.5) * 32.0);
            Point2::new(
                center.x + x * a.cos() - y * a.sin(),
                center.y + x * a.sin() + y * a.cos(),
            )
        };
        let k = 4.0 * (2.0_f64.sqrt() - 1.0) / 3.0;
        let mut segments = vec![CurveSegment::CubicBezier(
            CubicBezierSegment::new(p(0., 0.), p(k, 0.), p(1., 1. - k), p(1., 1.)).unwrap(),
        )];
        if kind == 1 {
            segments.push(CurveSegment::Line(
                LineSegment::new(p(1., 1.), p(0., 1.)).unwrap(),
            ));
            segments.push(CurveSegment::Line(
                LineSegment::new(p(0., 1.), p(0., 0.)).unwrap(),
            ));
        } else {
            segments.push(CurveSegment::Line(
                LineSegment::new(p(1., 1.), p(0., 0.)).unwrap(),
            ));
        }
        CanonicalMark::ClosedPath(
            CanonicalPathMark::new(
                id,
                CurvePath::new(segments, PathClosure::Closed).unwrap(),
                SiteScope::Guard,
                provenance,
                CanonicalFillRule::EvenOdd,
            )
            .unwrap(),
        )
    };
    let layer = RenderLayer::new_outputs(
        ChannelId(1),
        true,
        ColorValue {
            red: 0.0,
            green: 0.0,
            blue: 0.0,
            alpha: 1.0,
        },
        1.0,
        vec![RenderOutputLayer::new(
            PatternOutputLayerId(1),
            GeometryOutput::CanonicalMarks(vec![mark]),
            None,
        )],
    )
    .unwrap();
    RenderScene::new(
        CanvasSpec {
            width: 64.0,
            height: 64.0,
        },
        "cell-grid".into(),
        "cell-shape".into(),
        vec![layer],
    )
    .unwrap()
}

/// Enforces native curve budgets for interior cells and all canvas-edge/corner intersections.
/// Cubic/chord segments need one uncut cubic, sectors one, circles four arcs. Clipping may add
/// at most four canvas boundary spans plus curve subdivisions; sixteen drawn edges bound this set.
/// # Panics
/// Panics if SVG joining polygonizes a safe footprint or clipping exceeds its bounded overhead.
#[test]
fn adaptive_svg_contained_and_clipped_curves_remain_editable() {
    for kind in 0..3 {
        for angle in [0.0, 17.0, 45.0, 89.0, 90.0, 180.0, 270.0] {
            for (x, y) in [
                (32., 32.),
                (0., 32.),
                (64., 32.),
                (32., 0.),
                (32., 64.),
                (0., 0.),
                (64., 0.),
                (0., 64.),
                (64., 64.),
            ] {
                let svg = write_svg(&scene(kind, x, y, angle)).unwrap();
                let Some(data) = svg.split(" d=\"").nth(1) else {
                    assert!(
                        x != 32.0 || y != 32.0,
                        "contained shape must remain visible"
                    );
                    continue;
                };
                let data = data.split('"').next().unwrap();
                let edges = data
                    .chars()
                    .filter(|c| matches!(c, 'L' | 'C' | 'A' | 'a'))
                    .count();
                assert!(
                    edges <= 16,
                    "kind={kind} angle={angle} center=({x},{y}) edges={edges}"
                );
                if x == 32.0 && y == 32.0 {
                    if kind == 2 {
                        assert_eq!(data.matches('a').count(), 4);
                    } else {
                        assert_eq!(data.matches('C').count(), 1);
                        assert_eq!(data.matches('L').count(), if kind == 0 { 1 } else { 2 });
                    }
                }
            }
        }
    }
}
