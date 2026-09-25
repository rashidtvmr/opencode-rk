#![forbid(unsafe_code)]
//! Full plugin sidebar row list (mirrors `sidebar/*.tsx` row lists).
//!
//! ponytail: fixed 16-row / 18-line caps only. Upgrade: scrolling viewport.

/// Row cap (fail-closed; TS lists unbounded).
pub const MAX_ROWS: usize = 16;
/// Per-row char cap.
pub const MAX_ROW: usize = 128;
/// Rendered line cap.
pub const MAX_LINES: usize = 18;

/// Bounded sidebar row buffer.
#[derive(Debug, Clone, Default)]
pub struct PluginSidebar {
    rows: Vec<String>,
}

impl PluginSidebar {
    #[must_use]
    pub fn new() -> Self {
        Self { rows: Vec::new() }
    }

    /// Push a row; false when full.
    pub fn push(&mut self, row: &str) -> bool {
        if self.rows.len() >= MAX_ROWS {
            return false;
        }
        self.rows.push(row.chars().take(MAX_ROW).collect());
        true
    }

    /// Rows clipped to `width` chars, capped at 18 lines.
    #[must_use]
    pub fn lines(&self, width: usize) -> Vec<String> {
        self.rows
            .iter()
            .take(MAX_LINES)
            .map(|r| r.chars().take(width).collect())
            .collect()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_and_len() {
        let mut s = PluginSidebar::new();
        assert!(s.is_empty());
        assert!(s.push("a"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn rejects_when_full() {
        let mut s = PluginSidebar::new();
        for i in 0..MAX_ROWS {
            assert!(s.push(&format!("r{i}")));
        }
        assert!(!s.push("overflow"));
        assert_eq!(s.len(), MAX_ROWS);
    }

    #[test]
    fn truncates_long_row() {
        let mut s = PluginSidebar::new();
        assert!(s.push(&"x".repeat(MAX_ROW + 10)));
        assert_eq!(s.lines(usize::MAX)[0].chars().count(), MAX_ROW);
    }

    #[test]
    fn lines_clip_width() {
        let mut s = PluginSidebar::new();
        s.push("abcdef");
        assert_eq!(s.lines(3), vec!["abc".to_string()]);
        assert_eq!(s.lines(0), vec![String::new()]);
    }
}
