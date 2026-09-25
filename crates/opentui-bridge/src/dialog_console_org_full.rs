#![forbid(unsafe_code)]
//! Console org pick list (mirrors dialog-console-org.tsx:24).

/// Max orgs retained.
pub const MAX_ORGS: usize = 16;
/// Max chars per org name.
pub const MAX_NAME: usize = 128;

/// Org pick list with cursor selection.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConsoleOrg {
    pub orgs: Vec<String>,
    pub cursor: usize,
}

impl ConsoleOrg {
    pub fn new() -> Self {
        Self::default()
    }
    /// Append org; false on blank name or when at cap.
    pub fn push(&mut self, name: &str) -> bool {
        if name.trim().is_empty() || self.orgs.len() >= MAX_ORGS {
            return false;
        }
        self.orgs.push(name.chars().take(MAX_NAME).collect());
        true
    }

    /// Move cursor by delta, clamped to list bounds.
    pub fn move_cursor(&mut self, delta: isize) {
        if self.orgs.is_empty() {
            self.cursor = 0;
            return;
        }
        let next = self.cursor as isize + delta;
        self.cursor = next.clamp(0, self.orgs.len() as isize - 1) as usize;
    }

    /// Cursor-selected org name, if any.
    pub fn selected(&self) -> Option<&str> {
        self.orgs.get(self.cursor).map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_ok_and_selected() {
        let mut c = ConsoleOrg::new();
        assert!(c.push("acme"));
        assert_eq!(c.selected(), Some("acme"));
    }

    #[test]
    fn push_blank_rejected() {
        let mut c = ConsoleOrg::new();
        assert!(!c.push("   "));
        assert_eq!(c.selected(), None);
    }

    #[test]
    fn push_caps_at_16() {
        let mut c = ConsoleOrg::new();
        for i in 0..16 {
            assert!(c.push(&format!("o{i}")));
        }
        assert!(!c.push("extra"));
        assert_eq!(c.orgs.len(), 16);
    }

    #[test]
    fn push_truncates_to_128() {
        let mut c = ConsoleOrg::new();
        assert!(c.push(&"x".repeat(200)));
        assert_eq!(c.orgs[0].chars().count(), 128);
    }

    #[test]
    fn cursor_clamps_both_ends() {
        let mut c = ConsoleOrg::new();
        c.push("a");
        c.push("b");
        c.move_cursor(-5);
        assert_eq!(c.selected(), Some("a"));
        c.move_cursor(99);
        assert_eq!(c.selected(), Some("b"));
    }

    #[test]
    fn cursor_empty_stays_zero() {
        let mut c = ConsoleOrg::new();
        c.move_cursor(3);
        assert_eq!(c.cursor, 0);
        assert_eq!(c.selected(), None);
    }
}
