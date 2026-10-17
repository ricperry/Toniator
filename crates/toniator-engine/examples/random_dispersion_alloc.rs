//! Diagnostic-only requested-byte allocator measurements; never used for speed claims.
#[path = "p3_support/allocation.rs"]
mod allocation;
#[path = "p3_support/mod.rs"]
mod p3_support;
#[global_allocator]
static ALLOCATOR: allocation::CountingAllocator = allocation::CountingAllocator;

/// Runs the allocation diagnostic with the same current family/output APIs as ordinary timing.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    p3_support::run()
}
