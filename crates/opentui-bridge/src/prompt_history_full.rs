#![forbid(unsafe_code)]
//! Full prompt history (mirrors [`crate::prompt_history`], plain items).
pub const MAX_ITEMS: usize = 50;
pub const MAX_ITEM: usize = 8 * 1024;
#[derive(Debug, Clone, Default)]
pub struct HistoryFull {
    pub items: Vec<String>,
    pub cursor: Option<usize>,
}
fn floor(s: &str, max: usize) -> usize {
    let mut e = max.min(s.len());
    while e > 0 && !s.is_char_boundary(e) {
        e -= 1;
    }
    e
}
impl HistoryFull {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }
    pub fn push(&mut self, text: &str) {
        let t = text.trim();
        if t.is_empty() {
            return;
        }
        self.items.insert(0, t[..floor(t, MAX_ITEM)].to_string());
        self.items.truncate(MAX_ITEMS);
        self.cursor = None;
    }
    pub fn prev(&mut self) -> Option<&str> {
        if self.items.is_empty() {
            return None;
        }
        let n = match self.cursor {
            None => 0,
            Some(i) if i + 1 < self.items.len() => i + 1,
            _ => return None,
        };
        self.cursor = Some(n);
        Some(self.items[n].as_str())
    }
    pub fn next(&mut self) -> Option<&str> {
        match self.cursor {
            None => None,
            Some(0) => {
                self.cursor = None;
                None
            }
            Some(i) => {
                self.cursor = Some(i - 1);
                Some(self.items[i - 1].as_str())
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn push_fronts_lens() {
        let mut h = HistoryFull::new();
        h.push("a");
        h.push("b");
        assert_eq!(h.len(), 2);
        assert_eq!(h.prev(), Some("b"));
    }
    #[test]
    fn evict_caps_50() {
        let mut h = HistoryFull::new();
        for i in 0..MAX_ITEMS + 10 {
            h.push(&format!("e{i:04}"));
        }
        assert_eq!(h.len(), MAX_ITEMS);
        assert!(h.prev().unwrap().starts_with('e'));
        assert!(!h.items.contains(&"e0000".to_string()));
    }
    #[test]
    fn walk_bounds() {
        let mut h = HistoryFull::new();
        assert_eq!(h.prev(), None);
        assert_eq!(h.next(), None);
        h.push("a");
        h.push("b");
        assert_eq!(h.prev(), Some("b"));
        assert_eq!(h.prev(), Some("a"));
        assert_eq!(h.prev(), None);
        assert_eq!(h.cursor, Some(1));
        assert_eq!(h.next(), Some("b"));
        assert_eq!(h.next(), None);
        assert_eq!(h.cursor, None);
    }
    #[test]
    fn trunc_8kib_blank() {
        let mut h = HistoryFull::new();
        h.push("   ");
        assert_eq!(h.len(), 0);
        h.push(&"x".repeat(MAX_ITEM + 100));
        assert_eq!((h.len(), h.items[0].len()), (1, MAX_ITEM));
    }
    #[test]
    fn push_resets_cursor() {
        let mut h = HistoryFull::new();
        h.push("a");
        h.push("b");
        assert_eq!(h.prev(), Some("b"));
        h.push("c");
        assert_eq!(h.cursor, None);
        assert_eq!(h.prev(), Some("c"));
    }
    #[test]
    fn mb_cut_boundary() {
        let mut h = HistoryFull::new();
        h.push(&format!("{}{}", "e".repeat(MAX_ITEM - 1), "日本語"));
        assert!(h.items[0].len() <= MAX_ITEM);
        assert_eq!(h.prev().unwrap().len(), h.items[0].len());
    }
}
