#![forbid(unsafe_code)]
//! Ordered index-range selection (companion to `selection.rs`).
//!
//! TS truth `packages/tui/src/util/selection.ts:1-79` has no range type;
//! `selection.rs` covers copy semantics. This is the pure `[start, end)`
//! range primitive for caret/anchor spans. Boundary: indices only, no text.

/// Half-open `[start, end)` selection with `start <= end` invariant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    /// Inclusive anchor.
    pub start: usize,
    /// Exclusive edge (`len = end - start`).
    pub end: usize,
}

impl Selection {
    /// Ordered ctor: swaps so `start <= end`.
    #[must_use]
    pub const fn new(a: usize, b: usize) -> Self {
        Self {
            start: if a <= b { a } else { b },
            end: if a <= b { b } else { a },
        }
    }

    /// Covered length (`end - start`).
    #[must_use]
    pub const fn len(&self) -> usize {
        self.end - self.start
    }

    /// True when `start <= i < end`.
    #[must_use]
    pub const fn contains(&self, i: usize) -> bool {
        i >= self.start && i < self.end
    }

    /// Full span `0..total`.
    #[must_use]
    pub const fn select_all(total: usize) -> Self {
        Self {
            start: 0,
            end: total,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_orders_args() {
        assert_eq!(Selection::new(2, 7), Selection { start: 2, end: 7 });
        assert_eq!(Selection::new(7, 2), Selection { start: 2, end: 7 });
        assert_eq!(Selection::new(4, 4), Selection { start: 4, end: 4 });
    }

    #[test]
    fn len_is_span() {
        assert_eq!(Selection::new(2, 7).len(), 5);
        assert_eq!(Selection::new(4, 4).len(), 0);
    }

    #[test]
    fn contains_is_half_open() {
        let s = Selection::new(2, 5);
        assert!(!s.contains(1));
        assert!(s.contains(2));
        assert!(s.contains(4));
        assert!(!s.contains(5));
    }

    #[test]
    fn select_all_spans_total() {
        let s = Selection::select_all(9);
        assert_eq!((s.start, s.end, s.len()), (0, 9, 9));
        assert!(s.contains(8));
        assert!(!s.contains(9));
    }

    #[test]
    fn select_all_zero_is_empty() {
        let s = Selection::select_all(0);
        assert_eq!(s.len(), 0);
        assert!(!s.contains(0));
    }
}
