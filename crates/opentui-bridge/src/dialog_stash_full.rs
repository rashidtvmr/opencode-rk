#![forbid(unsafe_code)]
//! Stash pick list (mirrors
//! `packages/tui/src/component/dialog-stash.tsx:29`
//! `DialogStash` most-recent-first entry picks + `onSelect` restore).
//! Single-entry delete confirm gate lives in the TS layer (`toDelete`).

/// Max chars per stashed prompt.
pub const MAX_ITEM: usize = 256;
/// Max entries retained.
pub const MAX_ITEMS: usize = 32;

/// Cursor list of stashed prompt entries.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StashDialog {
    pub items: Vec<String>,
    pub cursor: usize,
}

impl StashDialog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append entry; false on blank input or when at cap.
    pub fn push(&mut self, input: &str) -> bool {
        if input.trim().is_empty() || self.items.len() >= MAX_ITEMS {
            return false;
        }
        self.items.push(input.chars().take(MAX_ITEM).collect());
        true
    }

    /// Move cursor by delta, clamped to bounds; no-op when empty.
    pub fn move_cursor(&mut self, delta: isize) {
        if self.items.is_empty() {
            self.cursor = 0;
            return;
        }
        let next = self.cursor as isize + delta;
        self.cursor = next.clamp(0, self.items.len() as isize - 1) as usize;
    }

    /// Entry under cursor, if any.
    pub fn selected(&self) -> Option<&str> {
        self.items.get(self.cursor).map(String::as_str)
    }

    /// Owned copy of the entry under cursor, if any.
    pub fn restore(&self) -> Option<String> {
        self.selected().map(str::to_owned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_ok_selected() {
        let mut d = StashDialog::new();
        assert!(d.push("hello"));
        assert_eq!(d.selected(), Some("hello"));
    }

    #[test]
    fn push_blank_rejected() {
        let mut d = StashDialog::new();
        assert!(!d.push("   "));
        assert!(d.items.is_empty());
        assert_eq!(d.selected(), None);
    }

    #[test]
    fn push_truncates_item() {
        let mut d = StashDialog::new();
        assert!(d.push(&"x".repeat(300)));
        assert_eq!(d.items[0].chars().count(), MAX_ITEM);
    }

    #[test]
    fn push_at_cap_rejected() {
        let mut d = StashDialog::new();
        for i in 0..MAX_ITEMS {
            assert!(d.push(&format!("e{i}")));
        }
        assert!(!d.push("extra"));
        assert_eq!(d.items.len(), MAX_ITEMS);
    }

    #[test]
    fn cursor_clamps_bounds() {
        let mut d = StashDialog::new();
        d.move_cursor(1);
        assert_eq!(d.cursor, 0);
        d.push("a");
        d.push("b");
        d.move_cursor(99);
        assert_eq!(d.cursor, 1);
        d.move_cursor(-99);
        assert_eq!(d.cursor, 0);
        assert_eq!(d.selected(), Some("a"));
    }

    #[test]
    fn restore_clones_selected() {
        let mut d = StashDialog::new();
        assert_eq!(d.restore(), None);
        d.push("a");
        d.push("b");
        d.move_cursor(1);
        assert_eq!(d.restore(), Some("b".to_owned()));
    }
}
