use crate::Bounds;

/// Inclusive signed instance range planned for one generic guide dimension.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuideDimensionCoverage {
    pub first_index: i64,
    pub last_index: i64,
}

/// Complete local generation domain and ordered per-dimension instance coverage proof.
#[derive(Clone, Debug, PartialEq)]
pub struct GuideCoveragePlan {
    pub generation_domain: Bounds,
    pub per_dimension: Vec<GuideDimensionCoverage>,
}
