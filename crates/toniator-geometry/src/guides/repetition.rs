use toniator_domain::{AuthoredStructureId, PatternMechanismId};

use crate::{CurveError, CurvePath, GuideInstanceId};

/// One complete derived finite guide path with deterministic dimension/index identity.
#[derive(Clone, Debug, PartialEq)]
pub struct GuidePathInstance {
    pub id: GuideInstanceId,
    pub source_structure_id: Option<AuthoredStructureId>,
    pub path: CurvePath,
}

/// Exact segment-local contributor location for a curve-derived site.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct GuidePathLocationProvenance {
    pub guide_id: GuideInstanceId,
    pub segment_index: usize,
    pub parameter_bits: u64,
}

/// Ordered guide emission for one resolved family root; it preserves caller order exactly.
#[derive(Clone, Debug, PartialEq)]
pub struct GuidePathSet {
    family_fingerprint: String,
    guide_mechanism_id: PatternMechanismId,
    guides: Vec<GuidePathInstance>,
}

impl GuidePathSet {
    /// Validates a nonempty ordered unique guide result without sorting or renumbering it.
    ///
    /// # Errors
    ///
    /// Returns a stable guide-set diagnostic for empty identity, duplicate IDs, or invalid paths.
    pub fn new(
        family_fingerprint: String,
        guide_mechanism_id: PatternMechanismId,
        guides: Vec<GuidePathInstance>,
    ) -> Result<Self, CurveError> {
        if family_fingerprint.is_empty() || guide_mechanism_id.0 == 0 || guides.is_empty() {
            return Err(CurveError::new(
                "curve.guide.path_set",
                "guide path sets require nonempty family identity and guides",
            ));
        }
        let mut ids = std::collections::BTreeSet::new();
        let mut completed_dimensions = std::collections::BTreeSet::new();
        let mut previous_dimension = None;
        let mut previous_identity = None;
        for guide in &guides {
            if !ids.insert(guide.id)
                || (previous_dimension != Some(guide.id.dimension_id)
                    && !completed_dimensions.insert(guide.id.dimension_id))
                || (previous_dimension == Some(guide.id.dimension_id)
                    && previous_identity
                        .is_some_and(|identity: GuideInstanceId| identity >= guide.id))
            {
                return Err(CurveError::new(
                    "curve.guide.path_set",
                    "guide path IDs must be unique in emission order",
                ));
            }
            previous_dimension = Some(guide.id.dimension_id);
            previous_identity = Some(guide.id);
        }
        for guide in &guides {
            guide.path.bounds()?;
        }
        Ok(Self {
            family_fingerprint,
            guide_mechanism_id,
            guides,
        })
    }

    /// Returns ordered immutable derived guide paths.
    pub fn guides(&self) -> &[GuidePathInstance] {
        &self.guides
    }
    /// Returns the family fingerprint associated with this derived immutable guide output.
    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }
    /// Returns the guide root mechanism that owns emitted guide identities.
    pub const fn guide_mechanism_id(&self) -> PatternMechanismId {
        self.guide_mechanism_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CurveSegment, LineSegment, PathClosure, Point2};
    use toniator_domain::GuideDimensionId;

    /// Proves cleanup components at one signed repetition index retain distinct ordered guide identities.
    #[test]
    fn split_components_share_an_index_without_identity_collision() {
        let path = CurvePath::new(
            vec![CurveSegment::Line(
                LineSegment::new(Point2::new(0.0, 0.0), Point2::new(1.0, 0.0))
                    .expect("finite line"),
            )],
            PathClosure::Open,
        )
        .expect("finite path");
        let guides = GuidePathSet::new(
            "split-components".into(),
            PatternMechanismId(1),
            vec![
                GuidePathInstance {
                    id: GuideInstanceId::with_component(GuideDimensionId(1), 2, 0),
                    source_structure_id: None,
                    path: path.clone(),
                },
                GuidePathInstance {
                    id: GuideInstanceId::with_component(GuideDimensionId(1), 2, 1),
                    source_structure_id: None,
                    path,
                },
            ],
        )
        .expect("ordered split components remain unique");
        assert_eq!(guides.guides()[1].id.component_ordinal, 1);
    }
}
