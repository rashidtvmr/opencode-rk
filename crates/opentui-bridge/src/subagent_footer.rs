#![forbid(unsafe_code)]
//! Subagent footer state (port of `packages/tui/src/routes/session/subagent-footer.tsx`).
//!
//! TS renders label + sibling position + usage/cost + Parent/Prev/Next
//! shortcuts; renderer-owned here. This keeps the bounded line buffer and
//! running/done/failed status behind it.

/// Max agent id chars.
pub const MAX_AGENT_ID: usize = 64;
/// Max buffered lines.
pub const MAX_LINES: usize = 100;
/// Max chars per line.
pub const MAX_LINE_LEN: usize = 512;

/// Footer status mirroring subagent lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SubFooterStatus {
    /// Subagent still running.
    #[default]
    Running,
    /// Finished successfully.
    Done,
    /// Finished with failure.
    Failed,
}

impl core::fmt::Display for SubFooterStatus {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Running => write!(f, "running"),
            Self::Done => write!(f, "done"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

fn trunc(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    s.chars().take(max).collect()
}

/// Bounded footer buffer for one subagent session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubagentFooter {
    /// Agent/session id, truncated to [`MAX_AGENT_ID`] chars.
    pub agent_id: String,
    /// Buffered lines, at most [`MAX_LINES`], each [`MAX_LINE_LEN`] chars.
    pub lines: Vec<String>,
    /// Lifecycle status.
    pub status: SubFooterStatus,
}

impl SubagentFooter {
    /// New running footer; id truncated to [`MAX_AGENT_ID`] chars.
    #[must_use]
    pub fn new(agent_id: &str) -> Self {
        Self {
            agent_id: trunc(agent_id, MAX_AGENT_ID),
            lines: Vec::new(),
            status: SubFooterStatus::Running,
        }
    }

    /// Push a line; drops oldest when full. Returns false when empty.
    pub fn push_line(&mut self, line: &str) -> bool {
        if line.is_empty() {
            return false;
        }
        if self.lines.len() >= MAX_LINES {
            self.lines.remove(0);
        }
        self.lines.push(trunc(line, MAX_LINE_LEN));
        true
    }

    /// Mark done (`ok=true`) or failed.
    pub fn finish(&mut self, ok: bool) {
        self.status = if ok {
            SubFooterStatus::Done
        } else {
            SubFooterStatus::Failed
        };
    }

    /// `"agent <id> <status> <N> lines"`.
    #[must_use]
    pub fn summary(&self) -> String {
        format!(
            "agent {} {} {} lines",
            self.agent_id,
            self.status,
            self.lines.len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_cap_evicts_oldest() {
        let mut f = SubagentFooter::new("a");
        for i in 0..MAX_LINES + 5 {
            assert!(f.push_line(&format!("l{i}")));
        }
        assert_eq!(f.lines.len(), MAX_LINES);
        assert_eq!(f.lines[0], format!("l{}", 5));
    }

    #[test]
    fn line_truncated_to_cap() {
        let mut f = SubagentFooter::new("a");
        assert!(f.push_line(&"x".repeat(MAX_LINE_LEN + 10)));
        assert_eq!(f.lines[0].chars().count(), MAX_LINE_LEN);
    }

    #[test]
    fn finish_ok_and_failed() {
        let mut ok = SubagentFooter::new("a");
        ok.finish(true);
        assert_eq!(ok.status, SubFooterStatus::Done);
        let mut bad = SubagentFooter::new("a");
        bad.finish(false);
        assert_eq!(bad.status, SubFooterStatus::Failed);
    }

    #[test]
    fn summary_format() {
        let mut f = SubagentFooter::new("agent1");
        f.push_line("hi");
        f.push_line("yo");
        f.finish(true);
        assert_eq!(f.summary(), "agent agent1 done 2 lines");
    }

    #[test]
    fn trunc_id_and_empty_lines() {
        let f = SubagentFooter::new(&"i".repeat(MAX_AGENT_ID + 8));
        assert_eq!(f.agent_id.chars().count(), MAX_AGENT_ID);
        let mut e = SubagentFooter::new("a");
        assert!(!e.push_line(""));
        assert_eq!(e.summary(), "agent a running 0 lines");
    }
}
