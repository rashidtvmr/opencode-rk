//! Per-tool call quota (EXT-011 quota slice).
//!
//! Pure counter over [`ToolQuota`]: validates shape only, no clock, no I/O,
//! no side effects beyond incrementing `used` on successful [`consume_quota`].
//! Distinct from the `ext_rate` window: quota is per named tool, not a time
//! window.

use thiserror::Error;

/// Per-tool call budget: `limit` total calls, `used` consumed so far.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolQuota {
    pub tool: String,
    pub limit: u64,
    pub used: u64,
}

/// Quota failures.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum QuotaError {
    #[error("tool name is empty")]
    EmptyTool,
    #[error("quota limit is zero")]
    ZeroLimit,
    #[error("quota exceeded: limit {limit}, used {used}")]
    OverQuota { limit: u64, used: u64 },
}

/// Remaining budget (`limit - used`). Empty tool rejected first, then zero
/// limit, then `used > limit`.
pub fn check_quota(q: &ToolQuota) -> Result<u64, QuotaError> {
    if q.tool.is_empty() {
        return Err(QuotaError::EmptyTool);
    }
    if q.limit == 0 {
        return Err(QuotaError::ZeroLimit);
    }
    if q.used > q.limit {
        return Err(QuotaError::OverQuota {
            limit: q.limit,
            used: q.used,
        });
    }
    Ok(q.limit - q.used)
}

/// Consume one call. Fails without mutating when empty/over (reports current
/// values) or when limit is zero. Increment saturates at `u64::MAX`.
pub fn consume_quota(q: &mut ToolQuota) -> Result<(), QuotaError> {
    let remaining = check_quota(q)?;
    if remaining == 0 {
        return Err(QuotaError::OverQuota {
            limit: q.limit,
            used: q.used,
        });
    }
    q.used = q.used.saturating_add(1);
    Ok(())
}
