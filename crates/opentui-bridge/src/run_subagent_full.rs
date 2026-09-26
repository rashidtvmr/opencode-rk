#![forbid(unsafe_code)]
//! Subagent pool over [`SubagentWire`] (TS `run/footer.subagent.tsx` rows ref).
//!
//! [`SubagentPool`] holds at most [`POOL_CAP`] wires; [`SubagentPool::spawn`]
//! rejects empty ids and a full pool, [`SubagentPool::feed`] buffers one line,
//! [`SubagentPool::statuses`] renders one capped status per wire.

use crate::subagent_wire::SubagentWire;

/// Max wires in one pool.
pub const POOL_CAP: usize = 8;

/// Bounded pool of subagent wires.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SubagentPool {
    pub items: Vec<SubagentWire>,
}

impl SubagentPool {
    /// Empty pool.
    #[must_use]
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Push a wire; false when id empty or pool holds [`POOL_CAP`].
    pub fn spawn(&mut self, id: &str, task: &str) -> bool {
        if id.is_empty() || self.items.len() >= POOL_CAP {
            return false;
        }
        self.items.push(SubagentWire::new(id, task));
        true
    }

    /// Buffer a line into wire `idx`; false when OOB or empty.
    pub fn feed(&mut self, idx: usize, line: &str) -> bool {
        match self.items.get_mut(idx) {
            Some(w) => w.push_output(line),
            None => false,
        }
    }

    /// One [`SubagentWire::status`] per wire (each capped at 512 chars).
    #[must_use]
    pub fn statuses(&self) -> Vec<String> {
        self.items.iter().map(SubagentWire::status).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_is_empty() {
        assert_eq!(SubagentPool::new().items.len(), 0);
    }

    #[test]
    fn spawn_fills_to_cap_then_rejects() {
        let mut p = SubagentPool::new();
        for i in 0..POOL_CAP {
            assert!(p.spawn(&format!("a{i}"), "t"));
        }
        assert_eq!(p.items.len(), POOL_CAP);
        assert!(!p.spawn("extra", "t"));
    }

    #[test]
    fn spawn_empty_id_rejected() {
        let mut p = SubagentPool::new();
        assert!(!p.spawn("", "t"));
        assert!(p.items.is_empty());
    }

    #[test]
    fn feed_buffers_line() {
        let mut p = SubagentPool::new();
        assert!(p.spawn("a1", "work"));
        assert!(p.feed(0, "hi"));
        assert_eq!(p.items[0].footer.lines, vec!["hi"]);
    }

    #[test]
    fn feed_oob_or_empty_false() {
        let mut p = SubagentPool::new();
        assert!(!p.feed(0, "hi"));
        assert!(p.spawn("a", "t"));
        assert!(!p.feed(0, ""));
        assert!(!p.feed(3, "hi"));
    }

    #[test]
    fn statuses_capped_512() {
        let mut p = SubagentPool::new();
        assert!(p.spawn(&"i".repeat(64), &"t".repeat(512)));
        let s = p.statuses();
        assert_eq!(s.len(), 1);
        assert!(s[0].chars().count() <= 512);
    }
}
