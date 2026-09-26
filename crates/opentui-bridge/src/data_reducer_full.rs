#![forbid(unsafe_code)]
//! Typed stores over TUI data.tsx:27-569 (22586B): Session/Message/Location
//! share one 30-case reducer; refresh gates on stale; locationKey=cwd:branch.
//! Single-row projection vs typed stores: kind selects row shape, reducer
//! case count fixed at REDUCER_CASES.

pub const REDUCER_CASES: usize = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataKind {
    Session,
    Message,
    Location,
}

#[must_use]
pub fn kind_name(kind: DataKind) -> &'static str {
    match kind {
        DataKind::Session => "session",
        DataKind::Message => "message",
        DataKind::Location => "location",
    }
}

#[must_use]
pub fn refresh_needed(stale: bool) -> bool {
    stale
}

#[must_use]
pub fn location_key(cwd: &str, branch: &str) -> String {
    if branch.is_empty() {
        cwd.to_string()
    } else {
        format!("{cwd}:{branch}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cases_is_thirty() {
        assert_eq!(REDUCER_CASES, 30);
    }

    #[test]
    fn names_cover_kinds() {
        assert_eq!(kind_name(DataKind::Session), "session");
        assert_eq!(kind_name(DataKind::Message), "message");
        assert_eq!(kind_name(DataKind::Location), "location");
    }

    #[test]
    fn refresh_and_key() {
        assert!(refresh_needed(true));
        assert!(!refresh_needed(false));
        assert_eq!(location_key("/r", "main"), "/r:main");
        assert_eq!(location_key("/r", ""), "/r");
    }
}
