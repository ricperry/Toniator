use std::sync::Mutex;

use toniator_domain::{
    CanvasSpec, PathStrokeStyle, PatternMechanismId, SourceMapping, SourceMappingComponent,
};
use toniator_patterns::{
    CubicBezierSegment, CurvePath, CurveSegment, FamilySite, FamilySiteId, FamilySiteProvenance,
    FamilySiteSet, GuideInstanceId, NominalCellBasis, PathClosure, Point2, SiteScope,
    StrokeResponse, Vector2, chain_curve_motif_rows_cancellable,
    realize_curve_motif_canonical_strokes_cancellable,
};
use toniator_sampling::{SourceFormatHint, decode_source};

/// Builds deterministic Along Guides sites for two rows and three adjacent cadence positions.
fn rows() -> FamilySiteSet {
    let mechanism = PatternMechanismId(7);
    let basis = NominalCellBasis::new(Vector2::new(10.0, 0.0), Vector2::new(0.0, 10.0))
        .expect("finite positive basis");
    let mut sites = Vec::new();
    for guide_index in [-1_i64, 0_i64] {
        for sequence in 0_i64..=2 {
            sites.push(FamilySite {
                id: FamilySiteId {
                    mechanism_id: mechanism,
                    ordinal: sites.len(),
                },
                position: Point2::new(sequence as f64 * 10.0, (guide_index + 1) as f64 * 20.0),
                nominal_cell_basis: basis,
                scope: SiteScope::Canvas,
                provenance: FamilySiteProvenance::AlongGuide {
                    guide_id: GuideInstanceId {
                        dimension_id: 1,
                        index: guide_index,
                        component_ordinal: 0,
                    },
                    guide_order: 0,
                    sequence,
                    absolute_arc_position_bits: (sequence as f64 * 10.0).to_bits(),
                    local_arc_position_bits: (sequence as f64 * 10.0).to_bits(),
                },
            });
        }
    }
    FamilySiteSet::new("curve-motif-test".into(), mechanism, sites)
        .expect("valid Along Guides sites")
}

/// Rebuilds the first provenance row with caller-owned sequence values for strict-cadence witnesses.
fn rows_with_first_row_sequences(sequences: &[i64]) -> FamilySiteSet {
    let source = rows();
    let mut sites = source.sites().to_vec();
    for (site, sequence) in sites
        .iter_mut()
        .filter(|site| {
            matches!(
                site.provenance,
                FamilySiteProvenance::AlongGuide { guide_id, .. } if guide_id.index == -1
            )
        })
        .zip(sequences.iter().copied())
    {
        if let FamilySiteProvenance::AlongGuide {
            sequence: current, ..
        } = &mut site.provenance
        {
            *current = sequence;
        }
    }
    FamilySiteSet::new(
        "curve-motif-sequence-test".into(),
        PatternMechanismId(7),
        sites,
    )
    .expect("sequence witness sites remain structurally valid")
}

/// Builds the ordinary two-row fixture at a larger Along Guides cadence for source-edge sampling.
fn wide_rows() -> FamilySiteSet {
    let mechanism = PatternMechanismId(9);
    let basis = NominalCellBasis::new(Vector2::new(100.0, 0.0), Vector2::new(0.0, 20.0))
        .expect("finite wide basis");
    let mut sites = Vec::new();
    for guide_index in [-1_i64, 0_i64] {
        for sequence in 0_i64..=2 {
            sites.push(FamilySite {
                id: FamilySiteId {
                    mechanism_id: mechanism,
                    ordinal: sites.len(),
                },
                position: Point2::new(sequence as f64 * 100.0, (guide_index + 1) as f64 * 20.0),
                nominal_cell_basis: basis,
                scope: SiteScope::Canvas,
                provenance: FamilySiteProvenance::AlongGuide {
                    guide_id: GuideInstanceId {
                        dimension_id: 1,
                        index: guide_index,
                        component_ordinal: 0,
                    },
                    guide_order: 0,
                    sequence,
                    absolute_arc_position_bits: (sequence as f64 * 100.0).to_bits(),
                    local_arc_position_bits: (sequence as f64 * 100.0).to_bits(),
                },
            });
        }
    }
    FamilySiteSet::new("curve-motif-wide-edge".into(), mechanism, sites)
        .expect("wide Along Guides sites validate")
}

