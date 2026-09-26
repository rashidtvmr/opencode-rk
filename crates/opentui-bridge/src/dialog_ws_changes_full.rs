#![forbid(unsafe_code)]
//! Workspace file-changes list (mirrors `packages/tui/src/component/dialog-workspace-file-changes.tsx:1` `DialogWorkspaceFileChanges` file list).
pub const MAX_FILES: usize = 64;
pub const MAX_FILE: usize = 512;
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WsChanges {
    pub files: Vec<String>,
    pub cursor: usize,
}
impl WsChanges {
    pub fn new() -> Self {
        Self::default()
    }
    /// Push file, truncated to MAX_FILE; false when empty or full.
    pub fn push(&mut self, file: &str) -> bool {
        if file.is_empty() || self.files.len() >= MAX_FILES {
            return false;
        }
        self.files.push(file.chars().take(MAX_FILE).collect());
        true
    }
    /// Move cursor by delta, clamped; no-op when empty.
    pub fn move_cursor(&mut self, delta: isize) {
        if self.files.is_empty() {
            self.cursor = 0;
            return;
        }
        let next = self.cursor as isize + delta;
        self.cursor = next.clamp(0, self.files.len() as isize - 1) as usize;
    }
    /// Focused file.
    #[must_use]
    pub fn selected(&self) -> Option<&str> {
        self.files.get(self.cursor).map(String::as_str)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn push_ok_truncates() {
        let mut w = WsChanges::new();
        assert!(w.push("a.rs"));
        assert_eq!(w.selected(), Some("a.rs"));
        assert!(w.push(&"x".repeat(600)));
        assert_eq!(w.files[1].chars().count(), MAX_FILE);
    }
    #[test]
    fn push_rejects_empty_and_full() {
        let mut w = WsChanges::new();
        assert!(!w.push(""));
        for i in 0..MAX_FILES {
            assert!(w.push(&format!("f{i}")));
        }
        assert!(!w.push("extra"));
        assert_eq!(w.files.len(), MAX_FILES);
    }
    #[test]
    fn move_cursor_clamps() {
        let mut w = WsChanges::new();
        w.move_cursor(1);
        assert_eq!(w.cursor, 0);
        for f in ["a", "b", "c"] {
            assert!(w.push(f));
        }
        w.move_cursor(10);
        assert_eq!(w.cursor, 2);
        w.move_cursor(-10);
        assert_eq!(w.cursor, 0);
        w.move_cursor(1);
        assert_eq!(w.selected(), Some("b"));
    }
    #[test]
    fn selected_none_when_empty() {
        let w = WsChanges::new();
        assert_eq!(w.selected(), None);
    }
}
