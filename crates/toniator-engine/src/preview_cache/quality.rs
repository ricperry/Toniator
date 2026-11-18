//! Typed final-consumer identity; document revisions and canonical geometry remain separate.
use toniator_render::PreviewRasterTarget;

/// Artist-requested/effective presentation precision; it never enters authored document state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Quality {
    #[default]
    Accurate,
    Fast,
}
/// Distinguishes numeric consumers even when they render the same scene and physical region.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Backend {
    CpuArea,
    HybridGpu,
}
/// Versioned consumer algorithm; changing it cannot reuse an earlier pixel/failure namespace.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Algorithm {
    AccurateAreaV1,
    RefinedHybridCircleV1,
}
/// Explicit presentation identity independent of document revision, source or canonical scene.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Provenance {
    pub requested: Quality,
    pub effective: Quality,
    pub backend: Backend,
    pub algorithm: Algorithm,
    pub epoch: u64,
    pub device_epoch: u64,
}
impl Default for Provenance {
    /// Starts every cache/controller in Accurate without initializing a GPU.
    fn default() -> Self {
        Self::accurate(0, 0, Quality::Accurate)
    }
}
impl Provenance {
    /// Constructs the fixed CPU namespace, including truthful Fast-request fallback intent.
    pub const fn accurate(epoch: u64, device_epoch: u64, requested: Quality) -> Self {
        Self {
            requested,
            effective: Quality::Accurate,
            backend: Backend::CpuArea,
            algorithm: Algorithm::AccurateAreaV1,
            epoch,
            device_epoch,
        }
    }
    /// Constructs the pinned approximate namespace without asserting Accurate coverage.
    pub const fn fast(epoch: u64, device_epoch: u64) -> Self {
        Self {
            requested: Quality::Fast,
            effective: Quality::Fast,
            backend: Backend::HybridGpu,
            algorithm: Algorithm::RefinedHybridCircleV1,
            epoch,
            device_epoch,
        }
    }
}
/// Distinguishes exact fitted/cropped targets from natural retained zoom roots.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Projection {
    #[default]
    Natural,
    Target(PreviewRasterTarget),
}
/// Includes backend/algorithm/epoch in every application texture and upload identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PixelIdentity {
    pub provenance: Provenance,
    pub id: u64,
}
/// Typed whole-scene/runtime refusal; only the caller's Accurate consumer may supply replacement pixels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FallbackReason {
    Unsupported(String),
    Resource(String),
    Unavailable(String),
    Device(String),
    Cancelled,
}
/// Complete exact failure intent; pans do not change scene/target/device retry identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FailureIntent {
    pub scene: String,
    /// Complete target/backdrop/work-policy/backend/version namespace; retry scopes ignore only
    /// explicitly irrelevant epoch/target components for their typed failure class.
    pub key: super::RootKey,
}
