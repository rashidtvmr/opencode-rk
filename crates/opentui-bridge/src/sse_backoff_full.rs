#![forbid(unsafe_code)]
//! SSE backoff 1s-30s + 16ms batch tick (TS truth `sdk.tsx:7-139`).

/// Initial reconnect delay.
pub const INIT_MS: u32 = 1000;
/// Max reconnect delay.
pub const MAX_MS: u32 = 30000;
/// Batch flush cadence.
pub const BATCH_MS: u32 = 16;

/// Double `cur`, saturating, clamped to 1s..30s.
#[must_use]
pub fn next_ms(cur: u32) -> u32 {
    cur.saturating_mul(2).clamp(INIT_MS, MAX_MS)
}

/// True when `elapsed_ms` reached one 16ms batch tick.
#[must_use]
pub fn is_batch_tick(elapsed_ms: u32) -> bool {
    elapsed_ms >= BATCH_MS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consts_match_ts() {
        assert_eq!((INIT_MS, MAX_MS, BATCH_MS), (1000, 30000, 16));
    }

    #[test]
    fn doubles() {
        assert_eq!(next_ms(1000), 2000);
        assert_eq!(next_ms(4000), 8000);
    }

    #[test]
    fn clamps_and_saturates() {
        assert_eq!(next_ms(20000), 30000);
        assert_eq!(next_ms(u32::MAX), 30000);
        assert_eq!(next_ms(0), 1000);
    }

    #[test]
    fn batch_tick_boundary() {
        assert!(!is_batch_tick(15));
        assert!(is_batch_tick(16));
        assert!(is_batch_tick(100));
    }

    #[test]
    fn chain_reaches_max() {
        let mut ms = INIT_MS;
        for _ in 0..8 {
            ms = next_ms(ms);
        }
        assert_eq!(ms, MAX_MS);
    }
}
