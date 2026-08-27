use std::cell::Cell;

use toniator_domain::{PatternMechanismId, PatternOutputLayerId, RegionResizeAlgorithm};
use toniator_geometry::{
    Bounds, FamilySite, FamilySiteId, FamilySiteProvenance, FamilySiteSet, NominalCellBasis,
    Point2, RegionTreatment, RegionTreatmentLimits, RegionTreatmentRequest, SiteScope, Vector2,
    VoronoiRegionLimits, VoronoiRegionRequest, build_voronoi_regions_cancellable,
    treat_region_requests_cancellable,
};

/// Builds a finite random-provenance family whose order remains authoritative to the adapter.
fn sites(points: &[(f64, f64)]) -> FamilySiteSet {
    FamilySiteSet::new(
        "stage20o-family".into(),
        PatternMechanismId(9),
        points
            .iter()
            .enumerate()
            .map(|(ordinal, &(x, y))| FamilySite {
                id: FamilySiteId {
                    mechanism_id: PatternMechanismId(9),
                    ordinal,
                },
                position: Point2::new(x, y),
                nominal_cell_basis: NominalCellBasis::new(
                    Vector2::new(1.0, 0.0),
                    Vector2::new(0.0, 1.0),
                )
                .expect("unit basis"),
                scope: SiteScope::Canvas,
                provenance: FamilySiteProvenance::Random {
                    candidate_ordinal: ordinal,
                    accepted_ordinal: ordinal,
                    exclusion_neighbor_ordinal: None,
                },
            })
            .collect(),
    )
    .expect("valid family")
}

/// Builds a 5 by 5 guard lattice with square finite cells through the requested canvas.
fn guarded_lattice() -> FamilySiteSet {
    sites(
        &[-20.0, -10.0, 0.0, 10.0, 20.0]
            .into_iter()
            .flat_map(|x| {
                [-20.0, -10.0, 0.0, 10.0, 20.0]
                    .into_iter()
                    .map(move |y| (x, y))
            })
            .collect::<Vec<_>>(),
    )
}

/// Builds a bounded ordinary-region request whose canvas does not create topology.
fn request() -> VoronoiRegionRequest {
    VoronoiRegionRequest {
        output_layer_id: PatternOutputLayerId(44),
        canvas: Bounds::new(Point2::new(-1.0, -1.0), Point2::new(1.0, 1.0)).expect("finite canvas"),
    }
}

/// Builds a wider request that retains the complete 3 by 3 finite interior of the guard lattice.
fn wide_request() -> VoronoiRegionRequest {
    VoronoiRegionRequest {
        output_layer_id: PatternOutputLayerId(44),
        canvas: Bounds::new(Point2::new(-14.0, -14.0), Point2::new(14.0, 14.0))
            .expect("finite canvas"),
    }
}

/// Proves guard-inclusive finite cells reach the canonical region boundary without canvas closure.
#[test]
fn guard_family_produces_canonical_center_region() {
    let family = sites(&[
        (-10.0, -10.0),
        (0.0, -10.0),
        (10.0, -10.0),
        (-10.0, 0.0),
        (0.0, 0.0),
        (10.0, 0.0),
        (-10.0, 10.0),
        (0.0, 10.0),
        (10.0, 10.0),
    ]);
    let (regions, diagnostics) = build_voronoi_regions_cancellable(
        &family,
        request(),
        VoronoiRegionLimits::default(),
        || false,
    )
    .expect("guard family covers canvas");
    assert_eq!(regions.regions().len(), 1);
    assert_eq!(
        regions.regions()[0].id.source_id,
        toniator_geometry::CanonicalRegionSourceId::SiteOwners(vec![FamilySiteId {
            mechanism_id: PatternMechanismId(9),
            ordinal: 4
        }])
    );
    assert_eq!(diagnostics.site_groups, 9);
}

