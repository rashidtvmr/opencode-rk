#![forbid(unsafe_code)]
//! Session timeline dialog rows (mirrors
//! `packages/tui/src/routes/session/dialog-timeline.tsx:10` `DialogTimeline`).
//!
//! String-only user-message titles; caller pushes newest last.

pub const MAX_ITEMS: usize = 32;
pub const MAX_ITEM_CHARS: usize = 256;

fn trunc(s: &str) -> String {
    s.chars().take(MAX_ITEM_CHARS).collect()
}

/// Capped timeline row list with wrapping cursor.
#[derive(Debug, Clone, Default)]
pub struct TlDlg {
    pub items: Vec<String>,
    pub cursor: usize,
}

impl TlDlg {
    /// Empty timeline.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append row, truncating to [`MAX_ITEM_CHARS`]; evicts oldest at cap.
    pub fn push(&mut self, item: &str) {
        if self.items.len() >= MAX_ITEMS {
            self.items.remove(0);
            if self.cursor > 0 {
                self.cursor -= 1;
            }
        }
        self.items.push(trunc(item));
    }

    /// Move cursor by delta, wrapping; no-op when empty.
    pub fn move_cursor(&mut self, delta: isize) {
        if self.items.is_empty() {
            return;
        }
        let n = self.items.len() as isize;
        self.cursor = (self.cursor as isize + delta).rem_euclid(n) as usize;
    }

    /// Row under cursor, or `None` when empty.
    pub fn selected(&self) -> Option<&str> {
        self.items.get(self.cursor).map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_truncates() {
        let mut d = TlDlg::new();
        d.push(&"x".repeat(300));
        assert_eq!(d.items[0].chars().count(), MAX_ITEM_CHARS);
    }

    #[test]
    fn push_evicts_oldest_at_cap() {
        let mut d = TlDlg::new();
        for i in 0..MAX_ITEMS + 1 {
            d.push(&i.to_string());
        }
        assert_eq!(d.items.len(), MAX_ITEMS);
        assert_eq!(d.items[0].as_str(), "1");
    }

    #[test]
    fn cursor_wraps() {
        let mut d = TlDlg::new();
        d.push("a");
        d.push("b");
        d.move_cursor(1);
        d.move_cursor(1);
        assert_eq!(d.selected(), Some("a"));
    }

    #[test]
    fn selected_none_when_empty() {
        assert_eq!(TlDlg::new().selected(), None);
    }
}
