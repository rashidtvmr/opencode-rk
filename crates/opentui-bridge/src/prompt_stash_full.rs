#![forbid(unsafe_code)]
//! Prompt stash (TS truth: `packages/tui/src/prompt/stash.tsx`).
//! LIFO stack of stashed prompt inputs; cap 16, each 4KiB chars.
pub const MAX_STASH: usize = 16;
pub const MAX_CHARS: usize = 4096;
#[derive(Debug, Clone, Default)]
pub struct PromptStash {
    items: Vec<String>,
}
impl PromptStash {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
    pub fn stash(&mut self, text: &str) -> bool {
        if text.trim().is_empty() {
            return false;
        }
        let t: String = text.chars().take(MAX_CHARS).collect();
        self.items.push(t);
        if self.items.len() > MAX_STASH {
            self.items.remove(0);
        }
        true
    }
    pub fn pop(&mut self) -> Option<String> {
        self.items.pop()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stash_pop_lifo() {
        let mut s = PromptStash::new();
        assert!(s.is_empty());
        assert!(s.stash("a"));
        assert!(s.stash("b"));
        assert_eq!(s.len(), 2);
        assert_eq!(s.pop().as_deref(), Some("b"));
        assert_eq!(s.pop().as_deref(), Some("a"));
        assert_eq!(s.pop(), None);
    }
    #[test]
    fn cap_16_evicts_oldest() {
        let mut s = PromptStash::new();
        for i in 0..MAX_STASH + 4 {
            assert!(s.stash(&format!("e{i:02}")));
        }
        assert_eq!(s.len(), MAX_STASH);
        assert_eq!(s.pop().as_deref(), Some("e19"));
        let mut v = vec![];
        while let Some(t) = s.pop() {
            v.push(t);
        }
        assert!(!v.iter().any(|t| t == "e00"));
        assert!(v.iter().any(|t| t == "e04"));
    }
    #[test]
    fn rejects_blank() {
        let mut s = PromptStash::new();
        assert!(!s.stash("   "));
        assert!(!s.stash(""));
        assert!(s.is_empty());
    }
    #[test]
    fn truncates_to_4kib_chars() {
        let mut s = PromptStash::new();
        let big = "e".repeat(MAX_CHARS + 100);
        assert!(s.stash(&big));
        assert_eq!(s.pop().unwrap().chars().count(), MAX_CHARS);
        let uni = "あ".repeat(MAX_CHARS + 10);
        assert!(s.stash(&uni));
        let got = s.pop().unwrap();
        assert_eq!(got.chars().count(), MAX_CHARS);
    }
}
