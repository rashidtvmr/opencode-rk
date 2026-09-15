//! Fixed-window rate limiter (EXT rate-limit slice).
//!
//! Pure counter over [`RateWindow`]: validates shape only, no clock, no I/O,
//! no side effects beyond incrementing `used` on successful [`consume`].

use thiserror::Error;

/// Fixed-window budget: `limit` total, `used` consumed so far.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateWindow {
    pub limit: u64,
    pub used: u64,
}

/// Rate-limit failures.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum RateError {
    #[error("rate limit is zero")]
    ZeroLimit,
    #[error("rate limit exceeded: limit {limit}, used {used}")]
    OverLimit { limit: u64, used: u64 },
}

/// Remaining budget (`limit - used`). Zero limit rejected first, then
/// `used > limit`.
pub fn check_window(w: &RateWindow) -> Result<u64, RateError> {
    if w.limit == 0 {
        return Err(RateError::ZeroLimit);
    }
    if w.used > w.limit {
        return Err(RateError::OverLimit {
            limit: w.limit,
            used: w.used,
        });
    }
    Ok(w.limit - w.used)
}

/// Consume one unit. Fails without mutating when empty/over (reports current
/// values) or when limit is zero.
pub fn consume(w: &mut RateWindow) -> Result<(), RateError> {
    let remaining = check_window(w)?;
    if remaining == 0 {
        return Err(RateError::OverLimit {
            limit: w.limit,
            used: w.used,
        });
    }
    w.used += 1;
    Ok(())
}
