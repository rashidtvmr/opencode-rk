#![forbid(unsafe_code)]
//! Poll ticker: TS truth `tui_entry.rs:646-650` (`wrapping_add(1)`, fire on `% 20`).
//!
//! `ponytail:` fixed cadence only; add jitter/backoff when polling needs it.

/// Fires every `every` bumps; `tick` counts bumps via wrapping add.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PollTicker {
    pub tick: u32,
    pub every: u32,
}

impl PollTicker {
    /// New ticker; zero `every` falls back to 20 (TS truth default).
    #[must_use]
    pub fn new(every: u32) -> Self {
        Self {
            tick: 0,
            every: if every == 0 { 20 } else { every },
        }
    }

    /// Advance one tick; true exactly when `tick % every == 0`.
    pub fn bump(&mut self) -> bool {
        self.tick = self.tick.wrapping_add(1);
        if self.every == 0 {
            return false;
        }
        self.tick % self.every == 0
    }

    /// Change cadence; zero rejected, old value kept.
    pub fn set_every(&mut self, every: u32) -> bool {
        if every == 0 {
            return false;
        }
        self.every = every;
        true
    }

    /// Current tick count.
    #[must_use]
    pub fn tick(&self) -> u32 {
        self.tick
    }
}

impl Default for PollTicker {
    fn default() -> Self {
        Self::new(20)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fires_at_default_twenty() {
        let mut t = PollTicker::default();
        for _ in 0..19 {
            assert!(!t.bump());
        }
        assert!(t.bump());
        assert_eq!(t.tick(), 20);
    }

    #[test]
    fn custom_cadence() {
        let mut t = PollTicker::new(3);
        assert!(!t.bump());
        assert!(!t.bump());
        assert!(t.bump());
    }

    #[test]
    fn set_every_rejects_zero() {
        let mut t = PollTicker::default();
        assert!(!t.set_every(0));
        assert_eq!(t.every, 20);
        assert!(t.set_every(5));
        assert_eq!(t.every, 5);
    }

    #[test]
    fn new_zero_falls_back() {
        assert_eq!(PollTicker::new(0).every, 20);
    }

    #[test]
    fn wraps_without_panic() {
        let mut t = PollTicker::new(20);
        t.tick = u32::MAX;
        assert!(t.bump());
        assert_eq!(t.tick(), 0);
    }
}
