#![forbid(unsafe_code)]
//! Shared scrollback line store.
//!
//! Mirrors `scrollback.shared.ts` line contract: bounded text rows
//! with a sticky pin flag. Oldest rows evict first.

/// Max bytes retained per line (4 KiB, char-boundary truncated).
pub const LINE_CAP: usize = 4 * 1024;

/// Max retained lines. Older lines evict first.
pub const CAP: usize = 2_000;

/// One scrollback line; text capped at [`LINE_CAP`] bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScrollLine {
    pub text: String,
    pub sticky: bool,
}

impl ScrollLine {
    #[must_use]
    pub fn new(text: impl AsRef<str>, sticky: bool) -> Self {
        Self {
            text: truncate(text.as_ref()),
            sticky,
        }
    }
}

fn truncate(s: &str) -> String {
    if s.len() <= LINE_CAP {
        return s.to_owned();
    }
    let mut end = LINE_CAP;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_owned()
}

/// Bounded scrollback store.
#[derive(Debug, Clone, Default)]
pub struct ScrollStore {
    pub lines: Vec<ScrollLine>,
}

impl ScrollStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, text: impl AsRef<str>, sticky: bool) -> bool {
        if self.lines.len() >= CAP {
            self.lines.remove(0);
        }
        self.lines.push(ScrollLine::new(text, sticky));
        true
    }

    #[must_use]
    pub fn sticky_lines(&self) -> Vec<String> {
        self.lines
            .iter()
            .filter(|l| l.sticky)
            .map(|l| l.text.clone())
            .collect()
    }

    #[must_use]
    pub fn tail(&self, n: usize) -> Vec<String> {
        let start = self.lines.len().saturating_sub(n);
        self.lines[start..].iter().map(|l| l.text.clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_evicts_oldest() {
        let mut s = ScrollStore::new();
        for i in 0..CAP + 5 {
            s.push(format!("l{i}"), false);
        }
        assert_eq!(s.lines.len(), CAP);
        assert_eq!(s.lines[0].text, "l5");
    }

    #[test]
    fn sticky_filter() {
        let mut s = ScrollStore::new();
        s.push("a", true);
        s.push("b", false);
        s.push("c", true);
        assert_eq!(s.sticky_lines(), vec!["a".to_string(), "c".to_string()]);
    }

    #[test]
    fn tail_n() {
        let mut s = ScrollStore::new();
        for i in 0..5 {
            s.push(format!("l{i}"), false);
        }
        assert_eq!(s.tail(2), vec!["l3".to_string(), "l4".to_string()]);
    }

    #[test]
    fn tail_over_len() {
        let mut s = ScrollStore::new();
        s.push("a", false);
        assert_eq!(s.tail(99), vec!["a".to_string()]);
    }

    #[test]
    fn cap_truncates() {
        let long = "x".repeat(LINE_CAP + 100);
        let l = ScrollLine::new(&long, false);
        assert_eq!(l.text.len(), LINE_CAP);
        let mut s = ScrollStore::new();
        s.push(long, false);
        assert_eq!(s.lines[0].text.len(), LINE_CAP);
    }

    #[test]
    fn push_returns_true() {
        let mut s = ScrollStore::new();
        assert!(s.push("a", true));
    }
}
