#![forbid(unsafe_code)]
//! Home cards (mirrors `home/tips.tsx` `home_bottom`).
pub const MAX_CARDS: usize = 16;
pub const MAX_CARD_LEN: usize = 128;
pub const MAX_LINES: usize = 18;
fn trunc(s: &str) -> String {
    if s.chars().count() <= MAX_CARD_LEN {
        s.to_string()
    } else {
        s.chars().take(MAX_CARD_LEN).collect()
    }
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PluginHome {
    cards: Vec<String>,
}
impl PluginHome {
    #[must_use]
    pub const fn new() -> Self {
        Self { cards: Vec::new() }
    }
    pub fn push(&mut self, card: &str) -> bool {
        if self.cards.len() >= MAX_CARDS {
            return false;
        }
        self.cards.push(trunc(card));
        true
    }
    #[must_use]
    pub fn lines(&self, width: usize) -> Vec<String> {
        self.cards
            .iter()
            .take(MAX_LINES)
            .map(|c| c.chars().take(width).collect())
            .collect()
    }
    #[must_use]
    pub fn len(&self) -> usize {
        self.cards.len()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty() {
        let h = PluginHome::new();
        assert_eq!(h.len(), 0);
        assert!(h.lines(10).is_empty());
    }
    #[test]
    fn push_ok() {
        let mut h = PluginHome::new();
        assert!(h.push("a"));
        assert_eq!(h.lines(10), vec!["a"]);
    }
    #[test]
    fn trunc_128() {
        let mut h = PluginHome::new();
        assert!(h.push(&"x".repeat(200)));
        assert_eq!(h.lines(200)[0].chars().count(), MAX_CARD_LEN);
    }
    #[test]
    fn cap_16() {
        let mut h = PluginHome::new();
        for i in 0..MAX_CARDS {
            assert!(h.push(&format!("c{i}")));
        }
        assert!(!h.push("full"));
        assert_eq!(h.len(), MAX_CARDS);
    }
    #[test]
    fn clip_width() {
        let mut h = PluginHome::new();
        h.push("abcdef");
        h.push("héllo✓world");
        assert_eq!(h.lines(3), vec!["abc", "hél"]);
        assert_eq!(h.lines(0), vec!["", ""]);
    }
    #[test]
    fn cap_18() {
        let mut h = PluginHome::new();
        for i in 0..MAX_CARDS {
            h.push(&format!("c{i}"));
        }
        assert_eq!(h.lines(80).len(), MAX_CARDS);
        assert!(h.lines(80).len() <= MAX_LINES);
    }
}
