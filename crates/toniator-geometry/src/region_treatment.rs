//! Geometry-owned closed-region treatment with no renderer topology work.

use std::{collections::BTreeMap, error::Error, fmt};

use toniator_domain::PatternOutputLayerId;

use crate::{
    CanonicalRegionLimits, CanonicalRegionProposal, CanonicalRegionSet, CanonicalRegionSourceGroup,
    CubicBezierSegment, CurvePath, CurveSegment, LineSegment, PathClosure, PathOffsetCleanup,
    PathOffsetEndpointPolicy, PathOffsetRequest, PathOffsetResult, PathOffsetWork, Point2,
    build_canonical_regions_cancellable, build_tagged_canonical_regions_cancellable,
    offset_path_with_work_region_round_cancellable,
};

/// Versioned private treatment contract used by pattern/engine cache identities.
pub const REGION_TREATMENT_CONTRACT_ID: &str = "toniator.region-treatment.v1";

/// Reference point owned by the region producer and excluded from source canonical geometry identity.
#[derive(Clone, Debug, PartialEq)]
pub struct RegionReference {
    /// Identifies exactly one untreated canonical region, including component ordinal.
    pub region_id: crate::CanonicalRegionId,
    /// Supplies the finite affine/sampling origin for that source component.
    pub point: Point2,
}

/// One fill-only treatment resolved by the domain after source sampling.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RegionTreatment {
    /// Replays exact accepted source geometry.
    Full,
    /// Applies a positive affine factor about each producer reference.
    Scale(f64),
    /// Applies an inward signed distance; positive values shrink and negative values grow.
    ConstantGap(f64),
}

/// One resolved treatment value for exactly one untreated canonical base region.
///
/// `treatment: None` deliberately omits the base before geometry construction. It is the sole
/// geometry-level representation of exact-zero sampled alpha and never creates transparent paths.
#[derive(Clone, Debug, PartialEq)]
pub struct RegionTreatmentRequest {
    /// Identifies the untreated canonical region whose components this request may derive.
    pub base_region_id: crate::CanonicalRegionId,
    /// Supplies the producer-owned affine origin when Scale needs one.
    pub reference: Option<Point2>,
    /// Supplies a typed resolved treatment, or omits the base entirely.
    pub treatment: Option<RegionTreatment>,
}

/// Deterministic treated-to-untreated ownership retained outside canonical geometry fingerprints.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegionTreatmentProvenance {
    /// Identifies one canonical treated component in canonical output order.
    pub treated_region_id: crate::CanonicalRegionId,
    /// Identifies the untreated base region that supplied every component construction input.
    pub base_region_id: crate::CanonicalRegionId,
}

/// Complete atomic result of region treatment, including source ownership for sampled paint lookup.
#[derive(Clone, Debug, PartialEq)]
pub struct RegionTreatmentResult {
    /// Stores canonical treated fill rings, which may be empty after collapse or suppression.
    pub regions: CanonicalRegionSet,
    /// Stores one ordered provenance item for every treated canonical region.
    pub provenance: Vec<RegionTreatmentProvenance>,
}

/// Request-wide bounds for treatment construction and canonical post-processing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RegionTreatmentLimits {
    /// Bounds all source/retained region, segment, and inspection work across the request.
    pub canonical: CanonicalRegionLimits,
    /// Bounds each accepted curve-offset primitive invoked by ConstantGap treatment.
    pub path_offset: crate::PathOffsetLimits,
}

impl Default for RegionTreatmentLimits {
    /// Supplies the accepted Stage 20Q canonical and path-offset defaults.
    fn default() -> Self {
        Self {
            canonical: CanonicalRegionLimits::default(),
            path_offset: crate::PathOffsetLimits::default(),
        }
    }
}

/// Stable treatment failure that never exposes partial treated geometry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegionTreatmentError {
    path: &'static str,
    message: &'static str,
}

impl RegionTreatmentError {
    /// Returns the stable failure path.
    pub const fn path(&self) -> &'static str {
        self.path
    }
    /// Returns the stable failure message.
    pub const fn message(&self) -> &'static str {
        self.message
    }
}

