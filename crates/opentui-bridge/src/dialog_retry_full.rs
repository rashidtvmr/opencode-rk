#![forbid(unsafe_code)]
//! Retry dialog action picker (mirrors `dialog-retry-action.tsx` selected/action).
//!
//! Source: dialog-retry-action.tsx:39-46 (selected/action two-choice,
//! default action, left/right cursor).

/// Max retry actions.
pub const MAX_ACTIONS: usize = 8;
/// Max action label chars.
pub const MAX_LABEL: usize = 64;

/// Retry action picker with cursor + attempt counter.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RetryDialog {
    pub actions: Vec<String>,
    pub cursor: usize,
    pub attempts: u32,
}

impl RetryDialog {
    /// Empty picker.
    pub fn new() -> Self {
        Self::default()
    }
    /// Push label; false when full/empty/oversize.
    pub fn push(&mut self, label: &str) -> bool {
        if self.actions.len() >= MAX_ACTIONS || label.is_empty() || label.len() > MAX_LABEL {
            return false;
        }
        self.actions.push(label.to_string());
        true
    }
    /// Move cursor by delta, clamped.
    pub fn move_cursor(&mut self, delta: isize) {
        if self.actions.is_empty() {
            self.cursor = 0;
            return;
        }
        let next = self.cursor as isize + delta;
        self.cursor = next.clamp(0, self.actions.len() as isize - 1) as usize;
    }
    /// Select current action, bump attempts.
    pub fn retry(&mut self) -> Option<String> {
        let sel = self.actions.get(self.cursor)?.clone();
        self.attempts = self.attempts.saturating_add(1);
        Some(sel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn push_cap() {
        let mut d = RetryDialog::new();
        for i in 0..MAX_ACTIONS {
            assert!(d.push(&format!("a{i}")));
        }
        assert!(!d.push("full"));
    }
    #[test]
    fn push_rejects_bad() {
        let mut d = RetryDialog::new();
        assert!(!d.push(""));
        assert!(!d.push(&"x".repeat(MAX_LABEL + 1)));
    }
    #[test]
    fn cursor_clamps() {
        let mut d = RetryDialog::new();
        d.push("a");
        d.push("b");
        d.move_cursor(99);
        assert_eq!(d.cursor, 1);
        d.move_cursor(-99);
        assert_eq!(d.cursor, 0);
    }
    #[test]
    fn retry_bumps() {
        let mut d = RetryDialog::new();
        assert_eq!(d.retry(), None);
        d.push("retry");
        d.push("dismiss");
        d.move_cursor(1);
        assert_eq!(d.retry(), Some("dismiss".to_string()));
        assert_eq!(d.attempts, 1);
    }
}
