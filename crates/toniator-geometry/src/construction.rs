//! Request-local exact construction policy and opt-in measurements; no authored intent lives here.
use crate::{Bounds, Point2};
use std::{
    cell::RefCell,
    marker::PhantomData,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

/// Distinguishes the complete reference from canvas-certified exact construction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ConstructionMode {
    #[default]
    Complete,
    Canvas,
}
/// Versioned support authority for current canvas-clipped consumers, independent of source intent.
pub const CANVAS_MATERIALIZATION_CONTRACT: &str = "toniator-canvas-materialization-v1";
/// Explicit coverage of a derived materialization; full preparation remains separately retained.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum MaterializationCoverage {
    #[default]
    Complete,
    Canvas {
        canvas: Bounds,
        retained: Bounds,
        contract: &'static str,
    },
}
impl MaterializationCoverage {
    /// Certifies the whole document canvas with outward numerical padding, without an optical filter halo.
    pub fn for_canvas(canvas: Bounds) -> Self {
        let scale = [canvas.min.x, canvas.min.y, canvas.max.x, canvas.max.y]
            .into_iter()
            .map(f64::abs)
            .fold(1.0, f64::max);
        let pad = (1.0e-9 + 128.0 * f64::EPSILON * scale).next_up();
        match canvas.expanded(pad) {
            Some(mut retained) => {
                retained.min.x = retained.min.x.next_down();
                retained.min.y = retained.min.y.next_down();
                retained.max.x = retained.max.x.next_up();
                retained.max.y = retained.max.y.next_up();
                Self::Canvas {
                    canvas,
                    retained,
                    contract: CANVAS_MATERIALIZATION_CONTRACT,
                }
            }
            None => Self::Complete,
        }
    }
    /// Checks exact canvas/contract compatibility; unsupported consumers must select complete preparation.
    pub fn supports(self, canvas: Bounds, contract: &str) -> bool {
        match self {
            Self::Complete => true,
            Self::Canvas {
                canvas: own,
                contract: own_contract,
                ..
            } => own == canvas && own_contract == contract,
        }
    }
    /// Returns a strict common exterior half-plane mask; uncertain arithmetic keeps full geometry.
    pub fn exterior(self, bounds: Bounds) -> u8 {
        let Self::Canvas { retained, .. } = self else {
            return 0;
        };
        u8::from(bounds.max.x < retained.min.x)
            | (u8::from(bounds.min.x > retained.max.x) << 1)
            | (u8::from(bounds.max.y < retained.min.y) << 2)
            | (u8::from(bounds.min.y > retained.max.y) << 3)
    }
}
/// Actual constructor and storage observations, excluded from all product fingerprints.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct ConstructionMetrics {
    pub outline_commands: usize,
    pub outline_constructors: usize,
    pub outline_connectors: usize,
    pub outline_reserved_bytes: usize,
    pub mark_constructors: usize,
    pub mark_reserved_bytes: usize,
    pub logical_grid_candidates: usize,
    pub grid_positions: usize,
    pub grid_search_positions: usize,
    pub grid_predicates: usize,
    pub full_rematerializations: usize,
    pub prepared_outline_bytes: usize,
    pub decision_scratch_bytes: usize,
    pub certificate_scratch_bytes: usize,
    pub prototype_bytes: usize,
    pub mark_slot_bytes: usize,
    pub fingerprint_buffer_bytes: usize,
    pub outline_index_bytes: usize,
    pub deferred_decision_bytes: usize,
}
/// Named observation sites make before-construction savings auditable.
#[derive(Clone, Copy)]
pub enum ConstructionCounter {
    OutlineCommands,
    OutlineConstructors,
    OutlineConnectors,
    OutlineReservedBytes,
    MarkConstructors,
    MarkReservedBytes,
    LogicalGridCandidates,
    GridPositions,
    GridSearchPositions,
    GridPredicates,
    FullRematerializations,
    PreparedOutlineBytes,
    DecisionScratchBytes,
    CertificateScratchBytes,
    PrototypeBytes,
    MarkSlotBytes,
    FingerprintBufferBytes,
    OutlineIndexBytes,
    DeferredDecisionBytes,
}
/// Shared opt-in counters survive Rayon dispatch without global mutable request state.
#[derive(Debug, Default)]
pub struct ConstructionMeasurements {
    values: [AtomicUsize; 19],
}
impl ConstructionMeasurements {
    /// Returns one diagnostic snapshot; overflow saturates measurements only, never creative work.
    pub fn snapshot(&self) -> ConstructionMetrics {
        let v = self.values.each_ref().map(|v| v.load(Ordering::Relaxed));
        ConstructionMetrics {
            outline_commands: v[0],
            outline_constructors: v[1],
            outline_connectors: v[2],
            outline_reserved_bytes: v[3],
            mark_constructors: v[4],
            mark_reserved_bytes: v[5],
            logical_grid_candidates: v[6],
            grid_positions: v[7],
            grid_search_positions: v[8],
            grid_predicates: v[9],
            full_rematerializations: v[10],
            prepared_outline_bytes: v[11],
            decision_scratch_bytes: v[12],
            certificate_scratch_bytes: v[13],
            prototype_bytes: v[14],
            mark_slot_bytes: v[15],
            fingerprint_buffer_bytes: v[16],
            outline_index_bytes: v[17],
            deferred_decision_bytes: v[18],
        }
    }
}
/// Immutable scheduling context; geometry and semantic identity do not depend on its measurements.
#[derive(Clone, Debug)]
pub struct ConstructionContext {
    pub mode: ConstructionMode,
    pub canvas: Option<Bounds>,
    pub measurements: Option<Arc<ConstructionMeasurements>>,
}
thread_local! { static CURRENT: RefCell<Option<ConstructionContext>> = const { RefCell::new(None) }; }
impl ConstructionContext {
    /// Creates a context with optional actual-work accounting for a concrete document canvas.
    pub fn new(mode: ConstructionMode, canvas: Option<Bounds>, measured: bool) -> Self {
        Self {
            mode,
            canvas,
            measurements: measured.then(|| Arc::new(ConstructionMeasurements::default())),
        }
    }
    /// Returns the current request context, defaulting direct low-level calls to complete geometry.
    pub fn current() -> Self {
        CURRENT
            .with(|v| v.borrow().clone())
            .unwrap_or_else(|| Self::new(ConstructionMode::Complete, None, false))
    }
    /// Runs a request or Rayon worker under scoped policy and restores its previous context on unwind.
    pub fn run<T>(&self, action: impl FnOnce() -> T) -> T {
        let _guard = self.enter();
        action()
    }
    /// Installs scoped request policy while preserving an outer opt-in measurement collector.
    pub fn enter(&self) -> ConstructionGuard {
        let previous = CURRENT.with(|v| v.replace(Some(self.clone())));
        ConstructionGuard(previous, PhantomData)
    }
    /// Counts actual reached work only when measurements were explicitly requested.
    pub fn count(&self, counter: ConstructionCounter, amount: usize) {
        if let Some(v) = &self.measurements {
            let _ = v.values[counter as usize].fetch_update(
                Ordering::Relaxed,
                Ordering::Relaxed,
                |v| Some(v.saturating_add(amount)),
            );
        }
    }
    /// Records actual vector capacity for diagnostic storage accounting, without changing allocation policy.
    pub fn count_storage<T>(&self, counter: ConstructionCounter, values: &Vec<T>) {
        self.count(
            counter,
            values.capacity().saturating_mul(std::mem::size_of::<T>()),
        );
    }
    /// Resolves canvas coverage only for explicitly selected optimization mode.
    pub fn coverage(&self) -> MaterializationCoverage {
        if self.mode == ConstructionMode::Canvas {
            self.canvas
                .map(MaterializationCoverage::for_canvas)
                .unwrap_or_default()
        } else {
            MaterializationCoverage::Complete
        }
    }
}
/// Restores a previous thread-local scheduling context without publishing any geometry.
pub struct ConstructionGuard(Option<ConstructionContext>, PhantomData<Rc<()>>);
impl Drop for ConstructionGuard {
    /// Restores the interrupted request context even during error unwinding.
    fn drop(&mut self) {
        CURRENT.with(|v| v.replace(self.0.take()));
    }
}
/// Bounds a point set outward; invalid or overflowing certificates return None and retain full work.
pub fn construction_bounds(points: impl IntoIterator<Item = Point2>) -> Option<Bounds> {
    let mut b = Bounds::from_points(points)?;
    b.min.x = b.min.x.next_down();
    b.min.y = b.min.y.next_down();
    b.max.x = b.max.x.next_up();
    b.max.y = b.max.y.next_up();
    if b.min.is_finite() && b.max.is_finite() {
        Some(b)
    } else {
        None
    }
}
