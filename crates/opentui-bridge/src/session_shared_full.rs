#![forbid(unsafe_code)]
//! Full session header shared by run views (std-only).
//!
//! Evidence: TS `packages/opencode/src/cli/cmd/run/session.shared.ts`
//! (`createSession` turns keyed by session id); sibling
//! `run_session_shared.rs` keeps id/title refs. This adds the full
//! header half: bounded id/title plus a saturating message count.

/// Max chars kept for a session id.
pub const MAX_ID_LEN: usize = 64;
/// Max chars kept for a session title.
pub const MAX_TITLE_LEN: usize = 256;

/// Bounded session header with message count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSharedFull {
    pub id: String,
    pub title: String,
    pub msgs: u32,
}

impl SessionSharedFull {
    /// Build with empty count, truncating over-long fields.
    #[must_use]
    pub fn new(id: &str, title: &str) -> Self {
        Self {
            id: truncate(id, MAX_ID_LEN),
            title: truncate(title, MAX_TITLE_LEN),
            msgs: 0,
        }
    }

    /// Rename; false (no change) on empty title.
    pub fn rename(&mut self, title: &str) -> bool {
        if title.is_empty() {
            return false;
        }
        self.title = truncate(title, MAX_TITLE_LEN);
        true
    }

    /// Saturating message-count increment.
    pub fn bump(&mut self) {
        self.msgs = self.msgs.saturating_add(1);
    }

    /// `"id8 title (N)"` header.
    #[must_use]
    pub fn header(&self) -> String {
        let short: String = self.id.chars().take(8).collect();
        format!("{} {} ({})", short, self.title, self.msgs)
    }
}

fn truncate(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caps_id_and_title() {
        let s = SessionSharedFull::new(&"x".repeat(70), &"y".repeat(300));
        assert_eq!(s.id.len(), 64);
        assert_eq!(s.title.len(), 256);
        assert_eq!(s.msgs, 0);
    }

    #[test]
    fn rename_empty_false() {
        let mut s = SessionSharedFull::new("abc", "T");
        assert!(!s.rename(""));
        assert_eq!(s.title, "T");
    }

    #[test]
    fn rename_ok() {
        let mut s = SessionSharedFull::new("abc", "T");
        assert!(s.rename("New"));
        assert_eq!(s.title, "New");
    }

    #[test]
    fn rename_truncates() {
        let mut s = SessionSharedFull::new("abc", "T");
        assert!(s.rename(&"z".repeat(300)));
        assert_eq!(s.title.len(), 256);
    }

    #[test]
    fn bump_increments() {
        let mut s = SessionSharedFull::new("abc", "T");
        s.bump();
        s.bump();
        assert_eq!(s.msgs, 2);
    }

    #[test]
    fn bump_saturates() {
        let mut s = SessionSharedFull::new("abc", "T");
        s.msgs = u32::MAX;
        s.bump();
        assert_eq!(s.msgs, u32::MAX);
    }

    #[test]
    fn header_parts() {
        let mut s = SessionSharedFull::new("123456789abcdef", "Hi");
        s.bump();
        let h = s.header();
        assert!(h.starts_with("12345678"));
        assert!(h.contains("Hi"));
        assert!(h.ends_with("(1)"));
        assert_eq!(h, "12345678 Hi (1)");
    }
}