/// Proves rows chain adjacent cadence sites exactly and use Euclidean parity for negative rows.
#[test]
fn curve_motif_rows_chain_and_compose_mirror_phase_deterministically() {
    let motif = CurvePath::polyline(
        vec![
            Point2::new(0.0, 0.0),
            Point2::new(0.5, 0.25),
            Point2::new(1.0, 0.0),
        ],
        PathClosure::Open,
    )
    .expect("asymmetric open motif");
    let baseline = chain_curve_motif_rows_cancellable(&rows(), &motif, false, None, &|| false)
        .expect("baseline rows chain");
    let mirrored = chain_curve_motif_rows_cancellable(&rows(), &motif, true, None, &|| false)
        .expect("mirrored rows chain");
    let phased = chain_curve_motif_rows_cancellable(&rows(), &motif, false, Some(0.25), &|| false)
        .expect("phased rows chain");
    let first = chain_curve_motif_rows_cancellable(&rows(), &motif, true, Some(0.25), &|| false)
        .expect("rows chain");
    let second = chain_curve_motif_rows_cancellable(&rows(), &motif, true, Some(0.25), &|| false)
        .expect("rows repeat");
    assert_eq!(first, second);
    assert_eq!(first.len(), 2);
    assert_eq!(first[0].segments().len(), 4);
    assert_eq!(first[0].segments()[1].end(), first[0].segments()[2].start());
    assert_eq!(baseline[0].start().x, 0.0);
    assert_eq!(mirrored[0].segments()[0].end().y, -2.5);
    assert_eq!(phased[0].start().x, 2.5);
    assert_eq!(first[0].start().x, 2.5);
    assert_eq!(first[0].segments()[0].end().y, -2.5);
}

/// Proves cancellation aborts before publishing any independently scheduled row output.
#[test]
fn curve_motif_rows_honor_cancellation() {
    let motif = CurvePath::line(Point2::new(0.0, 0.0), Point2::new(1.0, 0.0)).expect("open motif");
    let error = chain_curve_motif_rows_cancellable(&rows(), &motif, false, None, &|| true)
        .expect_err("cancelled rows reject");
    assert_eq!(error.path(), "evaluation.cancelled");
}

/// Rejects gapped, duplicate, and overflow-prone Along Guides sequence pairs before row work starts.
#[test]
fn curve_motif_rows_reject_nonconsecutive_or_duplicate_sequences_without_saturation() {
    let motif = CurvePath::line(Point2::new(0.0, 0.0), Point2::new(1.0, 0.0)).expect("open motif");
    for sequences in [
        [0_i64, 2, 3],
        [0_i64, 0, 1],
        [i64::MAX - 1, i64::MAX, i64::MIN],
    ] {
        let error = chain_curve_motif_rows_cancellable(
            &rows_with_first_row_sequences(&sequences),
            &motif,
            false,
            None,
            &|| false,
        )
        .expect_err("invalid sequence pair rejects");
        assert_eq!(error.path(), "curve_motif.sites");
    }
    let accepted = chain_curve_motif_rows_cancellable(
        &rows_with_first_row_sequences(&[i64::MAX - 2, i64::MAX - 1, i64::MAX]),
        &motif,
        false,
        None,
        &|| false,
    )
    .expect("highest representable consecutive sequence remains valid");
    assert_eq!(accepted.len(), 2);
}

