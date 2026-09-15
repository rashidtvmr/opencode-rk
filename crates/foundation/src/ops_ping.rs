//! Ops ping liveness slice (OPS-008), distinct from ops_health.

/// Returns true when failures are within budget.
#[must_use]
pub fn is_alive(failures: u64, max_failures: u64) -> bool {
    failures <= max_failures
}