impl fmt::Display for RegionTreatmentError {
    /// Formats the stable treatment failure without exposing a partial candidate.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path, self.message)
    }
}

impl Error for RegionTreatmentError {}

/// Treats independently resolved base regions and returns canonical geometry plus ownership.
///
/// Every request must address exactly one accepted untreated region. Full, Scale one, and zero
/// ConstantGap replay the complete source set byte-for-byte when every base is retained; omitted
/// bases are removed before construction, which is how exact-zero sampled alpha suppresses fills.
/// All other output passes through the canonical positive-ring builder, making component ordinals
/// contiguous within each canonical source identity after deterministic ordering.
///
/// # Errors
///
/// Returns stable identity, geometry, allocation, canonical-limit, offset, or cancellation
/// diagnostics without exposing a partial treated set or partially aligned provenance table.
pub fn treat_region_requests_cancellable(
    output_layer_id: PatternOutputLayerId,
    source: &CanonicalRegionSet,
    requests: &[RegionTreatmentRequest],
    limits: RegionTreatmentLimits,
    cancelled: impl Fn() -> bool,
) -> Result<RegionTreatmentResult, RegionTreatmentError> {
    let cancelled_ref = &cancelled;
    poll_treatment(cancelled_ref)?;
    if requests.len() != source.regions().len() {
        return Err(RegionTreatmentError {
            path: "region.treatment.identity.requests",
            message: "every untreated region requires exactly one treatment request",
        });
    }
    let mut source_by_id = BTreeMap::new();
    for region in source.regions() {
        if region.id.output_layer_id != output_layer_id {
            return Err(RegionTreatmentError {
                path: "region.treatment.identity.output",
                message: "treatment output identity must match every untreated region",
            });
        }
        source_by_id.insert(region.id.clone(), region);
    }
    let mut requests_by_id = BTreeMap::new();
    for request in requests {
        poll_treatment(cancelled_ref)?;
        if !source_by_id.contains_key(&request.base_region_id) {
            return Err(RegionTreatmentError {
                path: "region.treatment.identity.base",
                message: "treatment request must address an accepted untreated region",
            });
        }
        if requests_by_id
            .insert(request.base_region_id.clone(), request)
            .is_some()
        {
            return Err(RegionTreatmentError {
                path: "region.treatment.identity.duplicate",
                message: "treatment requests must not repeat an untreated region",
            });
        }
        validate_treatment_request(request)?;
    }
    if requests_by_id.len() != source_by_id.len() {
        return Err(RegionTreatmentError {
            path: "region.treatment.identity.requests",
            message: "treatment requests must cover every untreated region",
        });
    }
    if requests.iter().all(RegionTreatmentRequest::is_identity) {
        return Ok(RegionTreatmentResult {
            regions: source.clone(),
            provenance: source
                .regions()
                .iter()
                .map(|region| RegionTreatmentProvenance {
                    treated_region_id: region.id.clone(),
                    base_region_id: region.id.clone(),
                })
                .collect(),
        });
    }
    let mut offset_work =
        PathOffsetWork::new(limits.path_offset).map_err(|error| RegionTreatmentError {
            path: "region.treatment.limits.offset",
            message: error.message(),
        })?;

    let mut groups: BTreeMap<crate::CanonicalRegionSourceId, Vec<(CurvePath, u64)>> =
        BTreeMap::new();
    let mut owners = Vec::new();
    for (base_id, region) in source_by_id {
        poll_treatment(cancelled_ref)?;
        let request = requests_by_id[&base_id];
        let Some(treatment) = request.treatment else {
            continue;
        };
        let components = match treatment {
            RegionTreatment::Full => vec![region.ring.clone()],
            RegionTreatment::Scale(factor) => {
                if factor == 0.0 {
                    Vec::new()
                } else {
                    vec![scale_path(
                        &region.ring,
                        request.reference.expect("validated Scale reference"),
                        factor,
                    )?]
                }
            }
            RegionTreatment::ConstantGap(gap) => offset_region_path_with_work(
                &region.ring,
                gap / 2.0,
                &mut offset_work,
                cancelled_ref,
            )?,
        };
        if components.is_empty() {
            continue;
        }
        let owner_tag = u64::try_from(owners.len()).map_err(|_| RegionTreatmentError {
            path: "region.treatment.allocation.provenance",
            message: "treated base-owner ordinal exceeds u64",
        })?;
        owners.push(base_id);
        groups
            .entry(region.id.source_id.clone())
            .or_default()
            .extend(
                components
                    .into_iter()
                    .map(|component| (component, owner_tag)),
            );
    }
    if groups.is_empty() {
        return Ok(RegionTreatmentResult {
            regions: CanonicalRegionSet::empty(),
            provenance: Vec::new(),
        });
    }
    let mut source_groups = Vec::new();
    source_groups
        .try_reserve(groups.len())
        .map_err(|_| RegionTreatmentError {
            path: "region.treatment.allocation.groups",
            message: "treated source-group allocation failed",
        })?;
    for (source_id, components) in groups {
        source_groups.push(crate::TaggedCanonicalRegionSourceGroup {
            source_id,
            components,
        });
    }
    let (regions, _, tags) = build_tagged_canonical_regions_cancellable(
        output_layer_id,
        source_groups,
        limits.canonical,
        cancelled_ref,
    )
    .map_err(map_canonical_treatment_error)?;
    let mut provenance = Vec::new();
    provenance
        .try_reserve(regions.regions().len())
        .map_err(|_| RegionTreatmentError {
            path: "region.treatment.allocation.provenance",
            message: "treated provenance allocation failed",
        })?;
    for (treated, owner_tag) in regions.regions().iter().zip(tags) {
        poll_treatment(cancelled_ref)?;
        provenance.push(RegionTreatmentProvenance {
            treated_region_id: treated.id.clone(),
            base_region_id: owners
                .get(
                    usize::try_from(owner_tag).map_err(|_| RegionTreatmentError {
                        path: "region.treatment.identity.provenance",
                        message: "treated component owner tag exceeds usize",
                    })?,
                )
                .ok_or(RegionTreatmentError {
                    path: "region.treatment.identity.provenance",
                    message: "treated component must retain one base owner",
                })?
                .clone(),
        });
    }
    Ok(RegionTreatmentResult {
        regions,
        provenance,
    })
}

