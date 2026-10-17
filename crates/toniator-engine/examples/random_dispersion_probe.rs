//! Opt-in release random/dispersion measurements; production APIs retain all authority.
#[path = "p3_support/mod.rs"]
mod p3_support;

mod allocation {
    /// Compiles away in authoritative timings; this binary uses the ordinary system allocator.
    pub fn begin() {}
    /// Reports that the authoritative binary has no allocation instrumentation.
    pub fn end() -> Option<serde_json::Value> {
        None
    }
}

/// Runs the explicitly selected bounded probe and preserves failures as process errors.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    p3_support::run()
}
