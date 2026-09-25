#![forbid(unsafe_code)]
//! Full bg pulse stepper (mirrors `bg-pulse-render.ts` pulse cycle).
//! Sibling `bg_pulse.rs` covers time-based envelope; this covers stepped bar.
//!
//! ponytail: fixed 8-step bar only. Upgrade: wire `pulse_alpha` envelope.

/// Stepped background pulse cursor (8 steps).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BgPulse {
    pub step: u8,
}

impl BgPulse {
    #[must_use]
    pub const fn new() -> Self {
        Self { step: 0 }
    }

    /// Advance one step, wrapping 0..8.
    pub fn tick(&mut self) {
        self.step = (self.step + 1) % 8;
    }

    /// Current level.
    #[must_use]
    pub fn level(&self) -> u8 {
        self.step % 8
    }

    /// `"#"` bar scaled by level over `width`, capped at 64.
    #[must_use]
    pub fn bar(&self, width: usize) -> String {
        let w = width.min(64);
        let n = (self.level() as usize * w / 8).min(64).min(w);
        "#".repeat(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_zero() {
        assert_eq!(BgPulse::new().level(), 0);
    }

    #[test]
    fn tick_wraps_eight() {
        let mut p = BgPulse::new();
        for _ in 0..7 {
            p.tick();
        }
        assert_eq!(p.level(), 7);
        p.tick();
        assert_eq!(p.step, 0);
    }

    #[test]
    fn bar_scales_with_level() {
        let p = BgPulse { step: 4 };
        assert_eq!(p.bar(8), "####");
        assert_eq!(BgPulse::new().bar(8), "");
    }

    #[test]
    fn bar_caps_64() {
        let p = BgPulse { step: 7 };
        assert!(p.bar(usize::MAX).len() <= 64);
        assert_eq!(p.bar(64).len(), 56);
    }
}
