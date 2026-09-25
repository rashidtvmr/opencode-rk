#![forbid(unsafe_code)]
//! Agent picker state (mirrors `packages/tui/src/component/dialog-agent.tsx:6`
//! `DialogAgent` + `ui/dialog-select.tsx` select list: agent names from
//! `local.agent.list()`, current from `local.agent.current()?.name`, pick via
//! `local.agent.set(value)` then `dialog.clear()`).

/// Max agents shown (fail-closed bound).
pub const MAX_AGENTS: usize = 32;
/// Max chars per agent name.
pub const MAX_NAME: usize = 64;

/// Agent picker: name list + cursor. Host maps names to set/current.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AgentDialog {
    pub agents: Vec<String>,
    pub cursor: usize,
}

impl AgentDialog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert name; false when full, empty, too long, or duplicate.
    pub fn push(&mut self, name: &str) -> bool {
        if name.is_empty() || name.chars().count() > MAX_NAME {
            return false;
        }
        if self.agents.len() >= MAX_AGENTS || self.agents.iter().any(|a| a == name) {
            return false;
        }
        self.agents.push(name.to_string());
        true
    }

    /// Move cursor by delta, clamped to list bounds.
    pub fn move_cursor(&mut self, delta: isize) {
        if self.agents.is_empty() {
            self.cursor = 0;
            return;
        }
        let next = self.cursor as isize + delta;
        self.cursor = next.clamp(0, self.agents.len() as isize - 1) as usize;
    }

    /// Cursor selection (TS `options()[selected]` picked value).
    pub fn selected(&self) -> Option<&str> {
        self.agents.get(self.cursor).map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_accepts_and_selects_first() {
        let mut d = AgentDialog::new();
        assert!(d.push("plan"));
        assert!(d.push("build"));
        assert_eq!(d.selected(), Some("plan"));
    }

    #[test]
    fn push_rejects_bad_names() {
        let mut d = AgentDialog::new();
        assert!(!d.push(""));
        assert!(!d.push(&"x".repeat(65)));
        assert!(d.push("plan"));
        assert!(!d.push("plan"));
    }

    #[test]
    fn push_caps_at_32() {
        let mut d = AgentDialog::new();
        for i in 0..MAX_AGENTS {
            assert!(d.push(&format!("a{i}")));
        }
        assert!(!d.push("extra"));
        assert_eq!(d.agents.len(), 32);
    }

    #[test]
    fn cursor_clamps_both_ends() {
        let mut d = AgentDialog::new();
        d.move_cursor(5);
        assert_eq!(d.cursor, 0);
        d.push("a");
        d.push("b");
        d.move_cursor(99);
        assert_eq!(d.selected(), Some("b"));
        d.move_cursor(-99);
        assert_eq!(d.selected(), Some("a"));
    }

    #[test]
    fn cursor_steps_signed() {
        let mut d = AgentDialog::new();
        d.push("a");
        d.push("b");
        d.push("c");
        d.move_cursor(2);
        assert_eq!(d.selected(), Some("c"));
        d.move_cursor(-1);
        assert_eq!(d.selected(), Some("b"));
    }

    #[test]
    fn selected_none_when_empty() {
        let d = AgentDialog::new();
        assert_eq!(d.selected(), None);
    }
}
