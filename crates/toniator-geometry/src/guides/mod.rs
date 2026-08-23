//! Derived finite guide-path primitives for the Stage 20D headless boundary.

mod coverage;
mod prototype;
mod repetition;

pub use coverage::{GuideCoveragePlan, GuideDimensionCoverage};
pub use prototype::{construct_circular_arc, resolve_guide_prototype};
pub use repetition::{StructuralPathInstance, StructuralPathSet};
