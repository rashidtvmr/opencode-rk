#![forbid(unsafe_code)]
//! Hydration phase + 100-item window (TS: `packages/tui/src/context/sync.tsx:1-666`).
//!
//! Models bootstrap -> session-sync -> hydrated, where produce emits batches
//! and reconcile merges them; visible tail truncated to [`WINDOW`] items.
//!
//! Divergence: tick latch lives in `crate::ctx_sync_full`; 3 location events
//! in `crate::sync_store`; no network here, phase/window helpers only.

/// Max hydrated items retained (produce/reconcile window).
pub const WINDOW: usize = 100;

/// Bootstrap -> session-sync -> hydrated lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SyncPhase {
    #[default]
    Bootstrap,
    Sync,
    Hydrated,
}

/// Stable name for `phase` (`"bootstrap" | "sync" | "hydrated"`).
#[must_use]
pub const fn phase_name(phase: SyncPhase) -> &'static str {
    match phase {
        SyncPhase::Bootstrap => "bootstrap",
        SyncPhase::Sync => "sync",
        SyncPhase::Hydrated => "hydrated",
    }
}

/// Clamp `n` to [`WINDOW`] (`min(n, 100)`).
#[must_use]
pub const fn window_trunc(n: usize) -> usize {
    if n > WINDOW {
        WINDOW
    } else {
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_is_100() {
        assert_eq!(WINDOW, 100);
    }

    #[test]
    fn phase_names_cover_all_variants() {
        assert_eq!(phase_name(SyncPhase::Bootstrap), "bootstrap");
        assert_eq!(phase_name(SyncPhase::Sync), "sync");
        assert_eq!(phase_name(SyncPhase::Hydrated), "hydrated");
    }

    #[test]
    fn trunc_clamps_at_window() {
        assert_eq!(window_trunc(0), 0);
        assert_eq!(window_trunc(50), 50);
        assert_eq!(window_trunc(100), 100);
        assert_eq!(window_trunc(101), 100);
        assert_eq!(window_trunc(usize::MAX), 100);
    }

    #[test]
    fn trunc_never_exceeds_window() {
        for n in [0, 1, 99, 100, 101, 1000] {
            assert!(window_trunc(n) <= WINDOW);
        }
    }
}
