#![forbid(unsafe_code)]
//! Full status dialog: capped title plus status rows.
//!
//! Mirrors `packages/tui/src/component/dialog-status.tsx:10`
//! `DialogStatus` (title "Status" plus status rows).

/// Max chars kept in title.
pub const MAX_TITLE_CHARS: usize = 128;
/// Max rows kept.
pub const MAX_ROWS: usize = 32;
/// Max chars kept per row.
pub const MAX_ROW_CHARS: usize = 256;
/// Max lines returned by [`StatusDialog::lines`].
pub const MAX_LINES: usize = 34;

fn trunc(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

/// Title plus capped status rows.
#[derive(Debug, Clone, Default)]
pub struct StatusDialog {
    pub title: String,
    pub rows: Vec<String>,
}

impl StatusDialog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_title(&mut self, title: &str) {
        self.title = trunc(title, MAX_TITLE_CHARS);
    }

    pub fn push(&mut self, row: &str) -> bool {
        if self.rows.len() >= MAX_ROWS {
            return false;
        }
        self.rows.push(trunc(row, MAX_ROW_CHARS));
        true
    }

    pub fn lines(&self, width: usize) -> Vec<String> {
        let w = width.max(1);
        let mut out = Vec::with_capacity(1 + self.rows.len());
        out.push(trunc(&self.title, w));
        for r in &self.rows {
            out.push(trunc(r, w));
        }
        out.truncate(MAX_LINES);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_caps() {
        let mut d = StatusDialog::new();
        d.set_title(&"t".repeat(200));
        assert_eq!(d.title.chars().count(), MAX_TITLE_CHARS);
    }

    #[test]
    fn push_caps_rows_and_chars() {
        let mut d = StatusDialog::new();
        for i in 0..MAX_ROWS {
            assert!(d.push(&format!("r{i}")));
        }
        assert!(!d.push("overflow"));
        assert_eq!(d.rows.len(), MAX_ROWS);
        let mut e = StatusDialog::new();
        e.push(&"x".repeat(300));
        assert_eq!(e.rows[0].chars().count(), MAX_ROW_CHARS);
    }

    #[test]
    fn lines_clip_width_and_cap() {
        let mut d = StatusDialog::new();
        d.set_title("hello world");
        d.push("abcdef");
        let out = d.lines(3);
        assert_eq!(out, vec!["hel".to_string(), "abc".to_string()]);
        assert!(d.lines(0).iter().all(|l| l.chars().count() <= 1));
    }

    #[test]
    fn lines_unicode_safe() {
        let mut d = StatusDialog::new();
        d.set_title(&"é".repeat(200));
        d.push(&"ü".repeat(300));
        for l in d.lines(4) {
            assert!(l.chars().count() <= 4);
        }
    }
}
