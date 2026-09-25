#![forbid(unsafe_code)]
//! Home index cursor (TS: `packages/tui/src/routes/home/session-destination.tsx:26`
//! `selected` signal + `routes/session/question.tsx:29,270-279` wrap-around move).
//! TS home `index.tsx` absent at checkout; only `session-destination.tsx` exists,
//! so this mirrors the generic list-cursor pattern (also `ui/dialog-select.tsx`).
//! Divergences: Rust stores owned `String` rows (TS holds components); cursor
//! wraps modulo len (matches question `moveTo`); empty list is a no-op.

/// Max rows in one `HomeIndex` (fail-closed bound).
pub const MAX_HOME_ITEMS: usize = 64;
/// Max chars per row.
pub const MAX_HOME_ITEM_LEN: usize = 256;

/// Bounded home list with wrapping cursor.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct HomeIndex {
    items: Vec<String>,
    cursor: usize,
}

impl HomeIndex {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
    #[must_use]
    pub fn cursor(&self) -> usize {
        self.cursor
    }
    /// Append row; `false` when empty, too long, or full.
    pub fn push(&mut self, item: &str) -> bool {
        if item.is_empty() || item.chars().count() > MAX_HOME_ITEM_LEN {
            return false;
        }
        if self.items.len() >= MAX_HOME_ITEMS {
            return false;
        }
        self.items.push(item.to_string());
        true
    }
    /// Move cursor by `delta`, wrapping (empty: no-op).
    pub fn move_cursor(&mut self, delta: isize) {
        let n = self.items.len() as isize;
        if n == 0 {
            return;
        }
        let cur = self.cursor as isize;
        self.cursor = (((cur + delta) % n + n) % n) as usize;
    }
    /// Focused row, `None` when empty.
    #[must_use]
    pub fn selected(&self) -> Option<&str> {
        self.items.get(self.cursor).map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn push_ok_and_selected() {
        let mut h = HomeIndex::new();
        assert!(h.is_empty() && h.selected().is_none());
        assert!(h.push("a") && h.push("b"));
        assert_eq!(h.len(), 2);
        assert_eq!(h.selected(), Some("a"));
    }
    #[test]
    fn push_rejects_bad_or_full() {
        let mut h = HomeIndex::new();
        assert!(!h.push(""));
        assert!(!h.push(&"x".repeat(MAX_HOME_ITEM_LEN + 1)));
        for i in 0..MAX_HOME_ITEMS {
            assert!(h.push(&format!("i{i}")));
        }
        assert!(!h.push("one-more"));
    }
    #[test]
    fn move_wraps_both_ways() {
        let mut h = HomeIndex::new();
        for s in ["a", "b", "c"] {
            assert!(h.push(s));
        }
        h.move_cursor(-1);
        assert_eq!(h.selected(), Some("c"));
        h.move_cursor(1);
        assert_eq!(h.selected(), Some("a"));
        h.move_cursor(5);
        assert_eq!(h.selected(), Some("c"));
    }
    #[test]
    fn empty_move_noop() {
        let mut h = HomeIndex::new();
        h.move_cursor(1);
        h.move_cursor(-3);
        assert_eq!(h.cursor(), 0);
        assert!(h.selected().is_none());
    }
}
