#![forbid(unsafe_code)]
//! Live tool-call table for `opencode run` output (mirrors
//! `packages/opencode/src/cli/cmd/run/tool.ts` run/scroll phases:
//! a started tool is `Running`, a completed final is `Done`, an
//! error final is `Failed`).

/// Max rows held by [`ToolTable`].
pub const MAX_ROWS: usize = 64;
/// Max chars of [`ToolRow::name`].
pub const MAX_NAME_LEN: usize = 64;

/// Lifecycle of one tool call row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolRowState {
    Running,
    Done,
    Failed,
}

/// One tool call row: name (capped) + state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolRow {
    pub name: String,
    pub state: ToolRowState,
}

impl ToolRow {
    #[must_use]
    pub fn new(name: &str, state: ToolRowState) -> Self {
        Self {
            name: truncate(name),
            state,
        }
    }
}

fn truncate(s: &str) -> String {
    if s.chars().count() > MAX_NAME_LEN {
        s.chars().take(MAX_NAME_LEN).collect()
    } else {
        s.to_string()
    }
}

/// Ordered live rows, capped at [`MAX_ROWS`].
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ToolTable {
    pub rows: Vec<ToolRow>,
}

impl ToolTable {
    #[must_use]
    pub fn new() -> Self {
        Self { rows: Vec::new() }
    }

    /// Insert or update `name`. Returns false only when full and `name`
    /// is absent.
    pub fn upsert(&mut self, name: &str, state: ToolRowState) -> bool {
        let key = truncate(name);
        if let Some(row) = self.rows.iter_mut().find(|r| r.name == key) {
            row.state = state;
            return true;
        }
        if self.rows.len() >= MAX_ROWS {
            return false;
        }
        self.rows.push(ToolRow { name: key, state });
        true
    }

    /// Update state of an existing `name`. False when missing.
    pub fn set_state(&mut self, name: &str, state: ToolRowState) -> bool {
        let key = truncate(name);
        match self.rows.iter_mut().find(|r| r.name == key) {
            Some(row) => {
                row.state = state;
                true
            }
            None => false,
        }
    }

    /// `(running, done, failed)` counts.
    #[must_use]
    pub fn counts(&self) -> (u32, u32, u32) {
        let mut out = (0u32, 0u32, 0u32);
        for row in &self.rows {
            match row.state {
                ToolRowState::Running => out.0 += 1,
                ToolRowState::Done => out.1 += 1,
                ToolRowState::Failed => out.2 += 1,
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upsert_adds_row() {
        let mut t = ToolTable::new();
        assert!(t.upsert("bash", ToolRowState::Running));
        assert_eq!(t.rows.len(), 1);
        assert_eq!(t.rows[0], ToolRow::new("bash", ToolRowState::Running));
    }

    #[test]
    fn upsert_updates_existing() {
        let mut t = ToolTable::new();
        t.upsert("read", ToolRowState::Running);
        assert!(t.upsert("read", ToolRowState::Done));
        assert_eq!(t.rows.len(), 1);
        assert_eq!(t.rows[0].state, ToolRowState::Done);
    }

    #[test]
    fn set_state_missing_is_false() {
        let mut t = ToolTable::new();
        assert!(!t.set_state("ghost", ToolRowState::Done));
        t.upsert("edit", ToolRowState::Running);
        assert!(t.set_state("edit", ToolRowState::Failed));
        assert!(!t.set_state("ghost", ToolRowState::Done));
    }

    #[test]
    fn counts_split_states() {
        let mut t = ToolTable::new();
        t.upsert("a", ToolRowState::Running);
        t.upsert("b", ToolRowState::Done);
        t.upsert("c", ToolRowState::Failed);
        t.upsert("d", ToolRowState::Done);
        assert_eq!(t.counts(), (1, 2, 1));
    }

    #[test]
    fn caps_rows_and_name_len() {
        let mut t = ToolTable::new();
        for i in 0..MAX_ROWS {
            assert!(t.upsert(&format!("tool-{i}"), ToolRowState::Running));
        }
        assert!(!t.upsert("overflow", ToolRowState::Running));
        assert!(t.upsert("tool-0", ToolRowState::Done));
        let long = "n".repeat(200);
        let row = ToolRow::new(&long, ToolRowState::Running);
        assert_eq!(row.name.chars().count(), MAX_NAME_LEN);
    }

    #[test]
    fn state_transitions() {
        let mut t = ToolTable::new();
        t.upsert("task", ToolRowState::Running);
        assert!(t.set_state("task", ToolRowState::Done));
        assert!(t.set_state("task", ToolRowState::Failed));
        assert!(t.set_state("task", ToolRowState::Running));
        assert_eq!(t.rows[0].state, ToolRowState::Running);
        assert_eq!(t.counts(), (1, 0, 0));
    }
}
