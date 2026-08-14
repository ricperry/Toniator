use toniator_domain::PatternMechanismId;
use toniator_geometry::{
    AffineTransform2D, Bounds, FamilySite, FamilySiteId, FamilySiteProvenance, FamilySiteSet,
    GuideInstanceId, NominalCellBasis, Point2, SiteScope, Vector2,
};

#[test]
/// Proves finite affine transforms retain their reversible document-space contract.
fn rotation_about_center_then_document_translation_round_trips() {
    let transform = AffineTransform2D::rotate_about_then_translate(
        Point2::new(450.0, 300.0),
        17.0,
        Vector2::new(3.25, -4.5),
    )
    .expect("finite transform");
    let point = Point2::new(100.0, 200.0);
    let document = transform.apply_point(point);
    let restored = transform.inverse_point(document);

    assert!((restored.x - point.x).abs() < 1e-10);
    assert!((restored.y - point.y).abs() < 1e-10);
    assert_eq!(
        transform.apply_point(Point2::new(450.0, 300.0)),
        Point2::new(453.25, 295.5)
    );
}

#[test]
/// Proves inverse bounds retain every finite padded corner after transformation.
fn inverse_transform_maps_every_padded_corner_to_finite_local_bounds() {
    let transform = AffineTransform2D::rotate_about_then_translate(
        Point2::new(450.0, 300.0),
        89.5,
        Vector2::new(-20.0, 30.0),
    )
    .expect("finite transform");
    let bounds =
        Bounds::new(Point2::new(-25.5, -25.5), Point2::new(925.5, 625.5)).expect("finite bounds");
    let local = transform
        .inverse_bounds(bounds)
        .expect("finite local bounds");

    assert!(local.min.is_finite());
    assert!(local.max.is_finite());
    for corner in bounds.corners() {
        assert!(local.contains(transform.inverse_point(corner)));
    }
}

