#![forbid(unsafe_code)]
//! Scroll step accumulator (companion to `scroll_accel.rs`).
//!
//! `scroll_accel.rs` mirrors the `CustomSpeedScroll` selector boundary;
//! this module tracks per-gesture wheel acceleration: repeated
//! same-direction deltas within a short window grow the emitted step,
//! pauses and direction changes reset it.

/// Max rows emitted per wheel tick.
pub const MAX_STEP: u32 = 10;
/// Same-direction window: repeats within this gap accelerate (ms).
pub const ACCEL_WINDOW_MS: u64 = 120;

/// Direction-aware scroll-step accumulator.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ScrollStep {
    velocity: f32,
    last_ms: u64,
}

impl ScrollStep {
    /// Fresh accumulator (step 1, no history).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            velocity: 0.0,
            last_ms: 0,
        }
    }

    /// Push a wheel delta at `now_ms`; returns clamped step (1..=10).
    ///
    /// Same sign and gap <= 120ms doubles step (cap 10), else step is 1.
    /// Always records direction in `velocity` and time in `last_ms`.
    pub fn push(&mut self, delta: i32, now_ms: u64) -> u32 {
        let step = if self.velocity != 0.0
            && self.velocity.signum() == (delta as f32).signum()
            && now_ms.wrapping_sub(self.last_ms) <= ACCEL_WINDOW_MS
        {
            ((self.velocity.abs() as u32).max(1) * 2).min(MAX_STEP)
        } else {
            1
        };
        let sign = if delta < 0 { -1.0 } else { 1.0 };
        self.velocity = sign * step as f32;
        self.last_ms = now_ms;
        step
    }

    /// Clear accumulated velocity/history.
    pub fn reset(&mut self) {
        self.velocity = 0.0;
        self.last_ms = 0;
    }

    /// Rows for `delta` from current velocity: `|delta|` capped 10, min 1.
    #[must_use]
    pub fn step_for(&self, delta: i32) -> u32 {
        (delta.unsigned_abs().min(MAX_STEP)).max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_step_is_one() {
        let mut s = ScrollStep::new();
        assert_eq!(s.push(1, 1000), 1);
    }

    #[test]
    fn fast_repeat_grows() {
        let mut s = ScrollStep::new();
        assert_eq!(s.push(3, 1000), 1);
        assert_eq!(s.push(3, 1050), 2);
        assert_eq!(s.push(3, 1100), 4);
    }

    #[test]
    fn slow_gap_resets() {
        let mut s = ScrollStep::new();
        assert_eq!(s.push(3, 1000), 1);
        assert_eq!(s.push(3, 1200), 1);
    }

    #[test]
    fn step_caps_at_ten() {
        let s = ScrollStep::new();
        assert_eq!(s.step_for(120), MAX_STEP);
        assert_eq!(s.step_for(-999), MAX_STEP);
        let mut r = ScrollStep::new();
        assert_eq!(r.push(9, 1000), 1);
        assert_eq!(r.push(9, 1050), 2);
        assert_eq!(r.push(9, 1100), 4);
        assert_eq!(r.push(9, 1150), 8);
        assert_eq!(r.push(9, 1200), MAX_STEP);
    }

    #[test]
    fn direction_change_resets() {
        let mut s = ScrollStep::new();
        assert_eq!(s.push(4, 1000), 1);
        assert_eq!(s.push(-4, 1050), 1);
    }

    #[test]
    fn reset_clears_accel() {
        let mut s = ScrollStep::new();
        s.push(3, 1000);
        s.reset();
        assert_eq!(s.push(3, 1050), 1);
        assert_eq!(s.step_for(0), 1);
    }
}
