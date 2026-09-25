#![forbid(unsafe_code)]
//! Filtered timeline pairing [`TimelineDialog`] with a capped query.
//!
//! Mirrors `packages/tui/src/routes/session/dialog-timeline.tsx:10`
//! `DialogTimeline`: user-message rows, newest first, substring filter.

use crate::timeline_dialog::TimelineDialog;

/// Max chars kept in filter query.
pub const MAX_FILTER_CHARS: usize = 128;
/// Max chars of [`TimelineFull::status`] output.
pub const MAX_STATUS_CHARS: usize = 128;

/// Timeline dialog plus case-insensitive substring filter.
#[derive(Debug, Clone, Default)]
pub struct TimelineFull {
    pub dlg: TimelineDialog,
    pub filter: String,
}

impl TimelineFull {
    /// Empty timeline, empty filter.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set filter query, capped at [`MAX_FILTER_CHARS`] chars.
    pub fn set_filter(&mut self, q: &str) {
        self.filter = q.chars().take(MAX_FILTER_CHARS).collect();
    }

    /// Entries matching filter (case-insensitive substring); all when empty.
    pub fn filtered(&self) -> Vec<String> {
        if self.filter.is_empty() {
            return self.dlg.entries.clone();
        }
        let q = self.filter.to_lowercase();
        self.dlg
            .entries
            .iter()
            .filter(|e| e.to_lowercase().contains(&q))
            .cloned()
            .collect()
    }

    /// Count of filtered entries.
    pub fn count(&self) -> usize {
        self.filtered().len()
    }

    /// Cursor line of underlying dialog.
    pub fn cursor_line(&self) -> Option<String> {
        self.dlg.selected().map(str::to_string)
    }

    /// `"<n> entries cursor <i>"`, capped at [`MAX_STATUS_CHARS`] chars.
    pub fn status(&self) -> String {
        let s = format!("{} entries cursor {}", self.count(), self.dlg.cursor);
        s.chars().take(MAX_STATUS_CHARS).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full() -> TimelineFull {
        let mut f = TimelineFull::new();
        f.dlg.set_entries(vec!["Hello World".into(), "bye".into()]);
        f
    }

    #[test]
    fn empty_filter_returns_all() {
        assert_eq!(full().filtered().len(), 2);
    }

    #[test]
    fn filter_caps_128() {
        let mut f = full();
        f.set_filter(&"q".repeat(200));
        assert_eq!(f.filter.chars().count(), MAX_FILTER_CHARS);
    }

    #[test]
    fn filter_case_insensitive() {
        let mut f = full();
        f.set_filter("HELLO");
        assert_eq!(f.filtered(), vec!["Hello World".to_string()]);
    }

    #[test]
    fn count_matches_filtered() {
        let mut f = full();
        f.set_filter("bye");
        assert_eq!(f.count(), 1);
    }

    #[test]
    fn cursor_line_mirrors_selected() {
        let f = full();
        assert_eq!(f.cursor_line(), Some("Hello World".to_string()));
    }

    #[test]
    fn status_format() {
        assert_eq!(full().status(), "2 entries cursor 0");
    }
}