/// Maps a non-axis authored motif through a negative odd row while retaining exact C0 destinations.
#[test]
fn curve_motif_non_axis_mapping_keeps_endpoints_exact_and_uses_negative_row_parity() {
    let motif = CurvePath::polyline(
        vec![
            Point2::new(0.0, 0.0),
            Point2::new(0.35, 0.4),
            Point2::new(1.0, 0.0),
        ],
        PathClosure::Open,
    )
    .expect("non-axis motif validates");
    let basis = NominalCellBasis::new(Vector2::new(4.0, 3.0), Vector2::new(-3.0, 4.0))
        .expect("non-axis basis validates");
    let positions = [
        Point2::new(2.0, -1.0),
        Point2::new(6.0, 2.0),
        Point2::new(10.0, 5.0),
    ];
    let sites = FamilySiteSet::new(
        "curve-motif-non-axis".into(),
        PatternMechanismId(8),
        positions
            .into_iter()
            .enumerate()
            .map(|(ordinal, position)| FamilySite {
                id: FamilySiteId {
                    mechanism_id: PatternMechanismId(8),
                    ordinal,
                },
                position,
                nominal_cell_basis: basis,
                scope: SiteScope::Canvas,
                provenance: FamilySiteProvenance::AlongGuide {
                    guide_id: GuideInstanceId {
                        dimension_id: 2,
                        index: -1,
                        component_ordinal: 0,
                    },
                    guide_order: 0,
                    sequence: ordinal as i64,
                    absolute_arc_position_bits: (ordinal as f64 * 5.0).to_bits(),
                    local_arc_position_bits: (ordinal as f64 * 5.0).to_bits(),
                },
            })
            .collect(),
    )
    .expect("non-axis Along Guides sites validate");
    let rows = chain_curve_motif_rows_cancellable(&sites, &motif, true, Some(0.25), &|| false)
        .expect("non-axis row chains");
    let row = &rows[0];
    assert_eq!(row.start(), Point2::new(3.0, -0.25));
    assert_eq!(row.segments()[1].end(), Point2::new(7.0, 2.75));
    assert_eq!(row.segments()[1].end(), row.segments()[2].start());
    assert_eq!(row.end(), Point2::new(11.0, 5.75));
    assert!(
        row.segments()[0].end().y < 0.0,
        "negative odd row mirrors motif"
    );
}

/// Maps authored cubic controls through ordinary Curve Motif realization without losing exact C0 cadence endpoints.
#[test]
fn curve_motif_cubic_path_maps_controls_and_adjacent_endpoints_exactly() {
    let motif = CurvePath::new(
        vec![CurveSegment::CubicBezier(
            CubicBezierSegment::new(
                Point2::new(0.0, 0.0),
                Point2::new(0.25, 0.5),
                Point2::new(0.75, -0.25),
                Point2::new(1.0, 0.0),
            )
            .expect("authored cubic motif validates"),
        )],
        PathClosure::Open,
    )
    .expect("open cubic path validates");
    let source = decode_source(
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><rect width="1" height="1" fill="#ffffff"/></svg>"##,
        SourceFormatHint::Svg,
    )
    .expect("bounded source decodes");
    let realized = realize_curve_motif_canonical_strokes_cancellable(
        "curve-motif-cubic",
        &rows(),
        &motif,
        toniator_domain::AuthoredStructureId(48),
        PathStrokeStyle::default(),
        false,
        None,
        &source,
        &CanvasSpec {
            width: 20.0,
            height: 20.0,
        },
        SourceMapping::canonical(SourceMappingComponent::Red),
        StrokeResponse {
            minimum_thickness: 0.2,
            maximum_thickness: 0.2,
        },
        1024,
        4096,
        &|| false,
        &|_, _| {},
    )
    .expect("canonical cubic motif realizes");
    let path = &realized.strokes[0].path;
    assert_eq!(path.start(), Point2::new(0.0, 0.0));
    assert_eq!(path.end(), Point2::new(20.0, 0.0));
    let CurveSegment::CubicBezier(first) = &path.segments()[0] else {
        panic!("first mapped motif segment remains cubic")
    };
    assert_eq!(first.control_1(), Point2::new(2.5, 5.0));
    assert_eq!(first.control_2(), Point2::new(7.5, -2.5));
    assert_eq!(first.end(), Point2::new(10.0, 0.0));
    assert_eq!(first.end(), path.segments()[1].start());
    let CurveSegment::CubicBezier(second) = &path.segments()[1] else {
        panic!("second mapped motif segment remains cubic")
    };
    assert_eq!(second.control_1(), Point2::new(12.5, 5.0));
    assert_eq!(second.control_2(), Point2::new(17.5, -2.5));
}

