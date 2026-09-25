#![forbid(unsafe_code)]

//! Session flow: [`SessionIndex`] + capped session id with line helpers.
//!
//! TS truth: `crate::session_index::SessionIndex`,
//! `crate::id8::{id8, session_line}` (read-only reuse).

use crate::id8::{id8, session_line};
use crate::session_index::SessionIndex;

/// Max chars retained for the session id.
pub const MAX_ID_LEN: usize = 128;

/// Index state paired with its session id.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionFlow {
    pub idx: SessionIndex,
    pub id: String,
}

impl SessionFlow {
    /// Empty index with empty id.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set id, truncated to 128 chars (char-safe).
    pub fn set_id(&mut self, id: &str) {
        self.id = id.chars().take(MAX_ID_LEN).collect();
    }

    /// `"session <id8> <state> <n> msgs"` via [`session_line`].
    #[must_use]
    pub fn line(&self, state: &str, n: usize) -> String {
        session_line(&self.id, state, n)
    }

    /// First 8 chars of id via [`id8`].
    #[must_use]
    pub fn short(&self) -> String {
        id8(&self.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_is_empty() {
        let f = SessionFlow::new();
        assert_eq!(f.id, "");
        assert_eq!(f.idx, SessionIndex::new());
    }

    #[test]
    fn set_id_stores() {
        let mut f = SessionFlow::new();
        f.set_id("ses_abcdef123456");
        assert_eq!(f.id, "ses_abcdef123456");
    }

    #[test]
    fn set_id_truncates_to_128() {
        let mut f = SessionFlow::new();
        f.set_id(&"x".repeat(200));
        assert_eq!(f.id.chars().count(), MAX_ID_LEN);
    }

    #[test]
    fn short_is_id8() {
        let mut f = SessionFlow::new();
        f.set_id("ses_abcdef123456");
        assert_eq!(f.short(), "ses_abcd");
    }

    #[test]
    fn line_delegates_to_session_line() {
        let mut f = SessionFlow::new();
        f.set_id("ses_abcdef123");
        assert_eq!(f.line("idle", 3), "session ses_abcd idle 3 msgs");
    }
}
