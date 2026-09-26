#![forbid(unsafe_code)]
//! MCP server toggle dialog (mirrors `packages/tui/src/component/dialog-mcp.tsx:21`
//! `DialogMcp` + `Status` (:10-19): sorted server list, per-server
//! enabled flag from `local.mcp.isEnabled(name)`, toggle action
//! `dialog.mcp.toggle`).
//!
//! Divergences: TS reads live `sync.data.mcp` map + async loading state;
//! Rust is a snapshot list. TS sorts/filters via remeda pipe; host sorts
//! before insert here.

/// Max servers in one dialog (fail-closed bound).
pub const MAX_SERVERS: usize = 32;
/// Max chars per server name (mirrors option `title`).
pub const MAX_NAME: usize = 64;
/// MCP server list with per-server enabled flag and cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpDialog {
    pub servers: Vec<String>,
    pub cursor: usize,
    pub enabled: Vec<bool>,
}

impl McpDialog {
    pub fn new(names: &[&str]) -> Self {
        let servers: Vec<String> = names
            .iter()
            .filter(|n| !n.is_empty())
            .take(MAX_SERVERS)
            .map(|n| n.chars().take(MAX_NAME).collect())
            .collect();
        let enabled = vec![false; servers.len()];
        Self {
            servers,
            cursor: 0,
            enabled,
        }
    }

    pub fn len(&self) -> usize {
        self.servers.len()
    }
    pub fn is_empty(&self) -> bool {
        self.servers.is_empty()
    }
    /// Flip enabled flag at `idx`; returns new state, false if OOB.
    pub fn toggle(&mut self, idx: usize) -> bool {
        if idx >= self.enabled.len() {
            return false;
        }
        self.enabled[idx] = !self.enabled[idx];
        self.enabled[idx]
    }

    /// Move cursor by signed delta, wrapping; noop when empty.
    pub fn move_cursor(&mut self, delta: isize) {
        if self.servers.is_empty() {
            self.cursor = 0;
            return;
        }
        let len = self.servers.len() as isize;
        self.cursor = (self.cursor as isize + delta).rem_euclid(len) as usize;
    }

    /// Names of enabled servers, in list order.
    pub fn enabled_list(&self) -> Vec<String> {
        self.servers
            .iter()
            .zip(self.enabled.iter())
            .filter(|(_, e)| **e)
            .map(|(s, _)| s.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_caps_servers_and_names() {
        let many = vec!["x"; 40];
        let d = McpDialog::new(&many);
        assert_eq!(d.len(), MAX_SERVERS);
        assert_eq!(d.enabled.len(), MAX_SERVERS);
        let long = "a".repeat(100);
        let d2 = McpDialog::new(&[long.as_str()]);
        assert_eq!(d2.servers[0].chars().count(), MAX_NAME);
    }

    #[test]
    fn toggle_flips_and_oob_false() {
        let mut d = McpDialog::new(&["a", "b"]);
        assert!(d.toggle(0));
        assert!(!d.toggle(0));
        assert!(!d.toggle(9));
    }

    #[test]
    fn move_cursor_wraps() {
        let mut d = McpDialog::new(&["a", "b", "c"]);
        d.move_cursor(1);
        assert_eq!(d.cursor, 1);
        d.move_cursor(-2);
        assert_eq!(d.cursor, 2);
        d.move_cursor(3);
        assert_eq!(d.cursor, 2);
    }

    #[test]
    fn move_cursor_empty_noop() {
        let mut d = McpDialog::new(&[]);
        d.move_cursor(5);
        assert_eq!(d.cursor, 0);
    }

    #[test]
    fn enabled_list_order() {
        let mut d = McpDialog::new(&["a", "b", "c"]);
        d.toggle(2);
        d.toggle(0);
        assert_eq!(d.enabled_list(), vec!["a".to_string(), "c".to_string()]);
    }

    #[test]
    fn new_skips_empty() {
        let d = McpDialog::new(&["", "a"]);
        assert_eq!(d.servers, vec!["a".to_string()]);
    }
}
