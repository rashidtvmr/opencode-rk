#![forbid(unsafe_code)]
//! Full run command palette: typed query over `FooterMenuFull`.
//! TS truth: `footer.command.tsx` filters panel entries by query and
//! confirms the highlighted row; `FooterMenuFull` owns items/cursor/open.

use crate::footer_menu_full::FooterMenuFull;

/// Max chars kept in query (mirrors item cap in `FooterMenuFull`).
pub const MAX_QUERY_LEN: usize = 128;
/// Max rows returned by `matches`.
pub const MAX_MATCHES: usize = 32;

/// Command palette: query string plus popup menu.
/// `matches` only sees the highlighted row: the item list stays in the menu.
#[derive(Debug, Clone, Default)]
pub struct CommandFull {
    pub menu: FooterMenuFull,
    pub query: String,
}

impl CommandFull {
    pub fn new(menu: FooterMenuFull) -> Self {
        Self {
            menu,
            query: String::new(),
        }
    }

    /// Replace query, truncated to `MAX_QUERY_LEN` chars.
    pub fn set_query(&mut self, q: &str) {
        self.query = q.chars().take(MAX_QUERY_LEN).collect();
    }

    /// Selected row when it contains the query (case-insensitive).
    /// Empty query matches any selected row. At most one row, so
    /// `MAX_MATCHES` always holds. Empty when the menu is closed.
    pub fn matches(&self) -> Vec<String> {
        match self.menu.selected() {
            Some(row)
                if row
                    .to_lowercase()
                    .contains(&self.query.trim().to_lowercase()) =>
            {
                vec![row.to_string()]
            }
            _ => Vec::new(),
        }
    }

    /// Selected row text when the menu is open.
    pub fn confirm(&self) -> Option<String> {
        self.menu.selected().map(str::to_string)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_two() -> CommandFull {
        let mut menu = FooterMenuFull::new();
        assert!(menu.add_item("Build Project"));
        assert!(menu.add_item("Run Tests"));
        assert!(menu.open_menu());
        CommandFull::new(menu)
    }

    #[test]
    fn set_query_caps_at_128_chars() {
        let mut c = open_two();
        c.set_query(&"q".repeat(200));
        assert_eq!(c.query.chars().count(), MAX_QUERY_LEN);
    }

    #[test]
    fn matches_hits_case_insensitively() {
        let mut c = open_two();
        c.set_query("build");
        assert_eq!(c.matches(), vec!["Build Project".to_string()]);
        c.set_query("BUILD");
        assert_eq!(c.matches(), vec!["Build Project".to_string()]);
    }

    #[test]
    fn matches_miss_returns_empty() {
        let mut c = open_two();
        c.set_query("zzz");
        assert!(c.matches().is_empty());
    }

    #[test]
    fn matches_empty_query_returns_selected() {
        let c = open_two();
        assert_eq!(c.matches(), vec!["Build Project".to_string()]);
    }

    #[test]
    fn matches_closed_menu_returns_empty() {
        let mut menu = FooterMenuFull::new();
        assert!(menu.add_item("Build Project"));
        let mut c = CommandFull::new(menu);
        c.set_query("build");
        assert!(c.matches().is_empty());
    }

    #[test]
    fn confirm_returns_selected_when_open() {
        let c = open_two();
        assert_eq!(c.confirm(), Some("Build Project".to_string()));
    }

    #[test]
    fn confirm_none_when_closed() {
        let mut menu = FooterMenuFull::new();
        assert!(menu.add_item("Build Project"));
        let c = CommandFull::new(menu);
        assert_eq!(c.confirm(), None);
    }
}
