#![forbid(unsafe_code)]

//! Full command palette: filterable command list (mirrors
//! `packages/tui/src/component/command-palette.tsx:48-60` options mapping).

/// Max stored items.
pub const MAX_ITEMS: usize = 64;
/// Max chars per item.
pub const MAX_ITEM: usize = 128;
/// Max query chars.
pub const MAX_QUERY: usize = 128;
/// Max filtered rows returned.
pub const MAX_SHOWN: usize = 32;

/// Filterable command list with query cursor.
#[derive(Debug, Default, Clone)]
pub struct CmdPalette {
    pub items: Vec<String>,
    pub query: String,
    pub cursor: usize,
}

impl CmdPalette {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn push(&mut self, item: &str) -> bool {
        if self.items.len() >= MAX_ITEMS || item.is_empty() {
            return false;
        }
        self.items.push(item.chars().take(MAX_ITEM).collect());
        true
    }
    pub fn set_query(&mut self, q: &str) {
        self.query = q.chars().take(MAX_QUERY).collect();
        self.cursor = 0;
    }
    pub fn filtered(&self) -> Vec<String> {
        let q = self.query.to_lowercase();
        let mut out = Vec::new();
        for item in &self.items {
            if out.len() >= MAX_SHOWN {
                break;
            }
            if q.is_empty() || item.to_lowercase().contains(&q) {
                out.push(item.clone());
            }
        }
        out
    }
    pub fn move_cursor(&mut self, delta: isize) {
        let n = self.filtered().len() as isize;
        if n == 0 {
            return;
        }
        self.cursor = (self.cursor as isize + delta).rem_euclid(n) as usize;
    }
    pub fn selected_filtered(&self) -> Option<String> {
        self.filtered().get(self.cursor).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_selected_none() {
        let p = CmdPalette::new();
        assert!(p.filtered().is_empty());
        assert!(p.selected_filtered().is_none());
    }
    #[test]
    fn push_caps() {
        let mut p = CmdPalette::new();
        assert!(!p.push(""));
        assert!(p.push(&"x".repeat(MAX_ITEM + 10)));
        assert_eq!(p.items[0].chars().count(), MAX_ITEM);
        for i in 0..MAX_ITEMS {
            let _ = p.push(&format!("c{i}"));
        }
        assert_eq!(p.items.len(), MAX_ITEMS);
        assert!(!p.push("extra"));
    }
    #[test]
    fn query_cap_and_reset() {
        let mut p = CmdPalette::new();
        p.push("alpha");
        p.cursor = 5;
        p.set_query(&"q".repeat(MAX_QUERY + 10));
        assert_eq!(p.query.chars().count(), MAX_QUERY);
        assert_eq!(p.cursor, 0);
    }
    #[test]
    fn filter_case_insensitive_and_cap() {
        let mut p = CmdPalette::new();
        p.push("Session.New");
        p.push("session.open");
        p.push("theme.dark");
        p.set_query("SESSION");
        assert_eq!(p.filtered().len(), 2);
        p.set_query("zzz");
        assert!(p.filtered().is_empty());
        assert!(p.selected_filtered().is_none());
    }
    #[test]
    fn shown_caps_at_32() {
        let mut p = CmdPalette::new();
        for i in 0..MAX_ITEMS {
            p.push(&format!("cmd-{i}"));
        }
        assert_eq!(p.filtered().len(), MAX_SHOWN);
    }
    #[test]
    fn cursor_wraps_and_follows_query() {
        let mut p = CmdPalette::new();
        p.push("alpha");
        p.push("beta");
        p.move_cursor(1);
        assert_eq!(p.selected_filtered().as_deref(), Some("beta"));
        p.move_cursor(1);
        assert_eq!(p.selected_filtered().as_deref(), Some("alpha"));
        p.move_cursor(-1);
        assert_eq!(p.selected_filtered().as_deref(), Some("beta"));
        p.set_query("beta");
        assert_eq!(p.selected_filtered().as_deref(), Some("beta"));
    }
}
