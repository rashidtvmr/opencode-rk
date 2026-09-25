#![forbid(unsafe_code)]

//! Prompt component index order.
//! TS truth: packages/tui/src/component/prompt/index.tsx (prompt order).

/// Ordered prompt index, capped at 16 entries of 64 chars.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PromptIndex {
    order: Vec<String>,
}

const MAX_ENTRIES: usize = 16;
const MAX_CHARS: usize = 64;

fn norm(s: &str) -> String {
    s.chars().take(MAX_CHARS).collect()
}

impl PromptIndex {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn add(&mut self, entry: &str) -> bool {
        let key = norm(entry);
        if key.is_empty() || self.order.contains(&key) {
            return false;
        }
        if self.order.len() >= MAX_ENTRIES {
            self.order.remove(0);
        }
        self.order.push(key);
        true
    }
    pub fn has(&self, entry: &str) -> bool {
        self.order.contains(&norm(entry))
    }
    pub fn len(&self) -> usize {
        self.order.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn add_has_len() {
        let mut i = PromptIndex::new();
        assert!(i.add("a"));
        assert!(i.has("a"));
        assert_eq!(i.len(), 1);
    }
    #[test]
    fn dup_rejected() {
        let mut i = PromptIndex::new();
        assert!(i.add("a"));
        assert!(!i.add("a"));
        assert_eq!(i.len(), 1);
    }
    #[test]
    fn truncates_at_64() {
        let mut i = PromptIndex::new();
        assert!(i.add(&"x".repeat(80)));
        assert!(i.has(&"x".repeat(80)));
        assert_eq!(i.len(), 1);
    }
    #[test]
    fn evicts_oldest_at_16() {
        let mut i = PromptIndex::new();
        for n in 0..17 {
            assert!(i.add(&format!("p{n}")));
        }
        assert_eq!(i.len(), 16);
        assert!(!i.has("p0"));
        assert!(i.has("p16"));
    }
    #[test]
    fn empty_rejected() {
        let mut i = PromptIndex::new();
        assert!(!i.add(""));
        assert_eq!(i.len(), 0);
    }
}
