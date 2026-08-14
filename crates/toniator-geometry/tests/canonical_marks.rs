use toniator_domain::PatternMechanismId;
use toniator_geometry::{
    CanonicalFillRule, CanonicalPathMark, CurvePath, FamilySiteId, FamilySiteProvenance,
    PathClosure, Point2, SiteScope,
};

/// Returns truthful fixed provenance for one geometry-only canonical mark fixture.
fn provenance() -> FamilySiteProvenance {
    FamilySiteProvenance::Random {
        candidate_ordinal: 4,
        accepted_ordinal: 2,
        exclusion_neighbor_ordinal: Some(1),
    }
}

/// Accepts exact closed construction geometry with truthful bounds/fill metadata and rejects an
/// explicitly open path without repairing its topology or losing the supplied site authority.
#[test]
fn canonical_path_marks_require_closed_geometry_and_retain_truthful_metadata() {
    let site_id = FamilySiteId {
        mechanism_id: PatternMechanismId(9),
        ordinal: 2,
    };
    let open = CurvePath::line(Point2::new(-2.0, 1.0), Point2::new(4.0, 3.0)).unwrap();
    let error = CanonicalPathMark::new(
        site_id,
        open,
        SiteScope::Guard,
        provenance(),
        CanonicalFillRule::EvenOdd,
    )
    .unwrap_err();
    assert_eq!(error.path(), "canonical.path_mark.closure");

    let path = CurvePath::polyline(
        vec![
            Point2::new(-2.0, -1.0),
            Point2::new(4.0, -1.0),
            Point2::new(1.0, 5.0),
        ],
        PathClosure::Closed,
    )
    .unwrap();
    let mark = CanonicalPathMark::new(
        site_id,
        path.clone(),
        SiteScope::Guard,
        provenance(),
        CanonicalFillRule::EvenOdd,
    )
    .unwrap();
    assert_eq!(mark.source_site_id, site_id);
    assert_eq!(mark.path, path);
    assert_eq!(mark.bounds.min, Point2::new(-2.0, -1.0));
    assert_eq!(mark.bounds.max, Point2::new(4.0, 5.0));
    assert_eq!(mark.scope, SiteScope::Guard);
    assert_eq!(mark.provenance, provenance());
    assert_eq!(mark.fill_rule, CanonicalFillRule::EvenOdd);
}
