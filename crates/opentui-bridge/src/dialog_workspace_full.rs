#![forbid(unsafe_code)]
//! Workspace picker dialog (mirrors
//! `packages/tui/src/component/dialog-workspace-list.tsx:16`
//! `DialogWorkspaceList` sorted options + move/select).
//!
//! Divergences: TS `Workspace` records + delete/remove RPCs are host
//! concerns; Rust holds picked directory paths only. Insertion order
//! (host pre-sorts by name). `move_cursor` wraps like `DialogSelect`.

/// Max workspace paths held.
pub const MAX_PATHS: usize = 32;
/// Max chars stored per path.
pub const MAX_PATH_LEN: usize = 512;

/// Picker over workspace directory paths with a wrapping cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceDialog {
    pub paths: Vec<String>,
    pub cursor: usize,
}

impl WorkspaceDialog {
    /// Empty picker, cursor at 0.
    pub fn new() -> Self {
        Self {
            paths: Vec::new(),
            cursor: 0,
        }
    }

    /// Push `path`; blank rejected, long truncated, false when full.
    pub fn push(&mut self, path: &str) -> bool {
        if path.trim().is_empty() || self.paths.len() >= MAX_PATHS {
            return false;
        }
        self.paths.push(path.chars().take(MAX_PATH_LEN).collect());
        true
    }

    /// Move cursor by `delta`, wrapping; no-op when empty.
    pub fn move_cursor(&mut self, delta: isize) {
        if self.paths.is_empty() {
            return;
        }
        let n = self.paths.len() as isize;
        self.cursor = (self.cursor as isize + delta).rem_euclid(n) as usize;
    }

    /// Path under cursor, or `None` when empty/out of bounds.
    pub fn selected(&self) -> Option<&str> {
        self.paths.get(self.cursor).map(String::as_str)
    }
}

impl Default for WorkspaceDialog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_ok_and_selected() {
        let mut d = WorkspaceDialog::new();
        assert!(d.push("/repo/a"));
        assert_eq!(d.selected(), Some("/repo/a"));
    }

    #[test]
    fn push_blank_rejected() {
        let mut d = WorkspaceDialog::new();
        assert!(!d.push("   "));
        assert_eq!(d.selected(), None);
    }

    #[test]
    fn push_full_rejected() {
        let mut d = WorkspaceDialog::new();
        for i in 0..MAX_PATHS {
            assert!(d.push(&format!("/w/{i}")));
        }
        assert!(!d.push("/w/overflow"));
        assert_eq!(d.paths.len(), MAX_PATHS);
    }

    #[test]
    fn push_truncates_to_cap() {
        let mut d = WorkspaceDialog::new();
        assert!(d.push(&"p".repeat(600)));
        assert_eq!(d.paths[0].chars().count(), MAX_PATH_LEN);
    }

    #[test]
    fn move_cursor_wraps_both_ways() {
        let mut d = WorkspaceDialog::new();
        d.push("/a");
        d.push("/b");
        d.move_cursor(1);
        assert_eq!(d.selected(), Some("/b"));
        d.move_cursor(1);
        assert_eq!(d.selected(), Some("/a"));
        d.move_cursor(-1);
        assert_eq!(d.selected(), Some("/b"));
    }

    #[test]
    fn move_cursor_empty_noop() {
        let mut d = WorkspaceDialog::new();
        d.move_cursor(5);
        assert_eq!(d.selected(), None);
    }
}
