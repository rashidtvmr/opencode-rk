//! Subagent rows summary (TS `run/subagent-data.ts` read-only ref).
//!
//! [`SubagentRow`] is a flat row: capped id, capped status, line count.
//! [`summarize`] counts running/done/failed, [`filter_by_status`]
//! returns at most [`FILTER_CAP`] matches.

#![forbid(unsafe_code)]

/// Maximum id length in bytes.
pub const ROW_ID_CAP: usize = 64;

/// Maximum status length in bytes.
pub const ROW_STATUS_CAP: usize = 32;

/// Maximum rows returned by [`filter_by_status`].
pub const FILTER_CAP: usize = 500;

fn truncate(mut s: String, cap: usize) -> String {
    if s.len() > cap {
        let mut end = cap;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        s.truncate(end);
    }
    s
}

/// One subagent table row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubagentRow {
    id: String,
    status: String,
    lines: u32,
}

impl SubagentRow {
    /// Build a row, truncating id to 64 and status to 32 bytes.
    #[must_use]
    pub fn new(id: impl Into<String>, status: impl Into<String>, lines: u32) -> Self {
        Self {
            id: truncate(id.into(), ROW_ID_CAP),
            status: truncate(status.into(), ROW_STATUS_CAP),
            lines,
        }
    }

    /// Row id.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Row status.
    #[must_use]
    pub fn status(&self) -> &str {
        &self.status
    }

    /// Transcript line count.
    #[must_use]
    pub fn lines(&self) -> u32 {
        self.lines
    }
}

/// Summarize rows as "N subagents (R running, D done, F failed)".
#[must_use]
pub fn summarize(rows: &[SubagentRow]) -> String {
    let running = rows.iter().filter(|r| r.status == "running").count();
    let done = rows.iter().filter(|r| r.status == "done").count();
    let failed = rows.iter().filter(|r| r.status == "failed").count();
    format!(
        "{} subagents ({} running, {} done, {} failed)",
        rows.len(),
        running,
        done,
        failed
    )
}

/// Borrow rows matching `status`, capped at [`FILTER_CAP`].
#[must_use]
pub fn filter_by_status<'a>(rows: &'a [SubagentRow], status: &str) -> Vec<&'a SubagentRow> {
    rows.iter()
        .filter(|r| r.status == status)
        .take(FILTER_CAP)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_counts() {
        let rows = vec![
            SubagentRow::new("a", "running", 1),
            SubagentRow::new("b", "running", 2),
            SubagentRow::new("c", "done", 3),
            SubagentRow::new("d", "failed", 4),
        ];
        assert_eq!(
            summarize(&rows),
            "4 subagents (2 running, 1 done, 1 failed)"
        );
    }

    #[test]
    fn empty_zero() {
        assert_eq!(summarize(&[]), "0 subagents (0 running, 0 done, 0 failed)");
    }

    #[test]
    fn filter_matches() {
        let rows = vec![
            SubagentRow::new("a", "running", 1),
            SubagentRow::new("b", "done", 2),
            SubagentRow::new("c", "running", 3),
        ];
        let out = filter_by_status(&rows, "running");
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].id(), "a");
        assert_eq!(out[1].id(), "c");
    }

    #[test]
    fn filter_none() {
        let rows = vec![SubagentRow::new("a", "done", 1)];
        assert!(filter_by_status(&rows, "running").is_empty());
    }

    #[test]
    fn cap_respected() {
        let long_id = "x".repeat(ROW_ID_CAP + 10);
        let long_status = "y".repeat(ROW_STATUS_CAP + 10);
        let row = SubagentRow::new(long_id, long_status, 7);
        assert_eq!(row.id().len(), ROW_ID_CAP);
        assert_eq!(row.status().len(), ROW_STATUS_CAP);
        assert_eq!(row.lines(), 7);
        let rows: Vec<SubagentRow> = (0..FILTER_CAP + 50)
            .map(|i| SubagentRow::new(format!("id-{i}"), "running", i as u32))
            .collect();
        assert_eq!(filter_by_status(&rows, "running").len(), FILTER_CAP);
    }
}
