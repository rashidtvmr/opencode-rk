#![forbid(unsafe_code)]
//! Active-commit scrollback writer.
//!
//! Staging layer over retained rows: committed rows are bounded history,
//! active is the in-progress streaming row previewed but not yet retained.

/// Maximum retained committed rows.
pub const COMMIT_CAP: usize = 2_000;

/// Maximum retained bytes for the in-progress row.
pub const ACTIVE_CAP: usize = 8 * 1024;

/// Retained history plus one in-progress row.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WriterState {
    pub committed: Vec<String>,
    pub active: Option<String>,
}

impl WriterState {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            committed: Vec::new(),
            active: None,
        }
    }

    /// Append a completed row, evicting oldest beyond [`COMMIT_CAP`].
    pub fn write_line(&mut self, line: String) {
        self.committed.push(line);
        if self.committed.len() > COMMIT_CAP {
            let excess = self.committed.len() - COMMIT_CAP;
            self.committed.drain(..excess);
        }
    }

    /// Stage the in-progress row, truncating to [`ACTIVE_CAP`] bytes.
    pub fn set_active(&mut self, line: String) {
        self.active = Some(truncate_bytes(&line, ACTIVE_CAP));
    }

    /// Commit the staged row into retained history. False if none staged.
    pub fn flush_active(&mut self) -> bool {
        match self.active.take() {
            Some(line) => {
                self.write_line(line);
                true
            }
            None => false,
        }
    }

    /// Last `n` committed rows plus the staged row appended as preview.
    #[must_use]
    pub fn tail(&self, n: usize) -> Vec<String> {
        let start = self.committed.len().saturating_sub(n);
        let mut out: Vec<String> = self.committed[start..].to_vec();
        if let Some(active) = &self.active {
            out.push(active.clone());
        }
        out
    }
}

fn truncate_bytes(s: &str, cap: usize) -> String {
    if s.len() <= cap {
        return s.to_owned();
    }
    let mut end = cap;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_evicts_oldest() {
        let mut w = WriterState::new();
        for i in 0..=COMMIT_CAP {
            w.write_line(format!("row-{i}"));
        }
        assert_eq!(w.committed.len(), COMMIT_CAP);
        assert_eq!(w.committed.first().map(String::as_str), Some("row-1"));
    }

    #[test]
    fn active_truncates_to_cap() {
        let mut w = WriterState::new();
        w.set_active("x".repeat(ACTIVE_CAP + 100));
        assert_eq!(w.active.as_ref().map(String::len), Some(ACTIVE_CAP));
    }

    #[test]
    fn active_truncation_keeps_char_boundary() {
        let mut w = WriterState::new();
        w.set_active("a".repeat(ACTIVE_CAP - 1) + "é rest");
        let active = w.active.clone().unwrap();
        assert!(active.len() <= ACTIVE_CAP);
        assert!(w.active.as_deref().is_some());
    }

    #[test]
    fn flush_moves_active() {
        let mut w = WriterState::new();
        w.set_active("live".to_owned());
        assert!(w.flush_active());
        assert_eq!(w.committed, vec!["live"]);
        assert_eq!(w.active, None);
    }

    #[test]
    fn flush_none_false() {
        let mut w = WriterState::new();
        assert!(!w.flush_active());
        assert!(w.committed.is_empty());
    }

    #[test]
    fn tail_window_with_active() {
        let mut w = WriterState::new();
        for i in 0..5 {
            w.write_line(format!("row-{i}"));
        }
        w.set_active("live".to_owned());
        assert_eq!(w.tail(2), vec!["row-3", "row-4", "live"]);
    }
}
