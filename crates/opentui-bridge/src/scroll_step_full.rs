#![forbid(unsafe_code)]
//! Full scroll offset over a known total (companion to `scroll_step.rs`).
//!
//! Claim: absolute offset tracker. set_total grows/shrinks total and clamps
//! offset, step moves by signed delta clamped to 0..=total.saturating_sub(view).
//! Evidence: crates/opentui-bridge/src/scroll_step.rs:16-21 (step accumulator,
//! no absolute offset/total concept).
//! Boundary: absolute offset only; per-tick acceleration lives in ScrollStep.

/// Absolute scroll position within `total` rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ScrollFull {
    offset: usize,
    total: usize,
}

impl ScrollFull {
    /// Fresh position at top of an empty list.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            offset: 0,
            total: 0,
        }
    }

    /// Current top-row offset.
    #[must_use]
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// Total row count (for empty view max offset is `total`).
    #[must_use]
    pub const fn total(&self) -> usize {
        self.total
    }

    /// Replace total; clamps offset into `0..=total`.
    pub fn set_total(&mut self, total: usize) {
        self.total = total;
        self.offset = self.offset.min(total);
    }

    /// Move by signed `delta`, clamped to `0..=total.saturating_sub(view)`.
    pub fn step(&mut self, delta: isize, view: usize) {
        let max = self.total.saturating_sub(view) as isize;
        let next = (self.offset as isize + delta).clamp(0, max);
        self.offset = next as usize;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_at_zero() {
        let s = ScrollFull::new();
        assert_eq!(s.offset(), 0);
        assert_eq!(s.total(), 0);
    }

    #[test]
    fn set_total_clamps_offset() {
        let mut s = ScrollFull::new();
        s.set_total(10);
        s.step(9, 1);
        assert_eq!(s.offset(), 9);
        s.set_total(4);
        assert_eq!(s.offset(), 4);
    }

    #[test]
    fn step_down_clamps_at_max() {
        let mut s = ScrollFull::new();
        s.set_total(10);
        s.step(99, 4);
        assert_eq!(s.offset(), 6);
    }

    #[test]
    fn step_up_clamps_at_zero() {
        let mut s = ScrollFull::new();
        s.set_total(10);
        s.step(3, 4);
        s.step(-99, 4);
        assert_eq!(s.offset(), 0);
    }

    #[test]
    fn view_larger_than_total_pins_zero() {
        let mut s = ScrollFull::new();
        s.set_total(3);
        s.step(5, 10);
        assert_eq!(s.offset(), 0);
    }

    #[test]
    fn step_moves_both_directions() {
        let mut s = ScrollFull::new();
        s.set_total(10);
        s.step(4, 4);
        assert_eq!(s.offset(), 4);
        s.step(-2, 4);
        assert_eq!(s.offset(), 2);
    }
}
