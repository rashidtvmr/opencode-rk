#![forbid(unsafe_code)]
//! Prompt autocomplete list (mirrors `packages/tui/src/component/prompt/autocomplete.tsx`).
//!
//! TS: options memo + fuzzysort slice(0,10), move(-1|1) wraps, selected index.

/// Max items retained in a list.
pub const MAX_ITEMS: usize = 32;
/// Max chars for item text.
pub const MAX_TEXT: usize = 256;
/// Max chars for item hint.
pub const MAX_HINT: usize = 128;

fn trunc(s: &str, cap: usize) -> String {
    if s.chars().count() > cap {
        s.chars().take(cap).collect()
    } else {
        s.to_string()
    }
}

/// Single completion entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteItem {
    pub text: String,
    pub hint: String,
}

impl CompleteItem {
    pub fn new(text: &str, hint: &str) -> Self {
        Self {
            text: trunc(text, MAX_TEXT),
            hint: trunc(hint, MAX_HINT),
        }
    }
}

/// Ordered completion list with cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompleteList {
    pub items: Vec<CompleteItem>,
    pub cursor: usize,
}

impl CompleteList {
    pub fn new(mut items: Vec<CompleteItem>) -> Self {
        items.truncate(MAX_ITEMS);
        Self { items, cursor: 0 }
    }

    /// New list of items whose text starts with `prefix`. Cap kept.
    pub fn filter(&self, prefix: &str) -> Self {
        let items: Vec<CompleteItem> = self
            .items
            .iter()
            .filter(|i| i.text.starts_with(prefix))
            .take(MAX_ITEMS)
            .cloned()
            .collect();
        Self { items, cursor: 0 }
    }

    /// Move cursor by delta, wrapping. No-op when empty.
    pub fn move_cursor(&mut self, delta: isize) {
        if self.items.is_empty() {
            return;
        }
        let n = self.items.len() as isize;
        let next = (self.cursor as isize + delta).rem_euclid(n);
        self.cursor = next as usize;
    }

    pub fn current(&self) -> Option<&CompleteItem> {
        self.items.get(self.cursor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(texts: &[&str]) -> CompleteList {
        CompleteList::new(texts.iter().map(|t| CompleteItem::new(t, "")).collect())
    }

    #[test]
    fn filter_matches_prefix() {
        let l = list(&["/plan", "/push", "@file"]);
        let f = l.filter("/");
        assert_eq!(f.items.len(), 2);
        assert_eq!(f.cursor, 0);
    }

    #[test]
    fn filter_none_empty() {
        let l = list(&["/plan", "@file"]);
        let f = l.filter("zzz");
        assert!(f.items.is_empty());
        assert!(f.current().is_none());
    }

    #[test]
    fn cursor_wraps_forward() {
        let mut l = list(&["a", "b", "c"]);
        l.move_cursor(1);
        l.move_cursor(1);
        l.move_cursor(1);
        assert_eq!(l.cursor, 0);
    }

    #[test]
    fn cursor_wraps_backward() {
        let mut l = list(&["a", "b", "c"]);
        l.move_cursor(-1);
        assert_eq!(l.cursor, 2);
        assert_eq!(l.current().unwrap().text, "c");
    }

    #[test]
    fn current_none_empty() {
        let l = CompleteList::new(vec![]);
        assert!(l.current().is_none());
    }

    #[test]
    fn cap_32_items_and_text() {
        let items: Vec<CompleteItem> = (0..40)
            .map(|i| CompleteItem::new(&format!("x{i}"), ""))
            .collect();
        let l = CompleteList::new(items);
        assert_eq!(l.items.len(), 32);
        let long = "a".repeat(300);
        let it = CompleteItem::new(&long, &long);
        assert_eq!(it.text.chars().count(), MAX_TEXT);
        assert_eq!(it.hint.chars().count(), MAX_HINT);
    }
}