/// Validates the typed treatment and its required producer reference before construction.
///
/// # Errors
///
/// Returns stable region-treatment identity or geometry diagnostics for invalid resolved values.
fn validate_treatment_request(
    request: &RegionTreatmentRequest,
) -> Result<(), RegionTreatmentError> {
    match request.treatment {
        None | Some(RegionTreatment::Full) => Ok(()),
        Some(RegionTreatment::Scale(factor)) => {
            if !factor.is_finite() || factor < 0.0 {
                return Err(RegionTreatmentError {
                    path: "region.treatment.geometry.scale",
                    message: "region Scale factor must be finite and nonnegative",
                });
            }
            if !request.reference.is_some_and(Point2::is_finite) {
                return Err(RegionTreatmentError {
                    path: "region.treatment.identity.reference",
                    message: "region Scale requires a finite producer reference",
                });
            }
            Ok(())
        }
        Some(RegionTreatment::ConstantGap(gap)) if gap.is_finite() => Ok(()),
        Some(RegionTreatment::ConstantGap(_)) => Err(RegionTreatmentError {
            path: "region.treatment.geometry.gap",
            message: "region ConstantGap must be finite",
        }),
    }
}

/// Polls cancellation at a treatment-owned boundary.
///
/// # Errors
///
/// Returns only the canonical evaluation cancellation diagnostic.
fn poll_treatment(cancelled: &dyn Fn() -> bool) -> Result<(), RegionTreatmentError> {
    (!cancelled()).then_some(()).ok_or(RegionTreatmentError {
        path: "evaluation.cancelled",
        message: "evaluation cancelled",
    })
}

/// Maps canonical post-processing failures into the Stage 20Q treatment diagnostic namespace.
fn map_canonical_treatment_error(error: crate::CanonicalRegionError) -> RegionTreatmentError {
    RegionTreatmentError {
        path: match error.path() {
            "evaluation.cancelled" => "evaluation.cancelled",
            path if path.starts_with("region.limits") => "region.treatment.limits.canonical",
            path if path.starts_with("region.allocation") => {
                "region.treatment.allocation.canonical"
            }
            _ => "region.treatment.geometry.canonical",
        },
        message: error.message(),
    }
}

