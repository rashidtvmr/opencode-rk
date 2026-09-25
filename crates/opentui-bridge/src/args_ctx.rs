#![forbid(unsafe_code)]
//! Args context (std-only).
//!
//! Evidence (TS `packages/tui/src/context/args.tsx:1-16`):
//! - `createSimpleContext({ name: "Args", init: (props: Args) => props })`
//! - `Args` props: model/agent/prompt/sessionID strings, continue/fork/auto bools.
//! - Rust mirror stores raw CLI `args` + `cwd`; caps bound memory.

/// Max stored args.
pub const MAX_ARGS: usize = 64;
/// Max chars per arg.
pub const MAX_ARG_LEN: usize = 512;
/// Max chars for cwd.
pub const MAX_CWD: usize = 1024;

/// Args context snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgsCtx {
    pub args: Vec<String>,
    pub cwd: String,
}

impl ArgsCtx {
    #[must_use]
    pub fn new() -> Self {
        Self {
            args: Vec::new(),
            cwd: String::new(),
        }
    }

    /// Push arg; `false` when full. Longer truncates to [`MAX_ARG_LEN`].
    pub fn push(&mut self, a: &str) -> bool {
        if self.args.len() >= MAX_ARGS {
            return false;
        }
        self.args.push(a.chars().take(MAX_ARG_LEN).collect());
        true
    }

    /// Borrow arg at index, `None` when out of bounds.
    #[must_use]
    pub fn get(&self, i: usize) -> Option<&str> {
        self.args.get(i).map(String::as_str)
    }

    /// Number of stored args.
    #[must_use]
    pub fn len(&self) -> usize {
        self.args.len()
    }

    /// Set cwd. Empty rejects (`false`); longer truncates to [`MAX_CWD`].
    pub fn set_cwd(&mut self, cwd: &str) -> bool {
        if cwd.is_empty() {
            return false;
        }
        self.cwd = cwd.chars().take(MAX_CWD).collect();
        true
    }

    /// Borrow cwd.
    #[must_use]
    pub fn cwd(&self) -> &str {
        &self.cwd
    }
}

impl Default for ArgsCtx {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_get_roundtrip() {
        let mut c = ArgsCtx::new();
        assert!(c.push("--model"));
        assert!(c.push("gpt"));
        assert_eq!(c.len(), 2);
        assert_eq!(c.get(0), Some("--model"));
        assert_eq!(c.get(1), Some("gpt"));
    }

    #[test]
    fn oob_get_is_none() {
        let c = ArgsCtx::new();
        assert_eq!(c.get(0), None);
        assert_eq!(c.get(64), None);
    }

    #[test]
    fn cap_rejects_beyond_max() {
        let mut c = ArgsCtx::new();
        for i in 0..MAX_ARGS {
            assert!(c.push(&format!("a{i}")));
        }
        assert_eq!(c.len(), MAX_ARGS);
        assert!(!c.push("overflow"));
        assert_eq!(c.len(), MAX_ARGS);
    }

    #[test]
    fn trunc_long_arg() {
        let mut c = ArgsCtx::new();
        let long = "x".repeat(MAX_ARG_LEN + 50);
        assert!(c.push(&long));
        assert_eq!(c.get(0).unwrap().chars().count(), MAX_ARG_LEN);
    }

    #[test]
    fn cwd_set_get_reject_empty() {
        let mut c = ArgsCtx::new();
        assert_eq!(c.cwd(), "");
        assert!(!c.set_cwd(""));
        assert_eq!(c.cwd(), "");
        assert!(c.set_cwd("/tmp/work"));
        assert_eq!(c.cwd(), "/tmp/work");
    }

    #[test]
    fn cwd_truncates() {
        let mut c = ArgsCtx::new();
        let long = "d".repeat(MAX_CWD + 10);
        assert!(c.set_cwd(&long));
        assert_eq!(c.cwd().chars().count(), MAX_CWD);
    }
}
