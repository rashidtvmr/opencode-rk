#![forbid(unsafe_code)]
//! `PulseView`: tsx-level alpha wrapper over `crate::bg_pulse`.
//! Source: `bg-pulse.tsx:77-87` fps pin mount/cleanup; curve `crate::bg_pulse`.
//! ponytail: scalar alpha + ascii bar only; upgrade: per-cell colors when needed.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PulseView {
    alpha: f32,
}

impl PulseView {
    /// Clamp `alpha` into `0..=1` (NaN -> 0.0).
    #[must_use]
    pub fn new(alpha: f32) -> Self {
        let v = if alpha.is_nan() {
            0.0
        } else {
            alpha.clamp(0.0, 1.0)
        };
        Self { alpha: v }
    }

    /// Current alpha.
    #[must_use]
    pub fn alpha_of(&self) -> f32 {
        self.alpha
    }

    /// `#` bar of `width` (capped at 64) scaled by `alpha`.
    #[must_use]
    pub fn bar(&self, width: usize) -> String {
        let w = width.min(64);
        let n = (self.alpha * w as f32).round() as usize;
        "#".repeat(n.min(w))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamps_alpha() {
        assert_eq!(PulseView::new(-0.5).alpha_of(), 0.0);
        assert_eq!(PulseView::new(1.5).alpha_of(), 1.0);
        assert_eq!(PulseView::new(f32::NAN).alpha_of(), 0.0);
    }

    #[test]
    fn bar_scales_with_alpha() {
        assert_eq!(PulseView::new(0.0).bar(8), "");
        assert_eq!(PulseView::new(1.0).bar(8), "########");
        assert_eq!(PulseView::new(0.5).bar(8), "####");
    }

    #[test]
    fn bar_caps_at_64() {
        assert_eq!(PulseView::new(1.0).bar(100).len(), 64);
        assert_eq!(PulseView::new(1.0).bar(64).len(), 64);
    }
}