impl RegionTreatmentRequest {
    /// Reports whether this request retains a base with exact accepted geometry and identity.
    fn is_identity(&self) -> bool {
        matches!(
            self.treatment,
            Some(RegionTreatment::Full)
                | Some(RegionTreatment::Scale(1.0))
                | Some(RegionTreatment::ConstantGap(0.0))
        )
    }
}

/// Treats complete canonical source regions, preserving final clipping as a renderer-only concern.
///
/// # Errors
///
/// Returns identity, geometry, allocation, canonical-limit, or cancellation diagnostics without
/// publishing a partially aligned treated set.
pub fn treat_regions_cancellable(
    output_layer_id: PatternOutputLayerId,
    source: &CanonicalRegionSet,
    references: &[RegionReference],
    treatment: RegionTreatment,
    cancelled: impl Fn() -> bool,
) -> Result<CanonicalRegionSet, RegionTreatmentError> {
    if cancelled() {
        return Err(RegionTreatmentError {
            path: "evaluation.cancelled",
            message: "evaluation cancelled",
        });
    }
    if matches!(treatment, RegionTreatment::Full) {
        return Ok(source.clone());
    }
    let factor = match treatment {
        RegionTreatment::Scale(value) => value,
        _ => 1.0,
    };
    if matches!(treatment, RegionTreatment::Scale(_)) && (!factor.is_finite() || factor < 0.0) {
        return Err(RegionTreatmentError {
            path: "region.treatment.geometry.scale",
            message: "region Scale factor must be finite and nonnegative",
        });
    }
    if matches!(treatment, RegionTreatment::Scale(0.0)) {
        return Ok(CanonicalRegionSet::empty());
    }
    let mut groups = Vec::with_capacity(source.regions().len());
    for region in source.regions() {
        if cancelled() {
            return Err(RegionTreatmentError {
                path: "evaluation.cancelled",
                message: "evaluation cancelled",
            });
        }
        let reference = references
            .iter()
            .find(|entry| entry.region_id == region.id)
            .ok_or(RegionTreatmentError {
                path: "region.treatment.identity.reference",
                message: "every region treatment requires a matching source reference",
            })?;
        if !reference.point.is_finite() {
            return Err(RegionTreatmentError {
                path: "region.treatment.identity.reference",
                message: "region treatment reference must be finite",
            });
        }
        let components = match treatment {
            RegionTreatment::Scale(_) => vec![scale_path(&region.ring, reference.point, factor)?],
            RegionTreatment::ConstantGap(gap) => offset_region_path(
                &region.ring,
                gap / 2.0,
                crate::PathOffsetLimits::default(),
                &cancelled,
            )?,
            RegionTreatment::Full => unreachable!(),
        };
        if !components.is_empty() {
            groups.push(CanonicalRegionSourceGroup {
                source_id: region.id.source_id.clone(),
                components,
            });
        }
    }
    if groups.is_empty() {
        return Ok(CanonicalRegionSet::empty());
    }
    build_canonical_regions_cancellable(
        CanonicalRegionProposal {
            output_layer_id,
            source_groups: groups,
        },
        CanonicalRegionLimits::default(),
        cancelled,
    )
    .map(|(regions, _)| regions)
    .map_err(|error| RegionTreatmentError {
        path: match error.path() {
            "evaluation.cancelled" => "evaluation.cancelled",
            path if path.starts_with("region.limits") => "region.treatment.limits.canonical",
            path if path.starts_with("region.allocation") => {
                "region.treatment.allocation.canonical"
            }
            _ => "region.treatment.geometry.canonical",
        },
        message: error.message(),
    })
}

