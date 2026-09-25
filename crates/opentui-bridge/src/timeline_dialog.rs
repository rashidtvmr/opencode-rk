#![forbid(unsafe_code)]
//! Timeline dialog state (mirrors
//! `packages/tui/src/routes/session/dialog-timeline.tsx:10` `DialogTimeline`).
//!
//! Simplified string-only dialog: TS `DialogSelectOption<string>` rows map to
//! truncated text entries with open flag, wrapping cursor, selection accessor.

/// Max entries kept by [`TimelineDialog`].
pub const MAX_ENTRIES: usize = 256;
/// Max chars per entry.
pub const MAX_ENTRY_CHARS: usize = 512;

fn trunc(s: &str) -> String {
    s.chars().take(MAX_ENTRY_CHARS).collect()
}

/// String-only timeline dialog with open flag and wrapping cursor.
#[derive(Debug, Clone, Default)]
pub struct TimelineDialog {
    pub entries: Vec<String>,
    pub cursor: usize,
    pub open: bool,
}

impl TimelineDialog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open_dialog(&mut self) {
        self.open = true;
    }

    pub fn close_dialog(&mut self) {
        self.open = false;
    }

    pub fn set_entries(&mut self, entries: Vec<String>) -> usize {
        self.entries = entries
            .into_iter()
            .take(MAX_ENTRIES)
            .map(|e| trunc(&e))
            .collect();
        self.cursor = 0;
        self.entries.len()
    }

    pub fn move_cursor(&mut self, delta: isize) {
        let len = self.entries.len();
        if len == 0 {
            return;
        }
        let next = (self.cursor as isize + delta).rem_euclid(len as isize);
        self.cursor = next as usize;
    }

    pub fn selected(&self) -> Option<&str> {
        self.entries.get(self.cursor).map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dialog_with(n: usize) -> TimelineDialog {
        let mut d = TimelineDialog::new();
        d.set_entries((0..n).map(|i| format!("m{i}")).collect());
        d
    }

    #[test]
    fn open_close_toggles_flag() {
        let mut d = TimelineDialog::new();
        assert!(!d.open);
        d.open_dialog();
        assert!(d.open);
        d.close_dialog();
        assert!(!d.open);
    }

    #[test]
    fn cursor_wraps_forward() {
        let mut d = dialog_with(2);
        d.move_cursor(2);
        assert_eq!(d.cursor, 0);
        d.move_cursor(1);
        assert_eq!(d.cursor, 1);
    }

    #[test]
    fn cursor_wraps_backward() {
        let mut d = dialog_with(2);
        d.move_cursor(-1);
        assert_eq!(d.cursor, 1);
        assert_eq!(d.selected(), Some("m1"));
    }

    #[test]
    fn selected_none_when_empty() {
        let d = TimelineDialog::new();
        assert_eq!(d.selected(), None);
    }

    #[test]
    fn set_entries_caps_count() {
        let mut d = TimelineDialog::new();
        let kept = d.set_entries((0..300).map(|i| format!("m{i}")).collect());
        assert_eq!(kept, MAX_ENTRIES);
        assert_eq!(d.entries.len(), MAX_ENTRIES);
    }

    #[test]
    fn set_entries_truncates_chars() {
        let mut d = TimelineDialog::new();
        d.set_entries(vec!["x".repeat(600)]);
        assert_eq!(d.entries[0].chars().count(), MAX_ENTRY_CHARS);
    }
}
