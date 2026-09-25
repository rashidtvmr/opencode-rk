#![forbid(unsafe_code)]
//! Full footer string menu (mirrors `footer.menu.tsx` popup state).

/// Max items kept.
pub const MAX_ITEMS: usize = 32;
/// Max chars kept per item.
pub const MAX_ITEM_LEN: usize = 128;

fn take(s: &str) -> String {
    s.chars().take(MAX_ITEM_LEN).collect()
}

/// Popup string-menu state with wrapping cursor and open gate.
#[derive(Debug, Clone, Default)]
pub struct FooterMenuFull {
    items: Vec<String>,
    cursor: usize,
    open: bool,
}

impl FooterMenuFull {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Push truncated item. False at cap.
    pub fn add_item(&mut self, item: &str) -> bool {
        if self.items.len() >= MAX_ITEMS {
            return false;
        }
        self.items.push(take(item));
        true
    }

    /// Open when non-empty. False + stays closed when empty.
    pub fn open_menu(&mut self) -> bool {
        if self.items.is_empty() {
            return false;
        }
        self.cursor = 0;
        self.open = true;
        true
    }

    /// Close (keeps items so menu can reopen).
    pub fn close_menu(&mut self) {
        self.open = false;
        self.cursor = 0;
    }

    /// Move cursor with wraparound. No-op when closed or empty.
    pub fn move_cursor(&mut self, delta: isize) {
        if !self.open || self.items.is_empty() {
            return;
        }
        let n = self.items.len() as isize;
        self.cursor = (self.cursor as isize + delta).rem_euclid(n) as usize;
    }

    /// Highlighted item, or None when closed/empty.
    pub fn selected(&self) -> Option<&str> {
        if !self.open {
            return None;
        }
        self.items.get(self.cursor).map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filled(n: usize) -> FooterMenuFull {
        let mut m = FooterMenuFull::new();
        for i in 0..n {
            assert!(m.add_item(&format!("item-{i}")));
        }
        m
    }

    #[test]
    fn open_sets_flag() {
        let mut m = filled(2);
        assert!(m.open_menu());
        assert!(m.is_open());
        assert_eq!(m.selected(), Some("item-0"));
    }

    #[test]
    fn open_empty_stays_closed() {
        let mut m = FooterMenuFull::new();
        assert!(!m.open_menu());
        assert!(!m.is_open());
    }

    #[test]
    fn close_gates_selected() {
        let mut m = filled(2);
        assert!(m.open_menu());
        m.close_menu();
        assert!(!m.is_open());
        assert_eq!(m.selected(), None);
        assert_eq!(m.cursor(), 0);
    }

    #[test]
    fn cursor_wraps_both_ways() {
        let mut m = filled(3);
        assert!(m.open_menu());
        m.move_cursor(3);
        assert_eq!(m.cursor(), 0);
        m.move_cursor(-1);
        assert_eq!(m.cursor(), 2);
        m.move_cursor(5);
        assert_eq!(m.cursor(), 1);
        assert_eq!(m.selected(), Some("item-1"));
    }

    #[test]
    fn selected_none_when_closed() {
        let m = filled(2);
        assert_eq!(m.selected(), None);
    }

    #[test]
    fn add_item_caps_at_32_and_truncates() {
        let mut m = filled(MAX_ITEMS);
        assert!(!m.add_item("extra"));
        assert_eq!(m.len(), MAX_ITEMS);
        let mut t = FooterMenuFull::new();
        assert!(t.add_item(&"x".repeat(200)));
        assert_eq!(t.len(), 1);
        assert!(t.open_menu());
        assert_eq!(t.selected().unwrap().chars().count(), MAX_ITEM_LEN);
    }
}
