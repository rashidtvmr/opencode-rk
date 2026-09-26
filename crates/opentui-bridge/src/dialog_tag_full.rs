#![forbid(unsafe_code)]

//! Tag dialog selection state (dialog-tag.tsx:8): filtered tag list, cursor, select.

/// Max tags retained; each tag capped at 64 bytes (byte length).
pub const MAX_TAGS: usize = 32;
/// Max byte length per tag.
pub const MAX_TAG_LEN: usize = 64;

/// Selectable tag list with cursor.
#[derive(Debug, Default, Clone)]
pub struct TagDialog {
    /// Stored tags.
    pub tags: Vec<String>,
    /// Cursor index into tags.
    pub cursor: usize,
}

impl TagDialog {
    /// Empty dialog.
    pub fn new() -> Self {
        Self::default()
    }

    /// Push trimmed tag; false when empty, overlong, or full.
    pub fn push(&mut self, tag: &str) -> bool {
        let t = tag.trim();
        if t.is_empty() || t.len() > MAX_TAG_LEN || self.tags.len() >= MAX_TAGS {
            return false;
        }
        self.tags.push(t.to_string());
        true
    }

    /// Move cursor by delta with wrap; no-op on empty.
    pub fn move_cursor(&mut self, delta: isize) {
        if self.tags.is_empty() {
            self.cursor = 0;
            return;
        }
        let n = self.tags.len() as isize;
        self.cursor = (self.cursor as isize + delta).rem_euclid(n) as usize;
    }

    /// Selected tag, if any.
    pub fn selected(&self) -> Option<&str> {
        self.tags.get(self.cursor).map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_accepts_tag() {
        let mut d = TagDialog::new();
        assert!(d.push("v1.2"));
        assert_eq!(d.selected(), Some("v1.2"));
    }

    #[test]
    fn push_rejects_empty() {
        let mut d = TagDialog::new();
        assert!(!d.push("   "));
        assert_eq!(d.selected(), None);
    }

    #[test]
    fn push_rejects_overlong() {
        let mut d = TagDialog::new();
        assert!(!d.push(&"x".repeat(65)));
    }

    #[test]
    fn push_enforces_cap() {
        let mut d = TagDialog::new();
        for i in 0..MAX_TAGS {
            assert!(d.push(&format!("t{i}")));
        }
        assert!(!d.push("extra"));
        assert_eq!(d.tags.len(), MAX_TAGS);
    }

    #[test]
    fn cursor_wraps() {
        let mut d = TagDialog::new();
        d.push("a");
        d.push("b");
        d.move_cursor(1);
        assert_eq!(d.selected(), Some("b"));
        d.move_cursor(1);
        assert_eq!(d.selected(), Some("a"));
        d.move_cursor(-1);
        assert_eq!(d.selected(), Some("b"));
    }

    #[test]
    fn move_empty_stays_zero() {
        let mut d = TagDialog::new();
        d.move_cursor(3);
        assert_eq!(d.cursor, 0);
        assert_eq!(d.selected(), None);
    }
}
