#![forbid(unsafe_code)]
//! Bounded command-run flow (full shim over `command_shim`).
//!
//! TS truth: `crate::command_shim` stores name+desc, `invoke` is pure.
//! This flow tracks known names + total successful runs.
//!
//! `ponytail:` no callbacks/desc/keybinds; add when a real caller needs them.

/// Max registered names.
pub const MAX_NAMES: usize = 64;
/// Max chars per name.
pub const MAX_NAME_LEN: usize = 64;

/// Known command names plus successful-run counter.
#[derive(Debug, Default)]
pub struct ShimFlow {
    names: Vec<String>,
    runs: u64,
}

impl ShimFlow {
    /// Empty flow.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register name; false on empty/dup/full. Overlong truncated.
    pub fn register(&mut self, name: &str) -> bool {
        let n: String = name.chars().take(MAX_NAME_LEN).collect();
        if n.is_empty() {
            return false;
        }
        if self.names.iter().any(|e| e == &n) {
            return false;
        }
        if self.names.len() >= MAX_NAMES {
            return false;
        }
        self.names.push(n);
        true
    }

    /// Run known command; true bumps `runs`, false otherwise.
    pub fn run(&mut self, name: &str) -> bool {
        let n: String = name.chars().take(MAX_NAME_LEN).collect();
        if self.names.iter().any(|e| e == &n) {
            self.runs = self.runs.saturating_add(1);
            true
        } else {
            false
        }
    }

    /// Total successful runs.
    #[must_use]
    pub fn runs(&self) -> u64 {
        self.runs
    }

    /// Registered names.
    #[must_use]
    pub fn names(&self) -> &[String] {
        &self.names
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_run_bump() {
        let mut f = ShimFlow::new();
        assert!(f.register("a"));
        assert!(f.run("a"));
        assert_eq!(f.runs(), 1);
    }

    #[test]
    fn run_unknown_no_bump() {
        let mut f = ShimFlow::new();
        assert!(!f.run("nope"));
        assert_eq!(f.runs(), 0);
    }

    #[test]
    fn register_rejects_empty_dup_full() {
        let mut f = ShimFlow::new();
        assert!(!f.register(""));
        assert!(f.register("a"));
        assert!(!f.register("a"));
        for i in 0..MAX_NAMES - 1 {
            assert!(f.register(&format!("c{i}")));
        }
        assert!(!f.register("overflow"));
        assert_eq!(f.names().len(), MAX_NAMES);
    }

    #[test]
    fn truncates_long_name() {
        let mut f = ShimFlow::new();
        assert!(f.register(&"n".repeat(100)));
        assert_eq!(f.names()[0].len(), MAX_NAME_LEN);
        assert!(f.run(&"n".repeat(100)));
        assert_eq!(f.runs(), 1);
    }
}
