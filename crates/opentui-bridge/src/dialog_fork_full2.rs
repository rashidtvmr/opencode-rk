#![forbid(unsafe_code)]
//! Cursor list for full-session fork picks (mirrors
//! `packages/tui/src/routes/session/dialog-fork-from-timeline.tsx:12`
//! `DialogForkFromTimeline` option list; pick storage here, confirm gate in
//! `crate::fork_dialog_full` / `crate::session_fork_dialog`).

/// Max entries retained.
pub const MAX_ENTRIES: usize = 32;
/// Max chars per entry.
pub const MAX_LEN: usize = 256;

/// Cursor over forkable timeline entries.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ForkPick {
    pub entries: Vec<String>,
    pub cursor: usize,
}

impl ForkPick {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append entry; false on blank or at cap. Truncates to [`MAX_LEN`] chars.
    pub fn push(&mut self, entry: &str) -> bool {
        if entry.trim().is_empty() || self.entries.len() >= MAX_ENTRIES {
            return false;
        }
        self.entries.push(entry.chars().take(MAX_LEN).collect());
        true
    }

    /// Move cursor by delta, clamped to entries. No-op when empty.
    pub fn move_cursor(&mut self, delta: isize) {
        if self.entries.is_empty() {
            self.cursor = 0;
            return;
        }
        let next = self.cursor as isize + delta;
        self.cursor = next.clamp(0, self.entries.len() as isize - 1) as usize;
    }

    /// Entry under cursor, if any.
    pub fn selected(&self) -> Option<&str> {
        self.entries.get(self.cursor).map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_empty_no_selection() {
        let f = ForkPick::new();
        assert!(f.entries.is_empty());
        assert_eq!(f.cursor, 0);
        assert_eq!(f.selected(), None);
    }

    #[test]
    fn push_ok_selects_first() {
        let mut f = ForkPick::new();
        assert!(f.push("msg1"));
        assert_eq!(f.selected(), Some("msg1"));
    }

    #[test]
    fn push_blank_rejected() {
        let mut f = ForkPick::new();
        assert!(!f.push("   "));
        assert!(f.entries.is_empty());
    }

    #[test]
    fn push_truncates_entry() {
        let mut f = ForkPick::new();
        assert!(f.push(&"m".repeat(300)));
        assert_eq!(f.entries[0].chars().count(), MAX_LEN);
    }

    #[test]
    fn push_at_cap_rejected() {
        let mut f = ForkPick::new();
        for i in 0..MAX_ENTRIES {
            assert!(f.push(&format!("m{i}")));
        }
        assert!(!f.push("extra"));
        assert_eq!(f.entries.len(), MAX_ENTRIES);
    }

    #[test]
    fn cursor_clamps_both_ends() {
        let mut f = ForkPick::new();
        f.move_cursor(5);
        assert_eq!(f.cursor, 0);
        f.push("a");
        f.push("b");
        f.move_cursor(99);
        assert_eq!(f.cursor, 1);
        assert_eq!(f.selected(), Some("b"));
        f.move_cursor(-99);
        assert_eq!(f.cursor, 0);
        assert_eq!(f.selected(), Some("a"));
    }
}