/// Realizes source-driven zero, intermediate, and full Curve Motif thickness through canonical strokes.
#[test]
fn curve_motif_canonical_strokes_sample_source_thickness_without_row_length_scaling() {
    let source = decode_source(
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="3" height="1"><rect width="1" height="1" fill="#000000"/><rect x="1" width="1" height="1" fill="#808080"/><rect x="2" width="1" height="1" fill="#ffffff"/></svg>"##,
        SourceFormatHint::Svg,
    )
    .expect("bounded source decodes");
    let motif = CurvePath::polyline(
        vec![
            Point2::new(0.0, 0.0),
            Point2::new(0.5, 0.25),
            Point2::new(1.0, 0.0),
        ],
        PathClosure::Open,
    )
    .expect("asymmetric motif validates");
    let realized = realize_curve_motif_canonical_strokes_cancellable(
        "curve-motif-thickness",
        &rows(),
        &motif,
        toniator_domain::AuthoredStructureId(44),
        PathStrokeStyle::default(),
        false,
        None,
        &source,
        &CanvasSpec {
            width: 20.0,
            height: 10.0,
        },
        SourceMapping::canonical(SourceMappingComponent::Red),
        StrokeResponse {
            minimum_thickness: 0.0,
            maximum_thickness: 1.0,
        },
        1024,
        4096,
        &|| false,
        &|_, _| {},
    )
    .expect("canonical Curve Motif strokes realize");
    let widths: Vec<_> = realized
        .strokes
        .iter()
        .flat_map(|stroke| {
            stroke
                .profile
                .iter()
                .map(|sample| sample.normalized_thickness)
        })
        .collect();
    assert!(widths.iter().any(|width| *width <= 0.01));
    assert!(widths.iter().any(|width| *width >= 0.99));
    assert!(widths.iter().any(|width| (0.1..0.9).contains(width)));
    assert!(
        realized
            .strokes
            .iter()
            .all(|stroke| (stroke.nominal_basis - 10.0).abs() < 1e-12)
    );
}

/// Keeps the continuous Curve Motif centerline while resolving a sharp source zero without a coarse outline wedge.
#[test]
fn curve_motif_sharp_source_zero_refines_the_profile_without_breaking_centerline_c0() {
    let source = decode_source(
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="1"><rect width="2" height="1" fill="#ffffff"/><rect x="2" width="2" height="1" fill="#000000"/></svg>"##,
        SourceFormatHint::Svg,
    )
    .expect("bounded source decodes");
    let motif = CurvePath::line(Point2::new(0.0, 0.0), Point2::new(1.0, 0.0))
        .expect("open motif validates");
    let realized = realize_curve_motif_canonical_strokes_cancellable(
        "curve-motif-sharp-zero",
        &wide_rows(),
        &motif,
        toniator_domain::AuthoredStructureId(47),
        PathStrokeStyle::default(),
        false,
        None,
        &source,
        &CanvasSpec {
            width: 200.0,
            height: 20.0,
        },
        SourceMapping::canonical(SourceMappingComponent::Red),
        StrokeResponse {
            minimum_thickness: 0.0,
            maximum_thickness: 1.0,
        },
        4096,
        16_384,
        &|| false,
        &|_, _| {},
    )
    .expect("canonical Curve Motif stroke realizes");
    let stroke = realized.strokes.first().expect("first row stroke exists");
    assert!(
        stroke
            .path
            .segments()
            .windows(2)
            .all(|pair| pair[0].end() == pair[1].start()),
        "a zero response changes only visible width, never the connected centerline"
    );
    assert!(stroke.profile.iter().any(|sample| sample.width == 0.0));
    let zero_transitions = stroke
        .profile
        .windows(2)
        .filter(|pair| pair[0].normalized_thickness > 0.0 && pair[1].normalized_thickness == 0.0)
        .collect::<Vec<_>>();
    assert!(!zero_transitions.is_empty());
    assert!(
        zero_transitions.iter().all(|pair| {
            (pair[1].center.x - pair[0].center.x).hypot(pair[1].center.y - pair[0].center.y) <= 8.0
        }),
        "a sharp source transition may dissolve the visible outline but cannot span a coarse tapered wedge"
    );
}

