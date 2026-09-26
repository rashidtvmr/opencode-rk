#![forbid(unsafe_code)]
//! Rename-session dialog (mirrors `dialog-session-rename.tsx:18-28` value + `confirm(value)` :21-27).

/// Max chars for session title (TS `DialogPrompt value`, :20).
pub const MAX_TITLE: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameDialog {
    pub current: String,
    pub draft: String,
    pub done: bool,
}

impl RenameDialog {
    pub fn new(current: &str) -> Self {
        let cur: String = current.chars().take(MAX_TITLE).collect();
        Self {
            draft: cur.clone(),
            current: cur,
            done: false,
        }
    }
    #[must_use]
    pub fn draft(&self) -> &str {
        &self.draft
    }
    #[must_use]
    pub fn is_done(&self) -> bool {
        self.done
    }
    pub fn set_draft(&mut self, s: &str) {
        if self.done {
            return;
        }
        self.draft = s.chars().take(MAX_TITLE).collect();
    }
    pub fn submit(&mut self) -> Option<String> {
        if self.done || self.draft.is_empty() {
            return None;
        }
        self.done = true;
        Some(self.draft.clone())
    }
    pub fn cancel(&mut self) {
        self.done = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn new_seeds_draft_from_current() {
        let d = RenameDialog::new("hello");
        assert_eq!(d.current, "hello");
        assert_eq!(d.draft(), "hello");
        assert!(!d.is_done());
    }
    #[test]
    fn set_draft_truncates_to_cap_chars() {
        let mut d = RenameDialog::new("a");
        d.set_draft(&"x".repeat(MAX_TITLE + 10));
        assert_eq!(d.draft().chars().count(), MAX_TITLE);
    }
    #[test]
    fn submit_returns_draft_marks_done() {
        let mut d = RenameDialog::new("old");
        d.set_draft("new");
        assert_eq!(d.submit(), Some("new".to_string()));
        assert!(d.is_done());
        assert_eq!(d.submit(), None);
    }
    #[test]
    fn submit_empty_returns_none() {
        let mut d = RenameDialog::new("old");
        d.set_draft("");
        assert_eq!(d.submit(), None);
        assert!(!d.is_done());
    }
    #[test]
    fn cancel_blocks_submit_and_draft() {
        let mut d = RenameDialog::new("old");
        d.cancel();
        assert!(d.is_done());
        d.set_draft("late");
        assert_eq!(d.draft(), "old");
        assert_eq!(d.submit(), None);
    }
}
