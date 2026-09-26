#![forbid(unsafe_code)]
//! Pending attention badge (companion to `attention.rs`).
//!
//! TS truth `packages/tui/src/attention.ts` has no counter; new state.
//! Invariant: `needed == (count > 0)`; `clear` resets both.

/// Pending attention requests not yet acknowledged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Attention {
    pub needed: bool,
    pub count: u32,
}

impl Attention {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark attention needed; bump pending count (saturating).
    pub fn request(&mut self) {
        self.count = self.count.saturating_add(1);
        self.needed = true;
    }

    /// Acknowledge all pending: clear flag and reset count.
    pub fn clear(&mut self) {
        self.needed = false;
        self.count = 0;
    }

    #[must_use]
    pub fn is_needed(&self) -> bool {
        self.needed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_needs_nothing() {
        let a = Attention::new();
        assert!(!a.is_needed());
        assert_eq!(a.count, 0);
    }

    #[test]
    fn request_marks_needed_and_bumps() {
        let mut a = Attention::new();
        a.request();
        assert!(a.is_needed());
        assert_eq!(a.count, 1);
        a.request();
        assert_eq!(a.count, 2);
    }

    #[test]
    fn clear_resets_flag_and_count() {
        let mut a = Attention::new();
        a.request();
        a.request();
        a.clear();
        assert!(!a.is_needed());
        assert_eq!(a.count, 0);
    }

    #[test]
    fn count_saturates_at_max() {
        let mut a = Attention {
            needed: true,
            count: u32::MAX,
        };
        a.request();
        assert_eq!(a.count, u32::MAX);
        assert!(a.is_needed());
    }
}
