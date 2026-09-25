#![forbid(unsafe_code)]
//! Run footer popup menu (mirrors `footer.menu.tsx` selection state).
//!
//! TS truth holds selected/offset over a popup list; this keeps the
//! owned slice: capped items, wrapping cursor, open/close gate.

/// Max chars kept per label.
pub const MAX_LABEL_LEN: usize = 64;
/// Max chars kept per action.
pub const MAX_ACTION_LEN: usize = 64;
/// Max items kept on open.
pub const MAX_ITEMS: usize = 16;

fn take(s: &str, cap: usize) -> String {
    s.chars().take(cap).collect()
}

/// One selectable footer menu row.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MenuItem {
    pub label: String,
    pub action: String,
}

impl MenuItem {
    pub fn new(label: &str, action: &str) -> Self {
        Self {
            label: take(label, MAX_LABEL_LEN),
            action: take(action, MAX_ACTION_LEN),
        }
    }
}

/// Popup menu state: capped item list plus wrapping cursor.
#[derive(Debug, Clone, Default)]
pub struct FooterMenu {
    items: Vec<MenuItem>,
    cursor: usize,
    open: bool,
}

impl FooterMenu {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Open with items (capped at 16). False + stays closed when empty.
    pub fn open(&mut self, items: Vec<MenuItem>) -> bool {
        if items.is_empty() {
            return false;
        }
        self.items = items.into_iter().take(MAX_ITEMS).collect();
        self.cursor = 0;
        self.open = true;
        true
    }

    /// Move cursor with wraparound. No-op when closed or empty.
    pub fn move_cursor(&mut self, delta: isize) {
        if !self.open || self.items.is_empty() {
            return;
        }
        let n = self.items.len() as isize;
        self.cursor = (self.cursor as isize + delta).rem_euclid(n) as usize;
    }

    /// Currently highlighted item, or None when closed/empty.
    pub fn chosen(&self) -> Option<&MenuItem> {
        if !self.open {
            return None;
        }
        self.items.get(self.cursor)
    }

    /// Close and clear selection state.
    pub fn close(&mut self) {
        self.open = false;
        self.cursor = 0;
        self.items.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items(n: usize) -> Vec<MenuItem> {
        (0..n)
            .map(|i| MenuItem::new(&format!("label-{i}"), &format!("act-{i}")))
            .collect()
    }

    #[test]
    fn open_caps_at_16() {
        let mut m = FooterMenu::new();
        assert!(m.open(items(20)));
        assert_eq!(m.len(), MAX_ITEMS);
        assert!(m.is_open());
    }

    #[test]
    fn cursor_wraps_forward_and_back() {
        let mut m = FooterMenu::new();
        assert!(m.open(items(3)));
        m.move_cursor(3);
        assert_eq!(m.cursor(), 0);
        m.move_cursor(-1);
        assert_eq!(m.cursor(), 2);
        m.move_cursor(5);
        assert_eq!(m.cursor(), 1);
    }

    #[test]
    fn chosen_none_when_closed() {
        let m = FooterMenu::new();
        assert_eq!(m.chosen(), None);
    }

    #[test]
    fn close_clears() {
        let mut m = FooterMenu::new();
        assert!(m.open(items(2)));
        m.move_cursor(1);
        m.close();
        assert!(!m.is_open());
        assert_eq!(m.chosen(), None);
        assert_eq!(m.len(), 0);
        assert_eq!(m.cursor(), 0);
    }

    #[test]
    fn empty_open_false() {
        let mut m = FooterMenu::new();
        assert!(!m.open(vec![]));
        assert!(!m.is_open());
        assert_eq!(m.chosen(), None);
    }

    #[test]
    fn caps_truncate_label_and_action() {
        let item = MenuItem::new(&"l".repeat(100), &"a".repeat(100));
        assert_eq!(item.label.chars().count(), MAX_LABEL_LEN);
        assert_eq!(item.action.chars().count(), MAX_ACTION_LEN);
    }
}
