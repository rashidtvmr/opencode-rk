#![forbid(unsafe_code)]
//! Session list dialog (`dialog-session-list.tsx:45`); ordered ids, wrapping cursor.

/// Max session ids held (fail-closed bound).
pub const MAX_IDS: usize = 64;
/// Max chars per session id.
pub const MAX_ID: usize = 128;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionListDialog {
    ids: Vec<String>,
    cursor: usize,
}

impl SessionListDialog {
    #[must_use]
    pub fn new() -> Self {
        Self {
            ids: Vec::new(),
            cursor: 0,
        }
    }

    /// Append id; false when empty, too long, or full.
    pub fn push(&mut self, id: &str) -> bool {
        if id.is_empty() || id.chars().count() > MAX_ID || self.ids.len() >= MAX_IDS {
            return false;
        }
        self.ids.push(id.to_string());
        true
    }

    /// Move cursor by delta, wrapping; no-op when empty.
    pub fn move_cursor(&mut self, delta: isize) {
        let n = self.ids.len();
        if n == 0 {
            return;
        }
        let n = n as isize;
        self.cursor = (self.cursor as isize + delta).rem_euclid(n) as usize;
    }

    /// Selected id, if any.
    #[must_use]
    pub fn selected(&self) -> Option<&str> {
        self.ids.get(self.cursor).map(String::as_str)
    }

    /// First 8 chars of selected id, or empty when none.
    #[must_use]
    pub fn selected_short(&self) -> String {
        self.selected()
            .map(|s| s.chars().take(8).collect())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dlg() -> SessionListDialog {
        let mut d = SessionListDialog::new();
        d.push("abcdefgh1234");
        d.push("second-id");
        d
    }

    #[test]
    fn select_and_short() {
        let d = dlg();
        assert_eq!(d.selected(), Some("abcdefgh1234"));
        assert_eq!(d.selected_short(), "abcdefgh");
        let mut s = SessionListDialog::new();
        s.push("abc");
        assert_eq!(s.selected_short(), "abc");
    }

    #[test]
    fn push_rejects_bad() {
        let mut d = SessionListDialog::new();
        assert!(!d.push(""));
        assert!(!d.push(&"x".repeat(MAX_ID + 1)));
        assert!(d.push(&"x".repeat(MAX_ID)));
    }

    #[test]
    fn push_caps_at_64() {
        let mut d = SessionListDialog::new();
        for i in 0..MAX_IDS {
            assert!(d.push(&format!("ses-{i}")));
        }
        assert!(!d.push("overflow"));
    }

    #[test]
    fn cursor_wraps_both_ways() {
        let mut d = dlg();
        d.move_cursor(1);
        assert_eq!(d.selected(), Some("second-id"));
        d.move_cursor(1);
        assert_eq!(d.selected(), Some("abcdefgh1234"));
        d.move_cursor(-1);
        assert_eq!(d.selected(), Some("second-id"));
        d.move_cursor(7);
        assert_eq!(d.selected(), Some("abcdefgh1234"));
    }

    #[test]
    fn cursor_empty_noop() {
        let mut d = SessionListDialog::new();
        d.move_cursor(1);
        d.move_cursor(-5);
        assert_eq!(d.selected(), None);
        assert_eq!(d.selected_short(), "");
    }
}
