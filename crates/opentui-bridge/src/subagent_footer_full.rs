//! Full flow over [`SubagentWire`] (BRIDGE-PAR-269).
#![forbid(unsafe_code)]

use crate::subagent_wire::SubagentWire;

/// Max chars for [`SubFooterFlow::summary_capped`].
pub const SUMMARY_CAP: usize = 256;

/// Flow wrapper owning a [`SubagentWire`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubFooterFlow {
    pub wire: SubagentWire,
}

impl SubFooterFlow {
    /// New flow at 0% progress, footer running.
    #[must_use]
    pub fn new(id: &str, task: &str) -> Self {
        Self {
            wire: SubagentWire::new(id, task),
        }
    }

    /// Buffer an output line; false when empty.
    pub fn push(&mut self, line: &str) -> bool {
        self.wire.push_output(line)
    }

    /// Delegate to [`SubagentWire::status`].
    #[must_use]
    pub fn status(&self) -> String {
        self.wire.status()
    }

    /// Footer summary capped at [`SUMMARY_CAP`] chars.
    #[must_use]
    pub fn summary_capped(&self) -> String {
        let s = self.wire.footer.summary();
        if s.chars().count() <= SUMMARY_CAP {
            return s;
        }
        s.chars().take(SUMMARY_CAP).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_buffers_and_rejects_empty() {
        let mut f = SubFooterFlow::new("a", "t");
        assert!(f.push("hi"));
        assert!(!f.push(""));
        assert_eq!(f.wire.footer.lines, vec!["hi"]);
    }

    #[test]
    fn status_delegates_to_wire() {
        let mut f = SubFooterFlow::new("a1", "do work");
        f.push("hi");
        assert_eq!(f.status(), f.wire.status());
        assert_eq!(f.status(), "a1 0% do work | agent a1 running 1 lines");
    }

    #[test]
    fn summary_short_passthrough() {
        let f = SubFooterFlow::new("a", "t");
        assert_eq!(f.summary_capped(), "agent a running 0 lines");
    }

    #[test]
    fn summary_capped_at_256() {
        let f = SubFooterFlow::new(&"i".repeat(64), "t");
        assert!(f.summary_capped().chars().count() <= SUMMARY_CAP);
    }
}