/// Proves an ordinary Voronoi cell uses one positive-region UniformOffset to double radius at fill two.
#[test]
fn voronoi_cell_uniform_offset_fill_two_targets_four_times_area() {
    let family = sites(&[
        (-10.0, -10.0),
        (0.0, -10.0),
        (10.0, -10.0),
        (-10.0, 0.0),
        (0.0, 0.0),
        (10.0, 0.0),
        (-10.0, 10.0),
        (0.0, 10.0),
        (10.0, 10.0),
    ]);
    let (regions, _) = build_voronoi_regions_cancellable(
        &family,
        request(),
        VoronoiRegionLimits::default(),
        || false,
    )
    .expect("ordinary Voronoi cell builds");
    let resized = treat_region_requests_cancellable(
        PatternOutputLayerId(44),
        &regions,
        &[RegionTreatmentRequest {
            base_region_id: regions.regions()[0].id.clone(),
            reference: None,
            treatment: Some(RegionTreatment {
                algorithm: RegionResizeAlgorithm::UniformOffset,
                fill: 2.0,
            }),
        }],
        RegionTreatmentLimits::default(),
        || false,
    )
    .expect("ordinary Voronoi uniform offset resolves");
    assert!((resized.regions.regions()[0].area - regions.regions()[0].area * 4.0).abs() < 1e-6);
}

/// Proves exact duplicate positions retain all sorted owners while avoiding one insertion.
#[test]
fn exact_duplicates_coown_one_region() {
    let family = sites(&[
        (-10.0, -10.0),
        (0.0, -10.0),
        (10.0, -10.0),
        (-10.0, 0.0),
        (-0.0, 0.0),
        (0.0, 0.0),
        (10.0, 0.0),
        (-10.0, 10.0),
        (0.0, 10.0),
        (10.0, 10.0),
    ]);
    let (regions, diagnostics) = build_voronoi_regions_cancellable(
        &family,
        request(),
        VoronoiRegionLimits::default(),
        || false,
    )
    .expect("duplicate center remains finite");
    assert_eq!(diagnostics.duplicate_groups, 1);
    assert_eq!(diagnostics.avoided_insertions, 1);
    assert_eq!(
        regions.regions()[0].id.source_id,
        toniator_geometry::CanonicalRegionSourceId::SiteOwners(vec![
            FamilySiteId {
                mechanism_id: PatternMechanismId(9),
                ordinal: 4
            },
            FamilySiteId {
                mechanism_id: PatternMechanismId(9),
                ordinal: 5
            }
        ])
    );
}

/// Proves cancellation returns the global cancellation diagnostic before topology publication.
#[test]
fn cancellation_is_atomic() {
    let cancelled = Cell::new(true);
    let error = build_voronoi_regions_cancellable(
        &sites(&[(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)]),
        request(),
        VoronoiRegionLimits::default(),
        || cancelled.get(),
    )
    .expect_err("cancelled build fails");
    assert_eq!(error.path(), "evaluation.cancelled");
}

/// Proves site-group limits reject before mutable triangulation insertion.
#[test]
fn site_group_limit_is_enforced() {
    let limits = VoronoiRegionLimits::new(2, 10, 10, 10, 100).expect("nonzero limits");
    let error = build_voronoi_regions_cancellable(
        &sites(&[(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)]),
        request(),
        limits,
        || false,
    )
    .expect_err("site groups exceed bound");
    assert_eq!(error.path(), "region.voronoi.limits.site_groups");
}

/// Proves each remaining producer work limit fails before it can return a partial region set.
#[test]
fn topology_region_boundary_and_inspection_limits_are_enforced() {
    let family = guarded_lattice();
    for (limits, path) in [
        (
            VoronoiRegionLimits::new(100, 1, 100, 100, 10_000).expect("limits"),
            "region.voronoi.limits.topology_edges",
        ),
        (
            VoronoiRegionLimits::new(100, 1_000, 1, 1_000, 10_000).expect("limits"),
            "region.voronoi.limits.regions",
        ),
        (
            VoronoiRegionLimits::new(100, 1_000, 1_000, 3, 10_000).expect("limits"),
            "region.voronoi.limits.boundary_points",
        ),
        (
            VoronoiRegionLimits::new(100, 1_000, 1_000, 1_000, 1).expect("limits"),
            "region.voronoi.limits.inspections",
        ),
    ] {
        let error = build_voronoi_regions_cancellable(&family, wide_request(), limits, || false)
            .expect_err("limit rejects atomically");
        assert_eq!(error.path(), path);
    }
}

