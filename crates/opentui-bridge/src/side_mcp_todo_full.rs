#![forbid(unsafe_code)]
//! Sidebar MCP names + todo count (TS `sidebar/mcp.tsx`, `sidebar/todo.tsx`).
//! `ponytail:` no per-item status, add when caller needs it.

/// Sidebar MCP names plus todo count.
pub struct SideMcpTodo {
    mcp: Vec<String>,
    todos: u32,
}

impl SideMcpTodo {
    /// Empty state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            mcp: Vec::new(),
            todos: 0,
        }
    }

    /// Push MCP name (64-char cap); ignored past 16 entries.
    pub fn add_mcp(&mut self, name: &str) {
        if self.mcp.len() >= 16 {
            return;
        }
        self.mcp.push(name.chars().take(64).collect());
    }

    /// Increment todo count, saturating.
    pub fn bump_todo(&mut self) {
        self.todos = self.todos.saturating_add(1);
    }

    /// One-line summary, capped at 256 chars.
    #[must_use]
    pub fn summary(&self) -> String {
        let names = self.mcp.join(",");
        let s = format!("mcp:{} todos:{} {names}", self.mcp.len(), self.todos);
        s.chars().take(256).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_summary() {
        assert_eq!(SideMcpTodo::new().summary(), "mcp:0 todos:0 ");
    }

    #[test]
    fn add_and_summary() {
        let mut s = SideMcpTodo::new();
        s.add_mcp("server-a");
        s.bump_todo();
        assert_eq!(s.summary(), "mcp:1 todos:1 server-a");
    }

    #[test]
    fn mcp_capped_at_16() {
        let mut s = SideMcpTodo::new();
        for i in 0..20 {
            s.add_mcp(&format!("m{i}"));
        }
        assert_eq!(s.mcp.len(), 16);
    }

    #[test]
    fn name_capped_at_64_and_summary_at_256() {
        let mut s = SideMcpTodo::new();
        s.add_mcp(&"a".repeat(100));
        assert_eq!(s.mcp[0].chars().count(), 64);
        for _ in 0..16 {
            s.add_mcp(&"b".repeat(64));
        }
        assert!(s.summary().chars().count() <= 256);
    }

    #[test]
    fn bump_saturates() {
        let mut s = SideMcpTodo::new();
        s.todos = u32::MAX;
        s.bump_todo();
        assert_eq!(s.todos, u32::MAX);
    }
}
