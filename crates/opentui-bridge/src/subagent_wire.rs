#![forbid(unsafe_code)]
//! Subagent data+footer wire (BRIDGE-PAR-188).
//!
//! Thin adapter over [`SubagentDataFull`] (id/task/progress) and
//! [`SubagentFooter`] (bounded output lines + status). `push_output` and
//! `set_progress` delegate; [`SubagentWire::status`] renders
//! `"label | summary"` capped at [`STATUS_CAP`] chars.

use crate::subagent_data_full::SubagentDataFull;
use crate::subagent_footer::SubagentFooter;

/// Max chars for [`SubagentWire::status`].
pub const STATUS_CAP: usize = 512;

/// Wire wrapping subagent row + footer buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubagentWire {
    pub data: SubagentDataFull,
    pub footer: SubagentFooter,
}

impl SubagentWire {
    /// New wire at 0% progress, footer running.
    #[must_use]
    pub fn new(id: &str, task: &str) -> Self {
        Self {
            data: SubagentDataFull::new(id, task, 0),
            footer: SubagentFooter::new(id),
        }
    }

    /// Buffer an output line; false when empty.
    pub fn push_output(&mut self, line: &str) -> bool {
        self.footer.push_line(line)
    }

    /// Set progress 0..=100; false when >100.
    pub fn set_progress(&mut self, v: u8) -> bool {
        self.data.set_progress(v)
    }

    /// `"label | summary"` capped at [`STATUS_CAP`] chars.
    #[must_use]
    pub fn status(&self) -> String {
        let s = format!("{} | {}", self.data.label(), self.footer.summary());
        if s.chars().count() <= STATUS_CAP {
            return s;
        }
        s.chars().take(STATUS_CAP).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_wires_ids() {
        let w = SubagentWire::new("a1", "do work");
        assert_eq!(w.data.id(), "a1");
        assert_eq!(w.data.progress(), 0);
        assert_eq!(w.footer.agent_id, "a1");
    }

    #[test]
    fn push_output_buffers() {
        let mut w = SubagentWire::new("a", "t");
        assert!(w.push_output("hi"));
        assert!(!w.push_output(""));
        assert_eq!(w.footer.lines, vec!["hi"]);
    }

    #[test]
    fn set_progress_bounds() {
        let mut w = SubagentWire::new("a", "t");
        assert!(w.set_progress(50));
        assert_eq!(w.data.progress(), 50);
        assert!(!w.set_progress(101));
        assert_eq!(w.data.progress(), 50);
    }

    #[test]
    fn status_joins_label_summary() {
        let mut w = SubagentWire::new("a1", "do work");
        w.push_output("hi");
        assert_eq!(w.status(), "a1 0% do work | agent a1 running 1 lines");
    }

    #[test]
    fn status_caps_512() {
        let w = SubagentWire::new(&"i".repeat(64), &"t".repeat(512));
        assert!(w.status().chars().count() <= STATUS_CAP);
    }

    #[test]
    fn status_reflects_progress() {
        let mut w = SubagentWire::new("a", "t");
        w.set_progress(99);
        assert!(w.status().starts_with("a 99% t | agent a running"));
    }
}
