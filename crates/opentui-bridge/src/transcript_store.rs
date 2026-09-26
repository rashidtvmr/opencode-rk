//! Bounded transcript tail store (native TUI transcript path).
//!
//! Mirrors `ScrollbackSharedFull` caps (500 lines, 2KiB chars/line) without
//! scroll offset; `tui_entry` drains at most `MAX_NATIVE_TRANSCRIPT` (500).

#![forbid(unsafe_code)]

/// Max retained lines. Older lines evict first.
pub const MAX_TRANSCRIPT_LINES: usize = 500;
/// Max chars retained per line.
pub const LINE_CAP_CHARS: usize = 2 * 1024;

/// Bounded transcript store: newest lines at the back.
#[derive(Debug, Clone, Default)]
pub struct TranscriptStore {
    pub lines: Vec<String>,
}

impl TranscriptStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, line: &str) {
        if self.lines.len() >= MAX_TRANSCRIPT_LINES {
            self.lines.remove(0);
        }
        self.lines.push(truncate(line));
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    pub fn clear(&mut self) {
        self.lines.clear();
    }

    #[must_use]
    pub fn tail(&self, n: usize) -> &[String] {
        let start = self.lines.len().saturating_sub(n);
        &self.lines[start..]
    }

    #[must_use]
    pub fn as_vec(&self) -> Vec<String> {
        self.lines.clone()
    }
}

fn truncate(s: &str) -> String {
    if s.chars().count() <= LINE_CAP_CHARS {
        return s.to_owned();
    }
    s.chars().take(LINE_CAP_CHARS).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_and_len() {
        let mut t = TranscriptStore::new();
        assert!(t.is_empty());
        t.push("a");
        t.push("b");
        assert_eq!(t.len(), 2);
    }

    #[test]
    fn evicts_oldest_at_cap() {
        let mut t = TranscriptStore::new();
        for i in 0..MAX_TRANSCRIPT_LINES + 2 {
            t.push(&format!("l{i}"));
        }
        assert_eq!(t.len(), MAX_TRANSCRIPT_LINES);
        assert_eq!(t.as_vec()[0], "l2");
    }

    #[test]
    fn truncates_to_2kib_chars() {
        let mut t = TranscriptStore::new();
        t.push(&"x".repeat(3000));
        assert_eq!(t.as_vec()[0].chars().count(), LINE_CAP_CHARS);
        t.push(&"é".repeat(3000));
        assert_eq!(t.as_vec()[1].chars().count(), LINE_CAP_CHARS);
    }

    #[test]
    fn clear_empties() {
        let mut t = TranscriptStore::new();
        t.push("a");
        t.clear();
        assert!(t.is_empty());
        assert_eq!(t.len(), 0);
    }

    #[test]
    fn tail_returns_newest_n() {
        let mut t = TranscriptStore::new();
        for i in 0..5 {
            t.push(&format!("l{i}"));
        }
        let tail: Vec<&str> = t.tail(2).iter().map(String::as_str).collect();
        assert_eq!(tail, ["l3", "l4"]);
        assert_eq!(t.tail(99).len(), 5);
        assert!(TranscriptStore::new().tail(3).is_empty());
    }

    #[test]
    fn as_vec_clones_all() {
        let mut t = TranscriptStore::new();
        t.push("a");
        t.push("b");
        assert_eq!(t.as_vec(), vec!["a".to_owned(), "b".to_owned()]);
    }
}
