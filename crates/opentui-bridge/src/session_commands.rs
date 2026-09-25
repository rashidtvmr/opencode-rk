#![forbid(unsafe_code)]
//! Session command list (31 actions).
//!
//! `ponytail:` flat id/label table; upgrade to real dispatch when a caller needs it.

/// One session command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionCommand {
    pub id: &'static str,
    pub label: &'static str,
    pub needs_session: bool,
}

/// All 31 session commands.
pub const COMMANDS: [SessionCommand; 31] = [
    SessionCommand {
        id: "new",
        label: "New session",
        needs_session: false,
    },
    SessionCommand {
        id: "sessions",
        label: "List sessions",
        needs_session: false,
    },
    SessionCommand {
        id: "model",
        label: "Switch model",
        needs_session: true,
    },
    SessionCommand {
        id: "agents",
        label: "List agents",
        needs_session: true,
    },
    SessionCommand {
        id: "mcps",
        label: "List MCP servers",
        needs_session: true,
    },
    SessionCommand {
        id: "status",
        label: "Show status",
        needs_session: true,
    },
    SessionCommand {
        id: "themes",
        label: "Switch theme",
        needs_session: false,
    },
    SessionCommand {
        id: "fork",
        label: "Fork session",
        needs_session: true,
    },
    SessionCommand {
        id: "undo",
        label: "Undo",
        needs_session: true,
    },
    SessionCommand {
        id: "redo",
        label: "Redo",
        needs_session: true,
    },
    SessionCommand {
        id: "share",
        label: "Share session",
        needs_session: true,
    },
    SessionCommand {
        id: "export",
        label: "Export session",
        needs_session: true,
    },
    SessionCommand {
        id: "dialog-message",
        label: "Message dialog",
        needs_session: true,
    },
    SessionCommand {
        id: "dialog-timeline",
        label: "Timeline dialog",
        needs_session: true,
    },
    SessionCommand {
        id: "dialog-fork",
        label: "Fork dialog",
        needs_session: true,
    },
    SessionCommand {
        id: "dialog-subagent",
        label: "Subagent dialog",
        needs_session: true,
    },
    SessionCommand {
        id: "subagent-footer",
        label: "Subagent footer",
        needs_session: true,
    },
    SessionCommand {
        id: "sidebar-rail",
        label: "Toggle sidebar rail",
        needs_session: true,
    },
    SessionCommand {
        id: "footer-status",
        label: "Footer status",
        needs_session: true,
    },
    SessionCommand {
        id: "home",
        label: "Go home",
        needs_session: false,
    },
    SessionCommand {
        id: "destination",
        label: "Go to destination",
        needs_session: true,
    },
    SessionCommand {
        id: "prompt-send",
        label: "Send prompt",
        needs_session: true,
    },
    SessionCommand {
        id: "prompt-clear",
        label: "Clear prompt",
        needs_session: true,
    },
    SessionCommand {
        id: "prompt-history-next",
        label: "Next history",
        needs_session: true,
    },
    SessionCommand {
        id: "prompt-history-prev",
        label: "Previous history",
        needs_session: true,
    },
    SessionCommand {
        id: "prompt-edit",
        label: "Edit prompt",
        needs_session: true,
    },
    SessionCommand {
        id: "prompt-copy",
        label: "Copy prompt",
        needs_session: true,
    },
    SessionCommand {
        id: "prompt-paste",
        label: "Paste prompt",
        needs_session: true,
    },
    SessionCommand {
        id: "prompt-submit",
        label: "Submit prompt",
        needs_session: true,
    },
    SessionCommand {
        id: "prompt-cancel",
        label: "Cancel prompt",
        needs_session: true,
    },
    SessionCommand {
        id: "prompt-retry",
        label: "Retry prompt",
        needs_session: true,
    },
];

/// Find a command by id.
#[must_use]
pub fn find(id: &str) -> Option<SessionCommand> {
    COMMANDS.iter().copied().find(|c| c.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn len_is_31() {
        assert_eq!(COMMANDS.len(), 31);
    }

    #[test]
    fn ids_unique() {
        let mut ids: Vec<&str> = COMMANDS.iter().map(|c| c.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 31);
    }

    #[test]
    fn find_known() {
        let c = find("fork").expect("fork exists");
        assert_eq!(c.label, "Fork session");
        assert!(c.needs_session);
    }

    #[test]
    fn find_unknown_none() {
        assert_eq!(find("nope-missing"), None);
    }

    #[test]
    fn new_needs_no_session() {
        let c = find("new").expect("new exists");
        assert!(!c.needs_session);
    }

    #[test]
    fn labels_nonempty() {
        assert!(COMMANDS
            .iter()
            .all(|c| !c.id.is_empty() && !c.label.is_empty()));
    }
}