/// Proves the public family-site contract retains evaluator order and rejects
/// invalid identities, coordinates, and truthful provenance at stable paths.
#[test]
fn family_site_set_contract_rejects_invalid_ids_order_positions_and_provenance() {
    let product = PatternMechanismId(44);
    let intersection = |ordinal| FamilySite {
        id: FamilySiteId {
            mechanism_id: product,
            ordinal,
        },
        position: Point2::new(ordinal as f64, 2.0),
        nominal_cell_basis: NominalCellBasis::new(Vector2::new(1.0, 0.0), Vector2::new(0.0, 1.0))
            .unwrap(),
        scope: SiteScope::Canvas,
        provenance: FamilySiteProvenance::GuideIntersection {
            contributors: vec![
                GuideInstanceId {
                    dimension_id: 1,
                    index: 7,
                },
                GuideInstanceId {
                    dimension_id: 2,
                    index: -3,
                },
            ],
        },
    };
    let valid = FamilySiteSet::new(
        "family-v1".into(),
        product,
        vec![
            intersection(0),
            FamilySite {
                id: FamilySiteId {
                    mechanism_id: product,
                    ordinal: 1,
                },
                position: Point2::new(3.0, 4.0),
                nominal_cell_basis: NominalCellBasis::new(
                    Vector2::new(1.0, 0.0),
                    Vector2::new(0.0, 1.0),
                )
                .unwrap(),
                scope: SiteScope::Guard,
                provenance: FamilySiteProvenance::AlongGuide {
                    guide_id: GuideInstanceId {
                        dimension_id: 9,
                        index: -4,
                    },
                    guide_order: 2,
                    sequence: 8,
                    absolute_arc_position_bits: 18.5_f64.to_bits(),
                    local_arc_position_bits: 2.5_f64.to_bits(),
                },
            },
            FamilySite {
                id: FamilySiteId {
                    mechanism_id: product,
                    ordinal: 2,
                },
                position: Point2::new(5.0, 6.0),
                nominal_cell_basis: NominalCellBasis::new(
                    Vector2::new(1.0, 0.0),
                    Vector2::new(0.0, 1.0),
                )
                .unwrap(),
                scope: SiteScope::Canvas,
                provenance: FamilySiteProvenance::Random {
                    candidate_ordinal: 5,
                    accepted_ordinal: 2,
                    exclusion_neighbor_ordinal: Some(1),
                },
            },
        ],
    )
    .expect("valid ordered family sites");
    assert_eq!(valid.family_fingerprint(), "family-v1");
    assert_eq!(valid.product_mechanism_id(), product);
    assert_eq!(
        valid.iter().map(|site| site.id.ordinal).collect::<Vec<_>>(),
        [0, 1, 2]
    );
    assert_eq!(valid.len(), 3);
    assert!(!valid.is_empty());

    let error = |sites| {
        FamilySiteSet::new("family".into(), product, sites)
            .expect_err("invalid family sites")
            .path()
    };
    assert_eq!(
        FamilySiteSet::new("".into(), product, vec![])
            .expect_err("empty fingerprint")
            .path(),
        "family_sites.family_fingerprint"
    );
    assert_eq!(
        FamilySiteSet::new("family".into(), PatternMechanismId(0), vec![])
            .expect_err("zero product")
            .path(),
        "family_sites.product_mechanism_id"
    );
    assert_eq!(
        error(vec![intersection(0), intersection(0)]),
        "family_sites.id.duplicate"
    );
    let mut mismatch = intersection(0);
    mismatch.id.mechanism_id = PatternMechanismId(45);
    assert_eq!(
        error(vec![mismatch]),
        "family_sites.id.mechanism_id_mismatch"
    );
    assert_eq!(error(vec![intersection(1)]), "family_sites.id.ordinal");
    let mut nonfinite = intersection(0);
    nonfinite.position.x = f64::NAN;
    assert_eq!(error(vec![nonfinite]), "family_sites.position");
    let mut duplicate_contributors = intersection(0);
    duplicate_contributors.provenance = FamilySiteProvenance::GuideIntersection {
        contributors: vec![
            GuideInstanceId {
                dimension_id: 1,
                index: 0,
            },
            GuideInstanceId {
                dimension_id: 1,
                index: 0,
            },
        ],
    };
    assert_eq!(
        error(vec![duplicate_contributors]),
        "family_sites.provenance.guide_intersection.contributors"
    );
    let along = |dimension_id, arc_bits| FamilySite {
        id: FamilySiteId {
            mechanism_id: product,
            ordinal: 0,
        },
        position: Point2::new(0.0, 0.0),
        nominal_cell_basis: NominalCellBasis::new(Vector2::new(1.0, 0.0), Vector2::new(0.0, 1.0))
            .unwrap(),
        scope: SiteScope::Canvas,
        provenance: FamilySiteProvenance::AlongGuide {
            guide_id: GuideInstanceId {
                dimension_id,
                index: 0,
            },
            guide_order: 0,
            sequence: 0,
            absolute_arc_position_bits: arc_bits,
            local_arc_position_bits: 0.0_f64.to_bits(),
        },
    };
    assert_eq!(
        error(vec![along(0, 0.0_f64.to_bits())]),
        "family_sites.provenance.along_guide.guide_id"
    );
    assert_eq!(
        error(vec![along(1, f64::NAN.to_bits())]),
        "family_sites.provenance.along_guide.arc_position"
    );
    let random = FamilySite {
        id: FamilySiteId {
            mechanism_id: product,
            ordinal: 0,
        },
        position: Point2::new(0.0, 0.0),
        nominal_cell_basis: NominalCellBasis::new(Vector2::new(1.0, 0.0), Vector2::new(0.0, 1.0))
            .unwrap(),
        scope: SiteScope::Canvas,
        provenance: FamilySiteProvenance::Random {
            candidate_ordinal: 1,
            accepted_ordinal: 2,
            exclusion_neighbor_ordinal: Some(2),
        },
    };
    assert_eq!(
        error(vec![random]),
        "family_sites.provenance.random.ordinals"
    );
}
