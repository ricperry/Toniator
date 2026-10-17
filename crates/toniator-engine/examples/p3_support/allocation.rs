//! Example-only atomic System wrapper; reports requested bytes, excluding allocator metadata.
use serde_json::{Value, json};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
};
pub struct CountingAllocator;
static LIVE: AtomicU64 = AtomicU64::new(0);
static BASE: AtomicU64 = AtomicU64::new(0);
static PEAK: AtomicU64 = AtomicU64::new(0);
static BYTES: AtomicU64 = AtomicU64::new(0);
static CALLS: AtomicU64 = AtomicU64::new(0);
static REALLOCS: AtomicU64 = AtomicU64::new(0);
static ENABLED: AtomicBool = AtomicBool::new(false);
static SMALL: AtomicU64 = AtomicU64::new(0);

/// Records a successful requested-byte allocation after the system allocator succeeds.
/// Counters allocate no memory; atomic operations make concurrent allocator calls safe.
fn added(size: usize, enabled: bool) {
    let live = LIVE.fetch_add(size as u64, Ordering::Relaxed) + size as u64;
    if enabled {
        PEAK.fetch_max(live, Ordering::Relaxed);
        BYTES.fetch_add(size as u64, Ordering::Relaxed);
        CALLS.fetch_add(1, Ordering::Relaxed);
        if size <= 64 {
            SMALL.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// Delegates every layout and pointer to System without changing allocation semantics.
///
/// # Safety
/// Each method forwards the caller's GlobalAlloc contract unchanged. Only successful
/// allocations alter live bytes; failed reallocations retain the old allocation accounting.
unsafe impl GlobalAlloc for CountingAllocator {
    /// Allocates through System and counts only a non-null result.
    ///
    /// # Safety
    /// The caller provides the valid nonzero layout required by GlobalAlloc.
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let enabled = ENABLED.load(Ordering::Relaxed);
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            added(layout.size(), enabled);
        }
        pointer
    }
    /// Allocates zeroed memory through System while preserving the exact caller layout.
    ///
    /// # Safety
    /// The caller provides the valid nonzero layout required by GlobalAlloc.
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let enabled = ENABLED.load(Ordering::Relaxed);
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            added(layout.size(), enabled);
        }
        pointer
    }
    /// Releases the exact System allocation and subtracts its requested size.
    ///
    /// # Safety
    /// The caller supplies a live pointer with its original compatible layout.
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) };
        LIVE.fetch_sub(layout.size() as u64, Ordering::Relaxed);
    }
    /// Resizes through System and counts the full new request only on success.
    ///
    /// # Safety
    /// Pointer/layout/new size satisfy System's realloc contract. A null return
    /// retains the original pointer and accounting, matching the allocator contract.
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let enabled = ENABLED.load(Ordering::Relaxed);
        let next = unsafe { System.realloc(pointer, layout, new_size) };
        if !next.is_null() {
            let live = if new_size >= layout.size() {
                let delta = (new_size - layout.size()) as u64;
                LIVE.fetch_add(delta, Ordering::Relaxed) + delta
            } else {
                let delta = (layout.size() - new_size) as u64;
                LIVE.fetch_sub(delta, Ordering::Relaxed) - delta
            };
            if enabled {
                PEAK.fetch_max(live, Ordering::Relaxed);
                BYTES.fetch_add(new_size as u64, Ordering::Relaxed);
                CALLS.fetch_add(1, Ordering::Relaxed);
                REALLOCS.fetch_add(1, Ordering::Relaxed);
                if new_size <= 64 {
                    SMALL.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
        next
    }
}

/// Starts one quiescent serialized family window after source/setup, preserving live bytes.
/// The caller ensures no unrelated allocator work crosses begin/end boundaries.
pub fn begin() {
    ENABLED.store(false, Ordering::Relaxed);
    let base = LIVE.load(Ordering::Relaxed);
    BASE.store(base, Ordering::Relaxed);
    PEAK.store(base, Ordering::Relaxed);
    for counter in [&BYTES, &CALLS, &REALLOCS, &SMALL] {
        counter.store(0, Ordering::Relaxed);
    }
    ENABLED.store(true, Ordering::Relaxed);
}

/// Stops the allocation window before JSON or output hashing can affect its counters.
/// Bytes represent requested capacity, not malloc usable size, RSS or stack memory.
pub fn end() -> Option<Value> {
    ENABLED.store(false, Ordering::Relaxed);
    let base = BASE.load(Ordering::Relaxed);
    let live = LIVE.load(Ordering::Relaxed);
    let peak = PEAK.load(Ordering::Relaxed);
    let bytes = BYTES.load(Ordering::Relaxed);
    let calls = CALLS.load(Ordering::Relaxed);
    let reallocs = REALLOCS.load(Ordering::Relaxed);
    let small = SMALL.load(Ordering::Relaxed);
    Some(
        json!({"baseline_live":base,"end_live":live,"peak_live":peak,"peak_increment":peak.saturating_sub(base),"cumulative_requested_bytes":bytes,"successful_calls_including_realloc":calls,"realloc_calls":reallocs,"requests_at_most_64_bytes":small}),
    )
}