/// Reports monotonically increasing segment-and-row work from active Rayon row realization.
#[test]
fn curve_motif_canonical_strokes_report_ordered_live_row_work() {
    let source = decode_source(
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="1"><rect width="2" height="1" fill="#808080"/></svg>"##,
        SourceFormatHint::Svg,
    )
    .expect("bounded source decodes");
    let motif = CurvePath::polyline(
        vec![
            Point2::new(0.0, 0.0),
            Point2::new(0.5, 0.2),
            Point2::new(1.0, 0.0),
        ],
        PathClosure::Open,
    )
    .expect("asymmetric motif validates");
    let observations = Mutex::new(Vec::new());
    let realized = realize_curve_motif_canonical_strokes_cancellable(
        "curve-motif-progress",
        &rows(),
        &motif,
        toniator_domain::AuthoredStructureId(45),
        PathStrokeStyle::default(),
        true,
        Some(0.25),
        &source,
        &CanvasSpec {
            width: 20.0,
            height: 10.0,
        },
        SourceMapping::canonical(SourceMappingComponent::Red),
        StrokeResponse {
            minimum_thickness: 0.2,
            maximum_thickness: 0.8,
        },
        1024,
        4096,
        &|| false,
        &|completed, total| {
            observations
                .lock()
                .expect("progress lock")
                .push((completed, total))
        },
    )
    .expect("canonical Curve Motif strokes realize");
    let observations = observations.into_inner().expect("progress lock unwraps");
    assert_eq!(realized.strokes.len(), 2);
    assert!(observations.len() > realized.strokes.len());
    assert!(observations.windows(2).all(|pair| pair[0].0 < pair[1].0));
    assert!(
        observations
            .iter()
            .all(|(_, total)| *total == observations[0].1)
    );
    assert_eq!(
        observations.last(),
        Some(&(observations[0].1, observations[0].1))
    );
}

/// Rejects bounded Curve Motif profile and outline growth before publishing partial canonical strokes.
#[test]
fn curve_motif_canonical_strokes_report_profile_and_outline_limits() {
    let source = decode_source(
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="1"><rect width="2" height="1" fill="#ffffff"/></svg>"##,
        SourceFormatHint::Svg,
    )
    .expect("bounded source decodes");
    let motif = CurvePath::polyline(
        vec![
            Point2::new(0.0, 0.0),
            Point2::new(0.5, 0.2),
            Point2::new(1.0, 0.0),
        ],
        PathClosure::Open,
    )
    .expect("asymmetric motif validates");
    let realize = |max_profile_samples, max_outline_segments| {
        realize_curve_motif_canonical_strokes_cancellable(
            "curve-motif-limits",
            &rows(),
            &motif,
            toniator_domain::AuthoredStructureId(46),
            PathStrokeStyle::default(),
            false,
            None,
            &source,
            &CanvasSpec {
                width: 20.0,
                height: 10.0,
            },
            SourceMapping::canonical(SourceMappingComponent::Red),
            StrokeResponse {
                minimum_thickness: 0.2,
                maximum_thickness: 0.8,
            },
            max_profile_samples,
            max_outline_segments,
            &|| false,
            &|_, _| {},
        )
    };
    assert_eq!(
        realize(1, 4096)
            .expect_err("one sample cannot realize a Curve Motif")
            .path(),
        "realization.stroke.profile_limit"
    );
    assert_eq!(
        realize(1024, 1)
            .expect_err("one outline segment cannot realize a Curve Motif")
            .path(),
        "curve.outline.segment_limit"
    );
}
