#![forbid(unsafe_code)]
//! Provider-name pick list (mirrors `packages/tui/src/component/dialog-provider.tsx`
//! `DialogProvider` provider `onSelect(name)` + `dialog.clear()` picks).
//! Async load/error states stay with the caller; this is the owned cursor list.

/// Max chars per provider name.
pub const MAX_NAME: usize = 64;
/// Max providers retained.
pub const MAX_PROVIDERS: usize = 32;

/// Cursor list of provider names.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProviderDialog {
    pub names: Vec<String>,
    pub cursor: usize,
}

impl ProviderDialog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append provider; false on blank name or when at cap.
    pub fn push(&mut self, name: &str) -> bool {
        if name.trim().is_empty() || self.names.len() >= MAX_PROVIDERS {
            return false;
        }
        self.names.push(name.chars().take(MAX_NAME).collect());
        true
    }

    /// Move cursor by signed delta, clamped; noop when empty.
    pub fn move_cursor(&mut self, delta: isize) {
        if self.names.is_empty() {
            self.cursor = 0;
            return;
        }
        let next = self.cursor as isize + delta;
        self.cursor = next.clamp(0, self.names.len() as isize - 1) as usize;
    }

    /// Selected provider name, if any.
    pub fn selected(&self) -> Option<&str> {
        self.names.get(self.cursor).map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_ok() {
        let mut d = ProviderDialog::new();
        assert!(d.push("anthropic"));
        assert_eq!(d.selected(), Some("anthropic"));
    }

    #[test]
    fn push_blank_rejected() {
        let mut d = ProviderDialog::new();
        assert!(!d.push("  "));
        assert_eq!(d.selected(), None);
    }

    #[test]
    fn push_caps_at_32() {
        let mut d = ProviderDialog::new();
        for i in 0..MAX_PROVIDERS {
            assert!(d.push(&format!("p{i}")));
        }
        assert!(!d.push("extra"));
    }

    #[test]
    fn push_truncates_to_64() {
        let mut d = ProviderDialog::new();
        assert!(d.push(&"x".repeat(100)));
        assert_eq!(d.names[0].chars().count(), MAX_NAME);
    }

    #[test]
    fn cursor_clamps() {
        let mut d = ProviderDialog::new();
        d.push("a");
        d.push("b");
        d.move_cursor(99);
        assert_eq!(d.selected(), Some("b"));
        d.move_cursor(-99);
        assert_eq!(d.selected(), Some("a"));
    }

    #[test]
    fn cursor_empty_noop() {
        let mut d = ProviderDialog::new();
        d.move_cursor(5);
        assert_eq!(d.cursor, 0);
        assert_eq!(d.selected(), None);
    }
}
