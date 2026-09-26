#![forbid(unsafe_code)]

//! Prompt autocomplete candidates.
//! TS truth: packages/tui/src/component/prompt/autocomplete.tsx (options memo).

/// Autocomplete candidate list with wrapping cursor.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PromptAuto {
    pub cands: Vec<String>,
    pub cursor: usize,
}

const MAX_CANDS: usize = 32;
const MAX_CHARS: usize = 128;

impl PromptAuto {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn push(&mut self, s: &str) -> bool {
        if self.cands.len() >= MAX_CANDS || s.is_empty() {
            return false;
        }
        self.cands.push(s.chars().take(MAX_CHARS).collect());
        true
    }
    pub fn move_cursor(&mut self, delta: isize) {
        if self.cands.is_empty() {
            return;
        }
        let n = self.cands.len() as isize;
        self.cursor = (self.cursor as isize + delta).rem_euclid(n) as usize;
    }
    pub fn selected(&self) -> Option<&str> {
        self.cands.get(self.cursor).map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn push_caps_at_32() {
        let mut p = PromptAuto::new();
        for i in 0..40 {
            p.push(&format!("c{i}"));
        }
        assert_eq!(p.cands.len(), 32);
        assert!(!p.push("extra"));
    }
    #[test]
    fn push_truncates_128_and_rejects_empty() {
        let mut p = PromptAuto::new();
        assert!(!p.push(""));
        assert!(p.push(&"a".repeat(200)));
        assert_eq!(p.cands[0].chars().count(), 128);
    }
    #[test]
    fn cursor_wraps_forward() {
        let mut p = PromptAuto::new();
        for s in ["a", "b", "c"] {
            p.push(s);
        }
        p.move_cursor(3);
        assert_eq!(p.cursor, 0);
        assert_eq!(p.selected(), Some("a"));
    }
    #[test]
    fn cursor_wraps_backward() {
        let mut p = PromptAuto::new();
        for s in ["a", "b", "c"] {
            p.push(s);
        }
        p.move_cursor(-1);
        assert_eq!(p.cursor, 2);
        assert_eq!(p.selected(), Some("c"));
    }
    #[test]
    fn selected_none_when_empty() {
        assert_eq!(PromptAuto::new().selected(), None);
    }
}
