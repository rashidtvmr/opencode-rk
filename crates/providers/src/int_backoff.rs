//! Pure backoff math: linear capped delay, std only.

/// Upper bound for computed backoff in milliseconds.
pub const MAX_BACKOFF_MS: u64 = 60_000;

/// Linear capped backoff: `0` for attempt `0` or `base_ms == 0`,
/// else `base_ms.saturating_mul(attempt)` capped at [`MAX_BACKOFF_MS`].
pub fn backoff_ms(attempt: u32, base_ms: u64) -> u64 {
    if attempt == 0 || base_ms == 0 {
        return 0;
    }
    base_ms
        .saturating_mul(u64::from(attempt))
        .min(MAX_BACKOFF_MS)
}
