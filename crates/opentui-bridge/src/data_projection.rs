#![forbid(unsafe_code)]
//! Data projection rows (std-only).
//!
//! Evidence (TS `packages/tui/src/context/data.tsx`):
//! - `Data{session:{info,message,permission,question},project:{permission},location}`
//! - `session.message` keyed `Record<sessionID, SessionMessage[]>`; event
//!   `switch (event.type)` projects cases (`session.next.*`) into message rows.
//! - Projection here flattens one (kind,id,label) triple into a bounded row.

/// Max id chars.
pub const MAX_ID: usize = 64;
/// Max label chars.
pub const MAX_LABEL: usize = 256;
/// Max rows returned by [`filter_kind`].
pub const MAX_ROWS: usize = 1000;

/// Row kinds (`data.tsx` session/message + part/todo/config surfaces).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataKind {
    Session,
    Message,
    Part,
    Todo,
    Config,
}

impl DataKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Session => "session",
            Self::Message => "message",
            Self::Part => "part",
            Self::Todo => "todo",
            Self::Config => "config",
        }
    }
}

/// One projected row with bounded fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataRow {
    pub kind: DataKind,
    pub id: String,
    pub label: String,
}

fn trunc(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

/// Project one triple into a bounded row (truncates id/label).
#[must_use]
pub fn project(kind: DataKind, id: &str, label: &str) -> DataRow {
    DataRow {
        kind,
        id: trunc(id, MAX_ID),
        label: trunc(label, MAX_LABEL),
    }
}

/// Stable key `"kind:id"`.
#[must_use]
pub fn row_key(row: &DataRow) -> String {
    format!("{}:{}", row.kind.as_str(), row.id)
}

/// Rows of one kind, capped at [`MAX_ROWS`].
#[must_use]
pub fn filter_kind(rows: &[DataRow], kind: DataKind) -> Vec<DataRow> {
    rows.iter()
        .filter(|r| r.kind == kind)
        .take(MAX_ROWS)
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_truncates_id() {
        let row = project(DataKind::Session, &"x".repeat(100), "ok");
        assert_eq!(row.id.len(), MAX_ID);
    }

    #[test]
    fn project_truncates_label() {
        let row = project(DataKind::Message, "id", &"y".repeat(300));
        assert_eq!(row.label.chars().count(), MAX_LABEL);
    }

    #[test]
    fn project_passthrough() {
        let row = project(DataKind::Part, "a", "b");
        assert_eq!((row.id.as_str(), row.label.as_str()), ("a", "b"));
    }

    #[test]
    fn key_format() {
        let row = project(DataKind::Todo, "7", "do");
        assert_eq!(row_key(&row), "todo:7");
    }

    #[test]
    fn filter_matches_only_kind() {
        let rows = vec![
            project(DataKind::Session, "a", ""),
            project(DataKind::Message, "b", ""),
        ];
        let out = filter_kind(&rows, DataKind::Session);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].id, "a");
    }

    #[test]
    fn filter_none_empty() {
        let rows = vec![project(DataKind::Session, "a", "")];
        assert!(filter_kind(&rows, DataKind::Config).is_empty());
    }

    #[test]
    fn filter_caps_rows() {
        let rows: Vec<DataRow> = (0..MAX_ROWS + 10)
            .map(|i| project(DataKind::Config, &format!("c{i}"), ""))
            .collect();
        assert_eq!(filter_kind(&rows, DataKind::Config).len(), MAX_ROWS);
    }
}