/// Proves cancellation is polled after grouping during topology insertion and boundary traversal.
#[test]
fn cancellation_is_polled_across_build_phases() {
    for cancel_after in [1_usize, 12, 48, 96] {
        let calls = Cell::new(0_usize);
        let error = build_voronoi_regions_cancellable(
            &guarded_lattice(),
            request(),
            VoronoiRegionLimits::default(),
            || {
                let current = calls.get();
                calls.set(current + 1);
                current >= cancel_after
            },
        )
        .expect_err("phase cancellation rejects atomically");
        assert_eq!(error.path(), "evaluation.cancelled");
    }
}

/// Proves normalized exact duplicates co-own one cell while a distinct nearby coordinate does not.
#[test]
fn signed_zero_exact_duplicates_and_near_duplicates_remain_distinct() {
    let exact = sites(&[
        (-10.0, -10.0),
        (0.0, -10.0),
        (10.0, -10.0),
        (-10.0, 0.0),
        (-0.0, 0.0),
        (0.0, 0.0),
        (10.0, 0.0),
        (-10.0, 10.0),
        (0.0, 10.0),
        (10.0, 10.0),
    ]);
    let near = sites(&[
        (-10.0, -10.0),
        (0.0, -10.0),
        (10.0, -10.0),
        (-10.0, 0.0),
        (0.1, 0.0),
        (10.0, 0.0),
        (-10.0, 10.0),
        (0.0, 10.0),
        (10.0, 10.0),
    ]);
    let (_, exact_diagnostics) = build_voronoi_regions_cancellable(
        &exact,
        request(),
        VoronoiRegionLimits::default(),
        || false,
    )
    .expect("exact duplicates are grouped");
    let (_, near_diagnostics) =
        build_voronoi_regions_cancellable(&near, request(), VoronoiRegionLimits::default(), || {
            false
        })
        .expect("near duplicates remain distinct sites");
    assert_eq!(exact_diagnostics.duplicate_groups, 1);
    assert_eq!(near_diagnostics.duplicate_groups, 0);
    assert_eq!(near_diagnostics.site_groups, 9);
}

/// Proves the same ordered family replays canonical IDs and the exact same fingerprint.
#[test]
fn repeated_input_replays_canonical_ids_and_fingerprint() {
    let ordered = sites(&[
        (-10.0, -10.0),
        (0.0, -10.0),
        (10.0, -10.0),
        (-10.0, 0.0),
        (0.0, 0.0),
        (10.0, 0.0),
        (-10.0, 10.0),
        (0.0, 10.0),
        (10.0, 10.0),
    ]);
    let (first, _) = build_voronoi_regions_cancellable(
        &ordered,
        request(),
        VoronoiRegionLimits::default(),
        || false,
    )
    .expect("ordered family evaluates");
    let (second, _) = build_voronoi_regions_cancellable(
        &ordered,
        request(),
        VoronoiRegionLimits::default(),
        || false,
    )
    .expect("replayed family evaluates");
    assert_eq!(first.regions(), second.regions());
    assert_eq!(first.fingerprint(), second.fingerprint());
}

/// Proves zero through two groups and collinear topology fail as unbounded coverage, never as a panic.
#[test]
fn insufficient_and_collinear_sites_fail_coverage() {
    for points in [
        vec![],
        vec![(0.0, 0.0)],
        vec![(0.0, 0.0), (1.0, 0.0)],
        vec![(-1.0, 0.0), (0.0, 0.0), (1.0, 0.0)],
    ] {
        let error = build_voronoi_regions_cancellable(
            &sites(&points),
            request(),
            VoronoiRegionLimits::default(),
            || false,
        )
        .expect_err("degenerate family cannot cover regions");
        assert_eq!(error.path(), "region.voronoi.coverage.unbounded");
    }
}

