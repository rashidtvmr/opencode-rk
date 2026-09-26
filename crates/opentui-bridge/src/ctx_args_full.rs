#![forbid(unsafe_code)]
//! Full args context snapshot (std-only).
//! TS `packages/tui/src/context/args.tsx:1-16` simple Args props passthrough.

/// Max stored args.
pub const MAX_ARGS: usize = 32;
/// Max chars per arg.
pub const MAX_ARG_LEN: usize = 256;

/// Args snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CtxArgs {
    pub args: Vec<String>,
}

impl CtxArgs {
    #[must_use]
    pub fn new() -> Self {
        Self { args: Vec::new() }
    }

    /// Push arg; false when full. Truncates to [`MAX_ARG_LEN`] chars.
    pub fn push(&mut self, a: &str) -> bool {
        if self.args.len() >= MAX_ARGS {
            return false;
        }
        self.args.push(a.chars().take(MAX_ARG_LEN).collect());
        true
    }

    /// Borrow arg at index.
    #[must_use]
    pub fn get(&self, i: usize) -> Option<&str> {
        self.args.get(i).map(String::as_str)
    }

    /// Number of stored args.
    #[must_use]
    pub fn len(&self) -> usize {
        self.args.len()
    }
}

impl Default for CtxArgs {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_get_roundtrip() {
        let mut c = CtxArgs::new();
        assert!(c.push("--model"));
        assert!(c.push("gpt"));
        assert_eq!(c.len(), 2);
        assert_eq!(c.get(0), Some("--model"));
        assert_eq!(c.get(1), Some("gpt"));
    }

    #[test]
    fn oob_is_none() {
        let c = CtxArgs::new();
        assert_eq!(c.get(0), None);
        assert_eq!(c.get(99), None);
    }

    #[test]
    fn cap_rejects() {
        let mut c = CtxArgs::new();
        for i in 0..MAX_ARGS {
            assert!(c.push(&format!("a{i}")));
        }
        assert!(!c.push("x"));
        assert_eq!(c.len(), MAX_ARGS);
    }

    #[test]
    fn truncates() {
        let mut c = CtxArgs::new();
        let long = "x".repeat(MAX_ARG_LEN + 9);
        assert!(c.push(&long));
        assert_eq!(c.get(0).unwrap().chars().count(), MAX_ARG_LEN);
    }
}
