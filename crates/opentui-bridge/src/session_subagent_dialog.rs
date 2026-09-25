//! Subagent dialog state (TS `dialog-subagent.tsx` read-only ref).
//!
//! [`SubagentDialog`] tracks one open subagent dialog: a capped agent id,
//! a capped task, and an [`SubagentDialog::open`] flag. [`open`] rejects an
//! empty id and truncates over-cap inputs, [`SubagentDialog::close`]
//! marks it closed, and [`SubagentDialog::summary`] renders `id: task`
//! only while open.

#![forbid(unsafe_code)]

/// Maximum agent id length in bytes.
pub const AGENT_ID_CAP: usize = 64;

/// Maximum task length in bytes (1 KiB).
pub const TASK_CAP: usize = 1024;

/// One subagent dialog with capped fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubagentDialog {
    pub agent_id: String,
    pub task: String,
    pub open: bool,
}

/// Open a dialog; errs on empty id, truncates over-cap inputs.
pub fn open(
    agent_id: impl Into<String>,
    task: impl Into<String>,
) -> Result<SubagentDialog, String> {
    let agent_id = agent_id.into();
    let task = task.into();
    if agent_id.is_empty() {
        return Err("agent id must not be empty".to_string());
    }
    Ok(SubagentDialog {
        agent_id: truncate(&agent_id, AGENT_ID_CAP),
        task: truncate(&task, TASK_CAP),
        open: true,
    })
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
    /// Mark the dialog closed.
    pub fn close(&mut self) {
        self.open = false;
    }

    /// `id: task` while open, else `None`.
    #[must_use]
    pub fn summary(&self) -> Option<String> {
        if self.open {
            Some(format!("{}: {}", self.agent_id, self.task))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_empty_id_errs() {
        assert!(open("", "task").is_err());
    }

    #[test]
    fn open_truncates_long_id() {
        let d = open("x".repeat(100), "t").unwrap();
        assert_eq!(d.agent_id.len(), AGENT_ID_CAP);
        assert!(d.open);
    }

    #[test]
    fn open_truncates_long_task() {
        let d = open("a", "y".repeat(TASK_CAP + 10)).unwrap();
        assert_eq!(d.task.len(), TASK_CAP);
    }

    #[test]
    fn close_marks_closed() {
        let mut d = open("a", "t").unwrap();
        d.close();
        assert!(!d.open);
    }

    #[test]
    fn summary_some_when_open() {
        let d = open("a", "t").unwrap();
        assert_eq!(d.summary(), Some("a: t".to_string()));
    }

    #[test]
    fn summary_none_when_closed() {
        let mut d = open("a", "t").unwrap();
        d.close();
        assert_eq!(d.summary(), None);
    }
}
