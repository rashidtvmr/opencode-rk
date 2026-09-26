#![forbid(unsafe_code)]
//! Full loop driver: [`PollTicker`] cadence + rendered-frame counter.
//!
//! `ponytail:` fixed cadence via PollTicker; add jitter/backoff when needed.

use crate::poll_ticker::PollTicker;

/// Ticker-gated frame counter; `frames` bumps only when ticker fires.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoopFull {
    pub ticker: PollTicker,
    pub frames: u64,
}

impl LoopFull {
    /// New driver; `every` cadence delegates to [`PollTicker::new`].
    #[must_use]
    pub fn new(every: u32) -> Self {
        Self {
            ticker: PollTicker::new(every),
            frames: 0,
        }
    }

    /// Bump ticker; on fire `frames` wrapping+1. Returns fire flag.
    pub fn tick(&mut self) -> bool {
        let fired = self.ticker.bump();
        if fired {
            self.frames = self.frames.wrapping_add(1);
        }
        fired
    }

    /// Rendered frames so far.
    #[must_use]
    pub fn frames(&self) -> u64 {
        self.frames
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_defaults_zero_frames() {
        let l = LoopFull::new(20);
        assert_eq!(l.frames(), 0);
        assert_eq!(l.ticker.every, 20);
    }

    #[test]
    fn fire_increments_frames() {
        let mut l = LoopFull::new(2);
        assert!(!l.tick());
        assert_eq!(l.frames(), 0);
        assert!(l.tick());
        assert_eq!(l.frames(), 1);
    }

    #[test]
    fn zero_cadence_falls_back() {
        let l = LoopFull::new(0);
        assert_eq!(l.ticker.every, 20);
    }

    #[test]
    fn frames_wrap_without_panic() {
        let mut l = LoopFull::new(1);
        l.frames = u64::MAX;
        assert!(l.tick());
        assert_eq!(l.frames(), 0);
    }

    #[test]
    fn many_cycles_count_fires_only() {
        let mut l = LoopFull::new(3);
        for _ in 0..9 {
            l.tick();
        }
        assert_eq!(l.frames(), 3);
    }
}
