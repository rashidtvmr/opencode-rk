#![forbid(unsafe_code)]
//! Variant select list (mirrors
//! `packages/tui/src/component/dialog-variant.tsx:10`
//! `DialogVariant` options memo + flat `DialogSelect` pick).

/// Max variant options retained.
pub const MAX_OPTS: usize = 16;
/// Max chars per option label.
pub const MAX_OPT: usize = 128;

/// Flat variant picker with cursor over option labels.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VariantDialog {
    pub opts: Vec<String>,
    pub cursor: usize,
}

impl VariantDialog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append option; false on blank label or when at cap.
    pub fn push(&mut self, opt: &str) -> bool {
        if opt.trim().is_empty() || self.opts.len() >= MAX_OPTS {
            return false;
        }
        self.opts.push(opt.chars().take(MAX_OPT).collect());
        true
    }

    /// Move cursor by delta with wraparound; no-op when empty.
    pub fn move_cursor(&mut self, delta: isize) {
        if self.opts.is_empty() {
            return;
        }
        let n = self.opts.len() as isize;
        self.cursor = (self.cursor as isize + delta).rem_euclid(n) as usize;
    }

    /// Cursor-selected option label, if any.
    pub fn selected(&self) -> Option<&str> {
        self.opts.get(self.cursor).map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_ok_selected() {
        let mut d = VariantDialog::new();
        assert!(d.push("default"));
        assert!(d.push("fast"));
        assert_eq!(d.selected(), Some("default"));
    }

    #[test]
    fn push_blank_rejected() {
        let mut d = VariantDialog::new();
        assert!(!d.push("   "));
        assert!(d.opts.is_empty());
        assert_eq!(d.selected(), None);
    }

    #[test]
    fn push_truncates_label() {
        let mut d = VariantDialog::new();
        assert!(d.push(&"v".repeat(200)));
        assert_eq!(d.opts[0].chars().count(), MAX_OPT);
    }

    #[test]
    fn push_at_cap_rejected() {
        let mut d = VariantDialog::new();
        for i in 0..MAX_OPTS {
            assert!(d.push(&format!("v{i}")));
        }
        assert!(!d.push("extra"));
        assert_eq!(d.opts.len(), MAX_OPTS);
    }

    #[test]
    fn cursor_wraps_both_ways() {
        let mut d = VariantDialog::new();
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
    fn cursor_empty_noop() {
        let mut d = VariantDialog::new();
        d.move_cursor(1);
        assert_eq!(d.cursor, 0);
        assert_eq!(d.selected(), None);
    }
}