/// Builds inward signed-gap components through the accepted closed-path offset primitive.
///
/// # Errors
///
/// Maps reusable offset failures to the Stage 20Q geometry/cancellation boundary.
fn offset_region_path(
    path: &CurvePath,
    inward_distance: f64,
    limits: crate::PathOffsetLimits,
    cancelled: &impl Fn() -> bool,
) -> Result<Vec<CurvePath>, RegionTreatmentError> {
    if !inward_distance.is_finite() {
        return Err(RegionTreatmentError {
            path: "region.treatment.geometry.gap",
            message: "region ConstantGap must be finite",
        });
    }
    if inward_distance == 0.0 {
        return Ok(vec![path.clone()]);
    }
    let mut work = PathOffsetWork::new(limits).map_err(|error| RegionTreatmentError {
        path: "region.treatment.limits.offset",
        message: error.message(),
    })?;
    // Canonical rings are counter-clockwise, so their left normal points into the filled region.
    match offset_path_with_work_region_round_cancellable(
        PathOffsetRequest {
            path,
            signed_distance: inward_distance,
            endpoint_policy: PathOffsetEndpointPolicy::Preserve,
            cleanup: PathOffsetCleanup::DissolveCrossings,
            crossing_barriers: &[],
            limits,
        },
        &mut work,
        cancelled,
    ) {
        Ok(PathOffsetResult::Paths(components)) => Ok(components
            .into_iter()
            .filter_map(|component| {
                (component.path.closure() == PathClosure::Closed).then_some(component.path)
            })
            .collect()),
        Ok(PathOffsetResult::Collapsed) => Ok(Vec::new()),
        Err(error) => Err(RegionTreatmentError {
            path: match error.path() {
                "evaluation.cancelled" => "evaluation.cancelled",
                path if path.contains("limit") => "region.treatment.limits.offset",
                path if path.contains("allocation") => "region.treatment.allocation.offset",
                _ => "region.treatment.geometry.gap",
            },
            message: error.message(),
        }),
    }
}

/// Builds one signed-gap result while charging the treatment request's shared offset budget.
///
/// # Errors
///
/// Maps reusable shared-work offset failures into the Stage 20Q treatment namespace atomically.
fn offset_region_path_with_work(
    path: &CurvePath,
    inward_distance: f64,
    work: &mut PathOffsetWork,
    cancelled: &impl Fn() -> bool,
) -> Result<Vec<CurvePath>, RegionTreatmentError> {
    if !inward_distance.is_finite() {
        return Err(RegionTreatmentError {
            path: "region.treatment.geometry.gap",
            message: "region ConstantGap must be finite",
        });
    }
    if inward_distance == 0.0 {
        return Ok(vec![path.clone()]);
    }
    match offset_path_with_work_region_round_cancellable(
        PathOffsetRequest {
            path,
            signed_distance: inward_distance,
            endpoint_policy: PathOffsetEndpointPolicy::Preserve,
            cleanup: PathOffsetCleanup::DissolveCrossings,
            crossing_barriers: &[],
            limits: work.limits(),
        },
        work,
        cancelled,
    ) {
        Ok(PathOffsetResult::Paths(components)) => Ok(components
            .into_iter()
            .filter_map(|component| {
                (component.path.closure() == PathClosure::Closed).then_some(component.path)
            })
            .collect()),
        Ok(PathOffsetResult::Collapsed) => Ok(Vec::new()),
        Err(error) => Err(RegionTreatmentError {
            path: match error.path() {
                "evaluation.cancelled" => "evaluation.cancelled",
                path if path.contains("limit") => "region.treatment.limits.offset",
                path if path.contains("allocation") => "region.treatment.allocation.offset",
                _ => "region.treatment.geometry.gap",
            },
            message: error.message(),
        }),
    }
}

