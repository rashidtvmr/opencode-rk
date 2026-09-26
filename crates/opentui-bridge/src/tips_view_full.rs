#![forbid(unsafe_code)]
//! Rotating home tips (tips-view.tsx/tips.tsx). `ponytail:` Vec + idx.

/// Max tips held.
pub const MAX_TIPS: usize = 16;
/// Max chars per tip.
pub const MAX_TIP_LEN: usize = 256;

/// Rotating tip list; `next` cycles, `push` drops oldest at cap.
#[derive(Debug, Default, Clone)]
pub struct TipsView {
    tips: Vec<String>,
    idx: usize,
}

impl TipsView {
    #[must_use]
    pub fn new() -> Self {
        Self {
            tips: Vec::new(),
            idx: 0,
        }
    }

    /// Push tip, truncated to 256 chars; drops oldest past 16.
    pub fn push(&mut self, tip: impl Into<String>) {
        let t: String = tip.into().chars().take(MAX_TIP_LEN).collect();
        if self.tips.len() >= MAX_TIPS {
            self.tips.remove(0);
            self.idx = self.idx.saturating_sub(1);
        }
        self.tips.push(t);
    }

    /// Advance to next tip, wrapping; no-op when empty.
    pub fn next(&mut self) {
        if !self.tips.is_empty() {
            self.idx = (self.idx + 1) % self.tips.len();
        }
    }

    /// Current tip, or `None` when empty.
    #[must_use]
    pub fn current(&self) -> Option<&str> {
        self.tips.get(self.idx).map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_has_no_current() {
        assert!(TipsView::new().current().is_none());
    }

    #[test]
    fn push_shows_first() {
        let mut v = TipsView::new();
        v.push("a");
        assert_eq!(v.current(), Some("a"));
    }

    #[test]
    fn truncates_to_256_chars() {
        let mut v = TipsView::new();
        v.push("x".repeat(300));
        assert_eq!(v.current().unwrap().chars().count(), 256);
    }

    #[test]
    fn cap_drops_oldest() {
        let mut v = TipsView::new();
        for i in 0..17 {
            v.push(format!("t{i}"));
        }
        assert_eq!(v.current(), Some("t1"));
    }

    #[test]
    fn next_wraps() {
        let mut v = TipsView::new();
        v.push("a");
        v.push("b");
        v.next();
        assert_eq!(v.current(), Some("b"));
        v.next();
        assert_eq!(v.current(), Some("a"));
    }
}
