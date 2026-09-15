//! Integration retry planning: bounded attempts with capped backoff.

/// Maximum attempts accepted by [`plan_retry`].
pub const MAX_ATTEMPTS: u32 = 8;

/// Upper bound for computed backoff in milliseconds.
const MAX_BACKOFF_MS: u64 = 60_000;

/// Retry planning failure.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RetryError {
    /// Caller asked for zero attempts.
    #[error("retry requires at least one attempt")]
    ZeroAttempts,
    /// Caller asked for more than [`MAX_ATTEMPTS`].
    #[error("too many attempts: max {max}, asked {asked}")]
    TooManyAttempts { max: u32, asked: u32 },
}

/// Bounded retry plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPlan {
    /// Accepted attempt count (1..=MAX_ATTEMPTS).
    pub attempts: u32,
    /// Backoff in ms: `base_ms * attempts` saturating, capped at 60_000.
    pub backoff_ms: u64,
}

/// Validate `attempts` and compute capped backoff.
///
/// Rules: 0 -> [`RetryError::ZeroAttempts`]; `> MAX_ATTEMPTS` ->
/// [`RetryError::TooManyAttempts`]; otherwise
/// `base_ms.saturating_mul(attempts)` capped at 60_000.
pub fn plan_retry(attempts: u32, base_ms: u64) -> Result<RetryPlan, RetryError> {
    if attempts == 0 {
        return Err(RetryError::ZeroAttempts);
    }
    if attempts > MAX_ATTEMPTS {
        return Err(RetryError::TooManyAttempts {
            max: MAX_ATTEMPTS,
            asked: attempts,
        });
    }
    let backoff_ms = base_ms.saturating_mul(u64::from(attempts)).min(MAX_BACKOFF_MS);
    Ok(RetryPlan {
        attempts,
        backoff_ms,
    })
}
