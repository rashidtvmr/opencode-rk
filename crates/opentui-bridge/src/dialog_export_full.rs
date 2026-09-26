#![forbid(unsafe_code)]

/// Export format picker backing `dialog-export-options.tsx`.
pub struct ExportDialog {
    formats: Vec<String>,
    cursor: usize,
}

impl ExportDialog {
    pub fn new() -> Self {
        Self {
            formats: Vec::new(),
            cursor: 0,
        }
    }

    pub fn push(&mut self, format: &str) -> bool {
        if self.formats.len() >= 8 || format.is_empty() || format.len() > 32 {
            return false;
        }
        self.formats.push(format.to_string());
        true
    }

    pub fn move_cursor(&mut self, delta: isize) {
        if self.formats.is_empty() {
            return;
        }
        let len = self.formats.len() as isize;
        self.cursor = (self.cursor as isize + delta).rem_euclid(len) as usize;
    }

    pub fn selected(&self) -> Option<&str> {
        self.formats.get(self.cursor).map(String::as_str)
    }
}

impl Default for ExportDialog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_accepts_and_selects_first() {
        let mut d = ExportDialog::new();
        assert!(d.push("markdown"));
        assert_eq!(d.selected(), Some("markdown"));
    }

    #[test]
    fn push_rejects_over_cap_and_bad() {
        let mut d = ExportDialog::new();
        for i in 0..8 {
            assert!(d.push(&format!("f{i}")));
        }
        assert!(!d.push("extra"));
        assert!(!d.push(""));
        let long = "x".repeat(33);
        assert!(!d.push(&long));
    }

    #[test]
    fn cursor_wraps_both_ways() {
        let mut d = ExportDialog::new();
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
    fn empty_stays_none() {
        let mut d = ExportDialog::new();
        d.move_cursor(1);
        assert_eq!(d.selected(), None);
    }
}
