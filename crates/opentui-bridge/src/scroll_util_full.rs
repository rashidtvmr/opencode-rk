#![forbid(unsafe_code)]
//! Scroll offset pure helpers (companion to `scroll_step_full.rs`).
//!
//! TS truth `packages/tui/src/util/scroll.ts:8-26` covers only the
//! acceleration selector; these are stateless extracts of the
//! `ScrollFull::step` clamp (`scroll_step_full.rs:46-50`).
//! Boundary: pure fns, no struct/state; acceleration lives in `scroll_step.rs`.

/// Clamp signed `off` into `0..=total.saturating_sub(view)`.
#[must_use]
pub fn clamp_offset(off: isize, total: usize, view: usize) -> usize {
    let max = total.saturating_sub(view) as isize;
    off.clamp(0, max) as usize
}

/// Page step: `view - 1`, floored at 1 (view 0/1 -> 1).
#[must_use]
pub fn page_step(view: usize) -> usize {
    view.saturating_sub(1).max(1)
}

/// True when `off` shows the last row (`off + view >= total`).
#[must_use]
pub fn at_bottom(off: usize, total: usize, view: usize) -> bool {
    off.saturating_add(view) >= total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_pins_both_edges() {
        assert_eq!(clamp_offset(-5, 10, 4), 0);
        assert_eq!(clamp_offset(99, 10, 4), 6);
        assert_eq!(clamp_offset(3, 10, 4), 3);
    }

    #[test]
    fn clamp_empty_or_oversize_view_is_zero() {
        assert_eq!(clamp_offset(5, 0, 4), 0);
        assert_eq!(clamp_offset(5, 3, 10), 0);
        assert_eq!(clamp_offset(-1, 3, 10), 0);
    }

    #[test]
    fn page_step_floors_at_one() {
        assert_eq!(page_step(0), 1);
        assert_eq!(page_step(1), 1);
        assert_eq!(page_step(2), 1);
        assert_eq!(page_step(10), 9);
    }

    #[test]
    fn bottom_detects_last_page() {
        assert!(at_bottom(6, 10, 4));
        assert!(!at_bottom(5, 10, 4));
        assert!(at_bottom(0, 3, 10));
        assert!(at_bottom(0, 0, 0));
    }
}
