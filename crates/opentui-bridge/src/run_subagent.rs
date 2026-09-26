//! Subagent body + snapshots (TS `run/footer.subagent.tsx` read-only ref).
//!
//! [`SubagentState`] tracks one spawned subagent: a capped id, a lifecycle
//! [`SubStatus`], and a capped transcript. [`spawn`] rejects empty ids,
//! [`SubagentState::append_line`] evicts the oldest line past the cap,
//! [`SubagentState::finish`] latches `Done`/`Failed`, and
//! [`SubagentState::snapshot`] clones the transcript.

#![forbid(unsafe_code)]

/// Maximum id length in bytes.
pub const SUBAGENT_ID_CAP: usize = 64;

/// Maximum transcript lines; oldest is evicted past the cap.
pub const TRANSCRIPT_CAP: usize = 500;

/// Subagent lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SubStatus {
    /// Created but not yet running.
    #[default]
    Spawned,
    /// Actively producing transcript lines.
    Running,
    /// Finished successfully.
    Done,
    /// Finished with failure.
    Failed,
}

/// One spawned subagent with a capped transcript.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SubagentState {
    id: String,
    status: SubStatus,
    transcript: Vec<String>,
}

/// Spawn a running subagent; errs on empty id or id past [`SUBAGENT_ID_CAP`].
pub fn spawn(id: impl Into<String>) -> Result<SubagentState, &'static str> {
    let id = id.into();
    if id.is_empty() {
        return Err("subagent id must not be empty");
    }
    if id.len() > SUBAGENT_ID_CAP {
        return Err("subagent id over 64 bytes");
    }
    Ok(SubagentState {
        id,
        status: SubStatus::Running,
        transcript: Vec::new(),
    })
}

impl SubagentState {
    /// Subagent id.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Current lifecycle status.
    #[must_use]
    pub fn status(&self) -> SubStatus {
        self.status
    }

    /// Transcript line count.
    #[must_use]
    pub fn len(&self) -> usize {
        self.transcript.len()
    }

    /// True when no transcript lines are stored.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.transcript.is_empty()
    }

    /// Append a line; evicts the oldest past [`TRANSCRIPT_CAP`].
    pub fn append_line(&mut self, line: String) {
        if self.transcript.len() >= TRANSCRIPT_CAP {
            self.transcript.remove(0);
        }
        self.transcript.push(line);
    }

    /// Latch terminal status: `Done` when `ok`, else `Failed`.
    pub fn finish(&mut self, ok: bool) {
        self.status = if ok {
            SubStatus::Done
        } else {
            SubStatus::Failed
        };
    }

    /// Clone the transcript snapshot.
    #[must_use]
    pub fn snapshot(&self) -> Vec<String> {
        self.transcript.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_sets_running_with_id() {
        let s = spawn("agent-1").unwrap();
        assert_eq!(s.id(), "agent-1");
        assert_eq!(s.status(), SubStatus::Running);
        assert!(s.is_empty());
    }

    #[test]
    fn spawn_empty_id_errs() {
        assert!(spawn("").is_err());
    }

    #[test]
    fn spawn_long_id_errs() {
        assert!(spawn("x".repeat(SUBAGENT_ID_CAP + 1)).is_err());
    }

    #[test]
    fn append_caps_at_500_evicting_oldest() {
        let mut s = spawn("a").unwrap();
        for i in 0..TRANSCRIPT_CAP + 5 {
            s.append_line(format!("line-{i}"));
        }
        assert_eq!(s.len(), TRANSCRIPT_CAP);
        let snap = s.snapshot();
        assert_eq!(snap[0], "line-5");
        assert_eq!(
            snap[TRANSCRIPT_CAP - 1],
            format!("line-{}", TRANSCRIPT_CAP + 4)
        );
    }

    #[test]
    fn finish_done_and_failed() {
        let mut ok = spawn("a").unwrap();
        ok.finish(true);
        assert_eq!(ok.status(), SubStatus::Done);
        let mut bad = spawn("b").unwrap();
        bad.finish(false);
        assert_eq!(bad.status(), SubStatus::Failed);
    }

    #[test]
    fn snapshot_clones_transcript() {
        let mut s = spawn("a").unwrap();
        s.append_line("one".into());
        let snap = s.snapshot();
        assert_eq!(snap, vec!["one".to_string()]);
        s.append_line("two".into());
        assert_eq!(snap.len(), 1);
        assert_eq!(s.len(), 2);
    }
}
