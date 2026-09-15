//! Ops health-check slice: pure O(1) liveness predicate.
//!
//! Pure function over caller-owned counters. No I/O, no threads, no
//! allocation, no retained state, no clock, no env. Deterministic: same
//! inputs => same verdict. Diagnostics carry counters only (bool + u64),
//! never paths, bodies, or secret bytes.
#![forbid(unsafe_code)]

/// Liveness verdict. Caller-owned; no lifetime held here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Health {
    pub ok: bool,
    pub checked: u64,
}

/// Healthy iff `failures == 0`. `checked` passes through unchanged.
#[must_use]
pub fn check_health(failures: u64, checked: u64) -> Health {
    Health {
        ok: failures == 0,
        checked,
    }
}

/// Predicate over a prior verdict. Pure read, no mutation.
#[must_use]
pub fn is_healthy(h: &Health) -> bool {
    h.ok
}
