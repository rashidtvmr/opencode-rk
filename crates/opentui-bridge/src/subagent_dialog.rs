//! Subagent picker list (TS `dialog-subagent.tsx` read-only ref).
//!
//! [`SubagentDialog`] is a capped list of agent ids with an optional
//! active index. [`SubagentDialog::add`] rejects empty ids, truncates
//! over-cap ids, and rejects when full. [`SubagentDialog::select`]
//! activates an index, [`SubagentDialog::active_id`] reads it, and
//! [`SubagentDialog::remove`] shifts the active index down.

#![forbid(unsafe_code)]

/// Maximum agents held.
pub const AGENT_CAP: usize = 32;

/// Maximum agent id length in bytes.
pub const AGENT_ID_CAP: usize = 64;

/// Capped agent id list with optional active index.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SubagentDialog {
    pub agents: Vec<String>,
    pub active: Option<usize>,
}

fn truncate(s: &str, cap: usize) -> String {
    if s.len() <= cap {
        return s.to_string();
    }
    let mut end = cap;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

impl SubagentDialog {
    /// Push id; false on empty id or when full. Dups allowed.
    pub fn add(&mut self, id: &str) -> bool {
        if id.is_empty() || self.agents.len() >= AGENT_CAP {
            return false;
        }
        self.agents.push(truncate(id, AGENT_ID_CAP));
        true
    }

    /// Activate index; false when out of bounds.
    pub fn select(&mut self, idx: usize) -> bool {
        if idx < self.agents.len() {
            self.active = Some(idx);
            true
        } else {
            false
        }
    }

    /// Active agent id, if any.
    #[must_use]
    pub fn active_id(&self) -> Option<&str> {
        self.active
            .and_then(|i| self.agents.get(i).map(String::as_str))
    }

    /// Remove index, shifting active down; false when out of bounds.
    pub fn remove(&mut self, idx: usize) -> bool {
        if idx >= self.agents.len() {
            return false;
        }
        self.agents.remove(idx);
        self.active = match self.active {
            None => None,
            Some(a) if a == idx => None,
            Some(a) if a > idx => Some(a - 1),
            Some(a) => Some(a),
        };
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_then_select_active_id() {
        let mut d = SubagentDialog::default();
        assert!(d.add("a"));
        assert!(d.add("b"));
        assert!(d.select(1));
        assert_eq!(d.active_id(), Some("b"));
    }

    #[test]
    fn select_out_of_bounds_false() {
        let mut d = SubagentDialog::default();
        assert!(d.add("a"));
        assert!(!d.select(5));
        assert_eq!(d.active, None);
    }

    #[test]
    fn remove_shifts_active_down() {
        let mut d = SubagentDialog::default();
        d.add("a");
        d.add("b");
        d.add("c");
        d.select(2);
        assert!(d.remove(0));
        assert_eq!(d.active, Some(1));
        assert_eq!(d.active_id(), Some("c"));
    }

    #[test]
    fn remove_active_clears() {
        let mut d = SubagentDialog::default();
        d.add("a");
        d.select(0);
        assert!(d.remove(0));
        assert_eq!(d.active_id(), None);
    }

    #[test]
    fn dup_ids_allowed() {
        let mut d = SubagentDialog::default();
        assert!(d.add("a"));
        assert!(d.add("a"));
        assert_eq!(d.agents.len(), 2);
    }

    #[test]
    fn cap_and_empty_rejected() {
        let mut d = SubagentDialog::default();
        assert!(!d.add(""));
        for i in 0..AGENT_CAP {
            assert!(d.add(&format!("agent-{i}")));
        }
        assert!(!d.add("overflow"));
        assert_eq!(d.agents.len(), AGENT_CAP);
    }

    #[test]
    fn long_id_truncated() {
        let mut d = SubagentDialog::default();
        assert!(d.add(&"x".repeat(100)));
        assert_eq!(d.agents[0].len(), AGENT_ID_CAP);
    }
}
