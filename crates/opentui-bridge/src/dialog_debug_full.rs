#![forbid(unsafe_code)]
//! Scroll-capped debug-info row buffer (mirrors
//! `packages/tui/src/component/dialog-debug.tsx:24`
//! `entries` memo: label/value rows joined and copied to clipboard by `copy`).

/// Max rows retained.
pub const MAX_ROWS: usize = 64;
/// Max chars per row.
pub const MAX_ROW: usize = 512;

/// Scroll-capped debug row buffer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DebugDialog {
    pub rows: Vec<String>,
}

impl DebugDialog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append row; false when at cap. Truncates char-safe to `MAX_ROW`.
    pub fn push(&mut self, row: &str) -> bool {
        if self.rows.len() >= MAX_ROWS {
            return false;
        }
        self.rows.push(row.chars().take(MAX_ROW).collect());
        true
    }

    /// Last `max` rows, each clipped char-safe to `width` chars.
    pub fn lines(&self, width: usize, max: usize) -> Vec<String> {
        let n = max.min(self.rows.len());
        self.rows[self.rows.len() - n..]
            .iter()
            .map(|r| r.chars().take(width).collect())
            .collect()
    }

    /// Drop all rows.
    pub fn clear(&mut self) {
        self.rows.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_ok_truncates_row() {
        let mut d = DebugDialog::new();
        assert!(d.push(&"r".repeat(600)));
        assert_eq!(d.rows.len(), 1);
        assert_eq!(d.rows[0].chars().count(), MAX_ROW);
    }

    #[test]
    fn push_at_cap_rejected() {
        let mut d = DebugDialog::new();
        for i in 0..MAX_ROWS {
            assert!(d.push(&format!("row {i}")));
        }
        assert!(!d.push("extra"));
        assert_eq!(d.rows.len(), MAX_ROWS);
    }

    #[test]
    fn push_truncates_char_safe() {
        let mut d = DebugDialog::new();
        assert!(d.push(&"e".repeat(MAX_ROW + 10)));
        assert!(d.rows[0].is_char_boundary(d.rows[0].len()));
        assert_eq!(d.rows[0].chars().count(), MAX_ROW);
    }

    #[test]
    fn lines_clips_width_and_caps_max() {
        let mut d = DebugDialog::new();
        for i in 0..5 {
            d.push(&format!("row-{i}-suffix"));
        }
        let out = d.lines(5, 3);
        assert_eq!(out, vec!["row-2", "row-3", "row-4"]);
    }

    #[test]
    fn lines_empty_when_no_rows() {
        let d = DebugDialog::new();
        assert!(d.lines(10, 5).is_empty());
    }

    #[test]
    fn clear_drops_all() {
        let mut d = DebugDialog::new();
        d.push("a");
        d.push("b");
        d.clear();
        assert!(d.rows.is_empty());
        assert!(d.lines(10, 5).is_empty());
        assert!(d.push("c"));
    }
}
