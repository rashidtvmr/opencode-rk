#![forbid(unsafe_code)]
//! Full projection buffer over [`crate::data_projection`] rows (std-only).

/// Max buffered rows.
pub const MAX_PROJ_ROWS: usize = 512;
/// Max chars per row (4KiB).
pub const MAX_ROW_LEN: usize = 4096;

/// Bounded row buffer with a cursor into [`rows`](Self::rows).
#[derive(Debug, Clone, Default)]
pub struct ProjectionFull {
    pub rows: Vec<String>,
    pub cursor: usize,
}

impl ProjectionFull {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a row (truncated to 4KiB); false when full.
    pub fn push(&mut self, row: &str) -> bool {
        if self.rows.len() >= MAX_PROJ_ROWS {
            return false;
        }
        self.rows.push(row.chars().take(MAX_ROW_LEN).collect());
        true
    }

    /// Move cursor by delta, clamped to buffered rows.
    pub fn move_cursor(&mut self, delta: isize) {
        if self.rows.is_empty() {
            self.cursor = 0;
            return;
        }
        let cur = self.cursor as isize + delta;
        self.cursor = cur.clamp(0, self.rows.len() as isize - 1) as usize;
    }

    /// Rows from cursor, capped at `n`.
    #[must_use]
    pub fn window(&self, n: usize) -> Vec<String> {
        self.rows
            .iter()
            .skip(self.cursor)
            .take(n)
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_ok() {
        let mut p = ProjectionFull::new();
        assert!(p.push("a"));
        assert_eq!(p.rows.len(), 1);
    }

    #[test]
    fn push_caps_at_512() {
        let mut p = ProjectionFull::new();
        for i in 0..MAX_PROJ_ROWS {
            assert!(p.push(&format!("r{i}")));
        }
        assert!(!p.push("overflow"));
        assert_eq!(p.rows.len(), MAX_PROJ_ROWS);
    }

    #[test]
    fn push_truncates_row_to_4kib() {
        let mut p = ProjectionFull::new();
        assert!(p.push(&"x".repeat(MAX_ROW_LEN + 10)));
        assert_eq!(p.rows[0].chars().count(), MAX_ROW_LEN);
    }

    #[test]
    fn move_cursor_clamps() {
        let mut p = ProjectionFull::new();
        p.push("a");
        p.push("b");
        p.move_cursor(99);
        assert_eq!(p.cursor, 1);
        p.move_cursor(-99);
        assert_eq!(p.cursor, 0);
    }

    #[test]
    fn move_cursor_empty_stays_zero() {
        let mut p = ProjectionFull::new();
        p.move_cursor(5);
        assert_eq!(p.cursor, 0);
    }

    #[test]
    fn window_slices_from_cursor_capped() {
        let mut p = ProjectionFull::new();
        for r in ["a", "b", "c"] {
            p.push(r);
        }
        p.move_cursor(1);
        assert_eq!(p.window(10), vec!["b".to_string(), "c".to_string()]);
        assert_eq!(p.window(1), vec!["b".to_string()]);
    }
}