/// Affinely transforms every line/cubic construction point around a finite source reference.
///
/// # Errors
///
/// Returns geometry validation failures without changing closure or segment kind.
fn scale_path(
    path: &CurvePath,
    reference: Point2,
    factor: f64,
) -> Result<CurvePath, RegionTreatmentError> {
    let point = |value: Point2| {
        Point2::new(
            reference.x + (value.x - reference.x) * factor,
            reference.y + (value.y - reference.y) * factor,
        )
    };
    let mut segments = Vec::with_capacity(path.segments().len());
    for segment in path.segments() {
        let transformed = match segment {
            CurveSegment::Line(line) => CurveSegment::Line(
                LineSegment::new(point(line.start()), point(line.end())).map_err(|_| {
                    RegionTreatmentError {
                        path: "region.treatment.geometry.scale",
                        message: "scaled line coordinates must remain finite",
                    }
                })?,
            ),
            CurveSegment::CubicBezier(cubic) => CurveSegment::CubicBezier(
                CubicBezierSegment::new(
                    point(cubic.start()),
                    point(cubic.control_1()),
                    point(cubic.control_2()),
                    point(cubic.end()),
                )
                .map_err(|_| RegionTreatmentError {
                    path: "region.treatment.geometry.scale",
                    message: "scaled cubic coordinates must remain finite",
                })?,
            ),
        };
        segments.push(transformed);
    }
    CurvePath::new(segments, PathClosure::Closed).map_err(|_| RegionTreatmentError {
        path: "region.treatment.geometry.scale",
        message: "scaled region must remain closed and connected",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use toniator_domain::PatternMechanismId;

    /// Builds one canonical source set with a unique source identity per supplied closed ring.
    fn stage20q_source(paths: Vec<CurvePath>) -> CanonicalRegionSet {
        build_canonical_regions_cancellable(
            CanonicalRegionProposal {
                output_layer_id: PatternOutputLayerId(71),
                source_groups: paths
                    .into_iter()
                    .enumerate()
                    .map(|(ordinal, path)| CanonicalRegionSourceGroup {
                        source_id: crate::CanonicalRegionSourceId::SiteOwners(vec![
                            crate::FamilySiteId {
                                mechanism_id: PatternMechanismId(9),
                                ordinal,
                            },
                        ]),
                        components: vec![path],
                    })
                    .collect(),
            },
            CanonicalRegionLimits::default(),
            || false,
        )
        .unwrap()
        .0
    }

    /// Builds one positive closed triangle suitable for treatment witnesses.
    fn stage20q_triangle(left: f64) -> CurvePath {
        CurvePath::polyline(
            vec![
                Point2::new(left, 0.0),
                Point2::new(left + 2.0, 0.0),
                Point2::new(left, 2.0),
            ],
            PathClosure::Closed,
        )
        .unwrap()
    }

    /// Builds a request with the supplied typed treatment for one canonical base region.
    fn stage20q_request(
        region: &crate::CanonicalRegion,
        treatment: Option<RegionTreatment>,
        reference: Option<Point2>,
    ) -> RegionTreatmentRequest {
        RegionTreatmentRequest {
            base_region_id: region.id.clone(),
            reference,
            treatment,
        }
    }

    /// Builds two disjoint untreated ordinals deliberately sharing one source identity.
    fn stage20q_shared_source() -> CanonicalRegionSet {
        build_canonical_regions_cancellable(
            CanonicalRegionProposal {
                output_layer_id: PatternOutputLayerId(71),
                source_groups: vec![CanonicalRegionSourceGroup {
                    source_id: crate::CanonicalRegionSourceId::SiteOwners(vec![
                        crate::FamilySiteId {
                            mechanism_id: PatternMechanismId(9),
                            ordinal: 99,
                        },
                    ]),
                    components: vec![stage20q_triangle(0.0), stage20q_triangle(8.0)],
                }],
            },
            CanonicalRegionLimits::default(),
            || false,
        )
        .unwrap()
        .0
    }

    /// Verifies Full requests replay accepted canonical geometry bytes and IDs exactly.
    #[test]
    fn stage20q_full_replays_identity_and_provenance() {
        let source = stage20q_source(vec![stage20q_triangle(0.0)]);
        let result = treat_region_requests_cancellable(
            PatternOutputLayerId(71),
            &source,
            &[stage20q_request(
                &source.regions()[0],
                Some(RegionTreatment::Full),
                None,
            )],
            RegionTreatmentLimits::default(),
            || false,
        )
        .unwrap();
        assert_eq!(result.regions, source);
        assert_eq!(result.provenance.len(), 1);
        assert_eq!(
            result.provenance[0].treated_region_id,
            result.provenance[0].base_region_id
        );
    }

    /// Verifies factor zero and omitted bases publish an empty canonical treated set with no paint owner.
    #[test]
    fn stage20q_scale_zero_and_omission_remove_base_geometry() {
        let source = stage20q_source(vec![stage20q_triangle(0.0), stage20q_triangle(4.0)]);
        let result = treat_region_requests_cancellable(
            PatternOutputLayerId(71),
            &source,
            &[
                stage20q_request(
                    &source.regions()[0],
                    Some(RegionTreatment::Scale(0.0)),
                    Some(Point2::new(0.0, 0.0)),
                ),
                stage20q_request(&source.regions()[1], None, None),
            ],
            RegionTreatmentLimits::default(),
            || false,
        )
        .unwrap();
        assert!(result.regions.regions().is_empty());
        assert!(result.provenance.is_empty());
    }

    /// Verifies shared-source bases charge one request and fail atomically when cancellation
    /// arrives after the first base has been transformed but before provenance can publish.
    #[test]
    fn stage20q_shared_source_late_cancellation_is_atomic() {
        let source = stage20q_shared_source();
        let polls = std::cell::Cell::new(0usize);
        let error = treat_region_requests_cancellable(
            PatternOutputLayerId(71),
            &source,
            &[
                stage20q_request(
                    &source.regions()[0],
                    Some(RegionTreatment::Scale(0.5)),
                    Some(Point2::new(0.0, 0.0)),
                ),
                stage20q_request(
                    &source.regions()[1],
                    Some(RegionTreatment::ConstantGap(0.0)),
                    None,
                ),
            ],
            RegionTreatmentLimits::default(),
            || {
                let next = polls.get() + 1;
                polls.set(next);
                next > 6
            },
        )
        .unwrap_err();
        assert_eq!(error.path(), "evaluation.cancelled");
    }

    /// Verifies Scale transforms all cubic construction points about a producer reference outside the ring.
    #[test]
    fn stage20q_scale_transforms_cubic_about_outside_reference() {
        let cubic = CubicBezierSegment::new(
            Point2::new(2.0, 0.0),
            Point2::new(3.0, 0.0),
            Point2::new(3.0, 1.0),
            Point2::new(2.0, 2.0),
        )
        .unwrap();
        let path = CurvePath::new(
            vec![
                CurveSegment::CubicBezier(cubic),
                CurveSegment::Line(
                    LineSegment::new(Point2::new(2.0, 2.0), Point2::new(0.0, 0.0)).unwrap(),
                ),
                CurveSegment::Line(
                    LineSegment::new(Point2::new(0.0, 0.0), Point2::new(2.0, 0.0)).unwrap(),
                ),
            ],
            PathClosure::Closed,
        )
        .unwrap();
        let source = stage20q_source(vec![path]);
        let result = treat_region_requests_cancellable(
            PatternOutputLayerId(71),
            &source,
            &[stage20q_request(
                &source.regions()[0],
                Some(RegionTreatment::Scale(2.0)),
                Some(Point2::new(-1.0, -1.0)),
            )],
            RegionTreatmentLimits::default(),
            || false,
        )
        .unwrap();
        assert_eq!(result.provenance[0].base_region_id, source.regions()[0].id);
        assert!(result.regions.regions()[0].bounds.max.x > source.regions()[0].bounds.max.x);
        assert!(result.regions.regions()[0].area > source.regions()[0].area);
    }

    /// Verifies Scale one and zero ConstantGap take the exact identity replay path.
    #[test]
    fn stage20q_identity_numeric_treatments_replay_source_bytes() {
        let source = stage20q_source(vec![stage20q_triangle(0.0), stage20q_triangle(4.0)]);
        let result = treat_region_requests_cancellable(
            PatternOutputLayerId(71),
            &source,
            &[
                stage20q_request(
                    &source.regions()[0],
                    Some(RegionTreatment::Scale(1.0)),
                    Some(Point2::new(99.0, 99.0)),
                ),
                stage20q_request(
                    &source.regions()[1],
                    Some(RegionTreatment::ConstantGap(0.0)),
                    None,
                ),
            ],
            RegionTreatmentLimits::default(),
            || false,
        )
        .unwrap();
        assert_eq!(result.regions.fingerprint(), source.fingerprint());
        assert_eq!(result.regions, source);
    }

    /// Verifies signed ConstantGap applies inward positive and outward negative distances.
    #[test]
    fn stage20q_signed_constant_gap_has_inward_and_outward_semantics() {
        let source = stage20q_source(vec![stage20q_triangle(0.0)]);
        let inward = treat_region_requests_cancellable(
            PatternOutputLayerId(71),
            &source,
            &[stage20q_request(
                &source.regions()[0],
                Some(RegionTreatment::ConstantGap(0.25)),
                None,
            )],
            RegionTreatmentLimits::default(),
            || false,
        )
        .unwrap();
        let outward = treat_region_requests_cancellable(
            PatternOutputLayerId(71),
            &source,
            &[stage20q_request(
                &source.regions()[0],
                Some(RegionTreatment::ConstantGap(-0.25)),
                None,
            )],
            RegionTreatmentLimits::default(),
            || false,
        )
        .unwrap();
        assert!(inward.regions.regions()[0].area < source.regions()[0].area);
        assert!(outward.regions.regions()[0].area > source.regions()[0].area);
    }

    /// Verifies cancellation and aggregate canonical limits reject the whole treatment atomically.
    #[test]
    fn stage20q_treatment_cancellation_and_canonical_limit_are_atomic() {
        let source = stage20q_source(vec![stage20q_triangle(0.0), stage20q_triangle(4.0)]);
        let requests: Vec<_> = source
            .regions()
            .iter()
            .map(|region| {
                stage20q_request(
                    region,
                    Some(RegionTreatment::Scale(2.0)),
                    Some(Point2::new(0.0, 0.0)),
                )
            })
            .collect();
        let cancelled = treat_region_requests_cancellable(
            PatternOutputLayerId(71),
            &source,
            &requests,
            RegionTreatmentLimits::default(),
            || true,
        )
        .unwrap_err();
        assert_eq!(cancelled.path(), "evaluation.cancelled");
        let limited = treat_region_requests_cancellable(
            PatternOutputLayerId(71),
            &source,
            &requests,
            RegionTreatmentLimits {
                canonical: CanonicalRegionLimits::new(1, 8, 64, 64).unwrap(),
                ..RegionTreatmentLimits::default()
            },
            || false,
        )
        .unwrap_err();
        assert_eq!(limited.path(), "region.treatment.limits.canonical");
    }

    /// Verifies cumulative ConstantGap work fails atomically instead of resetting per base.
    #[test]
    fn stage20q_constant_gap_exhausts_shared_offset_budget_atomically() {
        let source = stage20q_source(vec![stage20q_triangle(0.0), stage20q_triangle(4.0)]);
        let requests: Vec<_> = source
            .regions()
            .iter()
            .map(|region| stage20q_request(region, Some(RegionTreatment::ConstantGap(0.25)), None))
            .collect();
        let error = treat_region_requests_cancellable(
            PatternOutputLayerId(71),
            &source,
            &requests,
            RegionTreatmentLimits {
                path_offset: crate::PathOffsetLimits {
                    maximum_segments: 1,
                    ..crate::PathOffsetLimits::default()
                },
                ..RegionTreatmentLimits::default()
            },
            || false,
        )
        .unwrap_err();
        assert_eq!(error.path(), "region.treatment.limits.offset");
    }

    /// Verifies late request cancellation prevents a mixed Scale/Gap candidate from publishing.
    #[test]
    fn stage20q_mixed_treatment_polls_cancellation_across_bases() {
        use std::cell::Cell;

        let source = stage20q_source(vec![stage20q_triangle(0.0), stage20q_triangle(4.0)]);
        let requests = vec![
            stage20q_request(
                &source.regions()[0],
                Some(RegionTreatment::Scale(1.5)),
                Some(Point2::new(0.0, 0.0)),
            ),
            stage20q_request(
                &source.regions()[1],
                Some(RegionTreatment::ConstantGap(0.25)),
                None,
            ),
        ];
        let polls = Cell::new(0_u32);
        let error = treat_region_requests_cancellable(
            PatternOutputLayerId(71),
            &source,
            &requests,
            RegionTreatmentLimits::default(),
            || {
                let next = polls.get() + 1;
                polls.set(next);
                next > 5
            },
        )
        .unwrap_err();
        assert_eq!(error.path(), "evaluation.cancelled");
        assert!(polls.get() > 5);
    }
}
