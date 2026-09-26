#![forbid(unsafe_code)]
//! Permission kind registry for tool authorization prompts.
//!
//! Extends `crate::run_permission::Stage`: default-allow kinds start at
//! `Stage::Always`, default-deny kinds start at `Stage::Ask`.

use crate::run_permission::Stage;

/// Tool permission kinds recognized by the bridge.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum PermissionKind {
    Bash,
    Edit,
    Write,
    Read,
    WebFetch,
    WebSearch,
    Grep,
    Glob,
    Task,
    TodoWrite,
    Mcp,
    External,
}

impl PermissionKind {
    /// All 12 kinds in canonical order.
    pub fn all() -> [PermissionKind; 12] {
        use PermissionKind::*;
        [
            Bash, Edit, Write, Read, WebFetch, WebSearch, Grep, Glob, Task, TodoWrite, Mcp,
            External,
        ]
    }
}

/// (label, description, default_allow). Read/Glob/Grep allow; rest deny.
pub fn info(kind: PermissionKind) -> (&'static str, &'static str, bool) {
    use PermissionKind::*;
    match kind {
        Bash => ("bash", "Run shell commands", false),
        Edit => ("edit", "Edit existing files", false),
        Write => ("write", "Create new files", false),
        Read => ("read", "Read file contents", true),
        WebFetch => ("webfetch", "Fetch URL content", false),
        WebSearch => ("websearch", "Search the web", false),
        Grep => ("grep", "Search file contents", true),
        Glob => ("glob", "Match file paths", true),
        Task => ("task", "Spawn subagent tasks", false),
        TodoWrite => ("todowrite", "Update task todo list", false),
        Mcp => ("mcp", "Call MCP server tools", false),
        External => ("external", "Call external plugin tools", false),
    }
}

/// Parse a tool/permission name (case-insensitive, `-`/`_` tolerant).
pub fn parse(name: &str) -> Option<PermissionKind> {
    use PermissionKind::*;
    let norm: Vec<u8> = name
        .bytes()
        .filter(|b| *b != b'-' && *b != b'_' && *b != b' ')
        .map(|b| b.to_ascii_lowercase())
        .collect();
    match norm.as_slice() {
        b"bash" => Some(Bash),
        b"edit" => Some(Edit),
        b"write" => Some(Write),
        b"read" => Some(Read),
        b"webfetch" => Some(WebFetch),
        b"websearch" => Some(WebSearch),
        b"grep" => Some(Grep),
        b"glob" => Some(Glob),
        b"task" => Some(Task),
        b"todowrite" => Some(TodoWrite),
        b"mcp" => Some(Mcp),
        b"external" => Some(External),
        _ => None,
    }
}

/// Initial prompt stage: default-allow kinds bypass (`Always`), rest `Ask`.
pub fn initial_stage(kind: PermissionKind) -> Stage {
    if info(kind).2 {
        Stage::Always
    } else {
        Stage::Ask
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn twelve_infos_non_empty() {
        let all = PermissionKind::all();
        assert_eq!(all.len(), 12);
        for k in all {
            let (label, desc, _) = info(k);
            assert!(!label.is_empty(), "{k:?} label empty");
            assert!(!desc.is_empty(), "{k:?} desc empty");
        }
    }

    #[test]
    fn read_default_allow() {
        assert_eq!(
            info(PermissionKind::Read),
            ("read", "Read file contents", true)
        );
        assert_eq!(initial_stage(PermissionKind::Read), Stage::Always);
    }

    #[test]
    fn glob_grep_default_allow() {
        assert!(info(PermissionKind::Glob).2);
        assert!(info(PermissionKind::Grep).2);
        assert_eq!(initial_stage(PermissionKind::Glob), Stage::Always);
    }

    #[test]
    fn bash_default_deny() {
        assert!(!info(PermissionKind::Bash).2);
        assert_eq!(initial_stage(PermissionKind::Bash), Stage::Ask);
    }

    #[test]
    fn parse_known_names() {
        assert_eq!(parse("bash"), Some(PermissionKind::Bash));
        assert_eq!(parse("WebFetch"), Some(PermissionKind::WebFetch));
        assert_eq!(parse("todo_write"), Some(PermissionKind::TodoWrite));
        assert_eq!(parse("todo-write"), Some(PermissionKind::TodoWrite));
        assert_eq!(parse("READ"), Some(PermissionKind::Read));
    }

    #[test]
    fn parse_unknown_returns_none() {
        assert_eq!(parse(""), None);
        assert_eq!(parse("rm -rf"), None);
        assert_eq!(parse("notatool"), None);
    }
}
