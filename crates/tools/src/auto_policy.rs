//! AUTO retry policy: validated max retries + backoff.
use thiserror::Error;

/// Maximum accepted retry count by [`make_policy`].
pub const MAX_RETRIES: u8 = 10;

/// Validated retry policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutoPolicy {
    /// Maximum retry attempts.
    pub max_retries: u8,
    /// Backoff between retries in milliseconds.
    pub backoff_ms: u64,
}

/// Retry policy validation failures.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AutoPolicyError {
    /// Retry count exceeds 10.
    #[error("too many retries")]
    TooManyRetries,
    /// Backoff must be non-zero.
    #[error("backoff must be non-zero")]
    ZeroBackoff,
}

/// Validate inputs into an [`AutoPolicy`].
///
/// Retry count checked first: `max_retries > 10` fails; zero backoff fails.
pub fn make_policy(max_retries: u8, backoff_ms: u64) -> Result<AutoPolicy, AutoPolicyError> {
    if max_retries > MAX_RETRIES {
        return Err(AutoPolicyError::TooManyRetries);
    }
    if backoff_ms == 0 {
        return Err(AutoPolicyError::ZeroBackoff);
    }
    Ok(AutoPolicy {
        max_retries,
        backoff_ms,
    })
}
