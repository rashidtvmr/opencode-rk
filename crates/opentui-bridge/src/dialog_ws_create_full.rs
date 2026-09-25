#![forbid(unsafe_code)]
//! Workspace create dialog (mirrors
//! `packages/tui/src/component/dialog-workspace-create.tsx:1`
//! `WorkspaceSelection::new` workspaceName + target path entry).

/// Max chars for workspace name.
pub const MAX_NAME: usize = 128;
/// Max chars for workspace path.
pub const MAX_PATH: usize = 512;

/// New-workspace form state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspaceCreate {
    pub name: String,
    pub path: String,
    pub done: bool,
}

impl WorkspaceCreate {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set name, truncated to [`MAX_NAME`] chars.
    pub fn set_name(&mut self, name: &str) {
        self.name = name.chars().take(MAX_NAME).collect();
    }

    /// Set path, truncated to [`MAX_PATH`] chars.
    pub fn set_path(&mut self, path: &str) {
        self.path = path.chars().take(MAX_PATH).collect();
    }

    /// Submit; `None` when name blank, else marks done and returns name.
    pub fn submit(&mut self) -> Option<String> {
        if self.name.trim().is_empty() {
            return None;
        }
        self.done = true;
        Some(self.name.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_name_truncates() {
        let mut w = WorkspaceCreate::new();
        w.set_name(&"n".repeat(200));
        assert_eq!(w.name.chars().count(), MAX_NAME);
    }

    #[test]
    fn set_path_truncates() {
        let mut w = WorkspaceCreate::new();
        w.set_path(&"p".repeat(600));
        assert_eq!(w.path.chars().count(), MAX_PATH);
    }

    #[test]
    fn submit_empty_none() {
        let mut w = WorkspaceCreate::new();
        w.set_name("   ");
        assert_eq!(w.submit(), None);
        assert!(!w.done);
    }

    #[test]
    fn submit_ok_done() {
        let mut w = WorkspaceCreate::new();
        w.set_name("demo");
        w.set_path("/tmp/x");
        assert_eq!(w.submit(), Some("demo".to_string()));
        assert!(w.done);
    }

    #[test]
    fn submit_blank_path_still_ok() {
        let mut w = WorkspaceCreate::new();
        w.set_name("ws");
        assert_eq!(w.submit(), Some("ws".to_string()));
        assert!(w.done);
    }
}
