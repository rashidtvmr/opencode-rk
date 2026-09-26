#![forbid(unsafe_code)]

/// The 37 upstream `packages/opencode/src/cli/cmd/run` modules.
pub const RUN_MODULES: &[&str] = &[
    "demo",
    "entry.body",
    "footer",
    "footer.width",
    "permission.shared",
    "prompt.editor",
    "prompt.shared",
    "question.shared",
    "runtime",
    "runtime.boot",
    "runtime.lifecycle",
    "runtime.queue",
    "runtime.shared",
    "runtime.stdin",
    "scrollback.shared",
    "scrollback.surface",
    "scrollback.writer",
    "session-data",
    "session-replay",
    "session.shared",
    "splash",
    "stream",
    "stream.transport",
    "subagent-data",
    "theme",
    "tool",
    "trace",
    "turn-summary",
    "types",
    "variant.shared",
    "footer.command",
    "footer.menu",
    "footer.permission",
    "footer.prompt",
    "footer.question",
    "footer.subagent",
    "footer.view",
];

#[must_use]
pub const fn module_count() -> usize {
    RUN_MODULES.len()
}

#[must_use]
pub fn has_module(name: &str) -> bool {
    RUN_MODULES.contains(&name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_is_frozen() {
        assert_eq!(module_count(), 37);
    }

    #[test]
    fn known_modules_are_present() {
        assert!(has_module("footer"));
        assert!(has_module("stream.transport"));
        assert!(has_module("footer.question"));
    }

    #[test]
    fn unknown_modules_are_absent() {
        assert!(!has_module("footer.missing"));
        assert!(!has_module(""));
    }
}
