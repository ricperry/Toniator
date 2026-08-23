use toniator_domain::{AuthoredStructureId, PatternMechanismId};

use crate::{CurveError, CurvePath, StructuralPathInstanceId};

/// One complete derived finite structural path with path-neutral source identity.
#[derive(Clone, Debug, PartialEq)]
pub struct StructuralPathInstance {
    pub id: StructuralPathInstanceId,
    pub source_structure_id: Option<AuthoredStructureId>,
    pub path: CurvePath,
}

/// Ordered structural-path emission for one resolved family root; it preserves caller order exactly.
#[derive(Clone, Debug, PartialEq)]
pub struct StructuralPathSet {
    family_fingerprint: String,
    source_mechanism_id: PatternMechanismId,
    paths: Vec<StructuralPathInstance>,
}

impl StructuralPathSet {
    /// Validates a nonempty ordered unique guide result without sorting or renumbering it.
    ///
    /// # Errors
    ///
    /// Returns a stable guide-set diagnostic for empty identity, duplicate IDs, or invalid paths.
    pub fn new(
        family_fingerprint: String,
        source_mechanism_id: PatternMechanismId,
        paths: Vec<StructuralPathInstance>,
    ) -> Result<Self, CurveError> {
        if family_fingerprint.is_empty() || source_mechanism_id.0 == 0 || paths.is_empty() {
            return Err(CurveError::new(
                "curve.guide.path_set",
                "guide path sets require nonempty family identity and guides",
            ));
        }
        let mut ids = std::collections::BTreeSet::new();
        let mut completed_sources = std::collections::BTreeSet::new();
        let mut previous_source = None;
        let mut previous_identity = None;
        for guide in &paths {
            if !ids.insert(guide.id)
                || (previous_source != Some(guide.id.source)
                    && !completed_sources.insert(guide.id.source))
                || (previous_source == Some(guide.id.source)
                    && previous_identity
                        .is_some_and(|identity: StructuralPathInstanceId| identity >= guide.id))
            {
                return Err(CurveError::new(
                    "curve.guide.path_set",
                    "guide path IDs must be unique in emission order",
                ));
            }
            previous_source = Some(guide.id.source);
            previous_identity = Some(guide.id);
        }
        for guide in &paths {
            guide.path.bounds()?;
        }
        Ok(Self {
            family_fingerprint,
            source_mechanism_id,
            paths,
        })
    }

    /// Returns ordered immutable derived structural paths.
    pub fn paths(&self) -> &[StructuralPathInstance] {
        &self.paths
    }
    /// Returns the family fingerprint associated with this derived immutable guide output.
    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }
    /// Returns the source mechanism that owns emitted structural-path identities.
    pub const fn source_mechanism_id(&self) -> PatternMechanismId {
        self.source_mechanism_id
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
        let paths = StructuralPathSet::new(
            "split-components".into(),
            PatternMechanismId(1),
            vec![
                StructuralPathInstance {
                    id: StructuralPathInstanceId::guide_dimension(GuideDimensionId(1), 2, 0),
                    source_structure_id: None,
                    path: path.clone(),
                },
                StructuralPathInstance {
                    id: StructuralPathInstanceId::guide_dimension(GuideDimensionId(1), 2, 1),
                    source_structure_id: None,
                    path,
                },
            ],
        )
        .expect("ordered split components remain unique");
        assert_eq!(paths.paths()[1].id.component_ordinal, 1);
    }
}
