#![forbid(unsafe_code)]
//! Workspace unavailable dialog (mirrors
//! `packages/tui/src/component/dialog-workspace-unavailable.tsx:1`
//! "Workspace Unavailable" + restore/cancel prompt).

/// Max chars for workspace path.
pub const MAX_PATH: usize = 512;
/// Max chars for unavailable reason.
pub const MAX_REASON: usize = 256;
/// Max chars for rendered line.
pub const MAX_LINE: usize = 512;

/// Workspace-down notice state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WsDown {
    pub path: String,
    pub reason: String,
}

impl WsDown {
    /// Build, truncating `path` to [`MAX_PATH`] and `reason` to [`MAX_REASON`] chars.
    pub fn new(path: &str, reason: &str) -> Self {
        Self {
            path: path.chars().take(MAX_PATH).collect(),
            reason: reason.chars().take(MAX_REASON).collect(),
        }
    }

    /// One-line notice, truncated to [`MAX_LINE`] chars.
    pub fn line(&self) -> String {
        let raw = if self.reason.trim().is_empty() {
            format!("workspace unavailable: {}", self.path)
        } else {
            format!("workspace unavailable: {} ({})", self.path, self.reason)
        };
        raw.chars().take(MAX_LINE).collect()
    }

    /// Borrow the stored path.
    pub fn path_of(&self) -> &str {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_truncates_path() {
        let w = WsDown::new(&"p".repeat(600), "gone");
        assert_eq!(w.path.chars().count(), MAX_PATH);
    }

    #[test]
    fn new_truncates_reason() {
        let w = WsDown::new("/tmp/ws", &"r".repeat(300));
        assert_eq!(w.reason.chars().count(), MAX_REASON);
    }

    #[test]
    fn line_caps_512() {
        let w = WsDown::new(&"p".repeat(512), &"r".repeat(256));
        assert!(w.line().chars().count() <= MAX_LINE);
    }

    #[test]
    fn path_of_returns_path() {
        let w = WsDown::new("/tmp/ws", "gone");
        assert_eq!(w.path_of(), "/tmp/ws");
    }

    #[test]
    fn line_hides_blank_reason() {
        let w = WsDown::new("/tmp/ws", "   ");
        assert_eq!(w.line(), "workspace unavailable: /tmp/ws");
    }
}