/// Proves relevant unbounded triangular and cocircular hulls fail instead of canvas-closing a cell.
#[test]
fn relevant_unbounded_hulls_fail_coverage() {
    for points in [
        vec![(-10.0, -10.0), (10.0, -10.0), (0.0, 10.0)],
        vec![(-10.0, 0.0), (0.0, -10.0), (10.0, 0.0), (0.0, 10.0)],
    ] {
        let error = build_voronoi_regions_cancellable(
            &sites(&points),
            request(),
            VoronoiRegionLimits::default(),
            || false,
        )
        .expect_err("relevant unbounded hull rejects");
        assert_eq!(error.path(), "region.voronoi.coverage.unbounded");
    }
}

/// Proves off-canvas finite Voronoi cells are discarded by exact relevance rather than bounds alone.
#[test]
fn finite_off_canvas_cells_are_discarded() {
    let (regions, _) = build_voronoi_regions_cancellable(
        &guarded_lattice(),
        request(),
        VoronoiRegionLimits::default(),
        || false,
    )
    .expect("guard lattice evaluates");
    assert_eq!(regions.regions().len(), 1);
    assert_eq!(
        regions.regions()[0].id.source_id,
        toniator_geometry::CanonicalRegionSourceId::SiteOwners(vec![FamilySiteId {
            mechanism_id: PatternMechanismId(9),
            ordinal: 12,
        }])
    );
}

/// Proves the public bounds constructor rejects malformed and non-finite canvas and support
/// envelopes before a `VoronoiRegionRequest` can exist.
///
/// `Bounds` deliberately has no unchecked public constructor, so the ordinary-region builder
/// receives only its validated request boundary and does not need an unsafe test-only escape
/// hatch for impossible request state.
#[test]
fn request_canvas_and_support_bounds_reject_nonfinite_or_malformed_inputs() {
    for (min, max) in [
        (Point2::new(f64::NAN, -1.0), Point2::new(1.0, 1.0)),
        (Point2::new(-1.0, f64::NEG_INFINITY), Point2::new(1.0, 1.0)),
        (Point2::new(-1.0, -1.0), Point2::new(f64::INFINITY, 1.0)),
        (Point2::new(1.0, -1.0), Point2::new(-1.0, 1.0)),
        (Point2::new(-1.0, 1.0), Point2::new(1.0, -1.0)),
    ] {
        assert!(
            Bounds::new(min, max).is_none(),
            "invalid bounds cannot become a Region request canvas"
        );
    }
    let canvas = request().canvas;
    for support in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.25] {
        assert!(
            canvas.expanded(support).is_none(),
            "invalid support cannot produce a public region-relevance envelope"
        );
    }
}

/// Proves the sole `FamilySiteSet` authority rejects a foreign mechanism before duplicate
/// coordinates could acquire a mixed-mechanism Region co-owner identity.
///
/// Co-owner ordering is consequently defined only for site IDs emitted by one product mechanism;
/// the same-mechanism full-ID ordering remains covered by `exact_duplicates_coown_one_region`.
#[test]
fn mixed_mechanism_duplicate_owners_are_rejected_upstream() {
    let basis =
        NominalCellBasis::new(Vector2::new(1.0, 0.0), Vector2::new(0.0, 1.0)).expect("unit basis");
    let product = PatternMechanismId(9);
    let error = FamilySiteSet::new(
        "stage20o-mixed-owner".into(),
        product,
        vec![
            FamilySite {
                id: FamilySiteId {
                    mechanism_id: product,
                    ordinal: 0,
                },
                position: Point2::new(0.0, 0.0),
                nominal_cell_basis: basis,
                scope: SiteScope::Canvas,
                provenance: FamilySiteProvenance::Random {
                    candidate_ordinal: 0,
                    accepted_ordinal: 0,
                    exclusion_neighbor_ordinal: None,
                },
            },
            FamilySite {
                id: FamilySiteId {
                    mechanism_id: PatternMechanismId(10),
                    ordinal: 1,
                },
                position: Point2::new(-0.0, 0.0),
                nominal_cell_basis: basis,
                scope: SiteScope::Canvas,
                provenance: FamilySiteProvenance::Random {
                    candidate_ordinal: 1,
                    accepted_ordinal: 1,
                    exclusion_neighbor_ordinal: None,
                },
            },
        ],
    )
    .expect_err("a family cannot mix output-mechanism identities");
    assert_eq!(error.path(), "family_sites.id.mechanism_id_mismatch");
}
