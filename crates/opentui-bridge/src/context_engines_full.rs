#![forbid(unsafe_code)]

//! Named context-engine registry (mirrors `packages/tui/src/context/` set).
//!
//! Evidence: `directory.ts:1-12` per-concern contexts composed in
//! `helper.tsx`; `kv.tsx:1-5` store-per-context; `ctx_bundle.rs:18-22`
//! `CtxBundle` facade over lane-local contexts. This adds the bounded
//! engine-name registry companion (`names` + `active`).

/// Max engine entries.
pub const MAX_ENGINES: usize = 22;
/// Max chars per engine name / active.
pub const MAX_NAME: usize = 64;

/// Bounded registry of context-engine names plus active selection.
#[derive(Debug, Clone, Default)]
pub struct ContextEngines {
    names: Vec<String>,
    active: String,
}

impl ContextEngines {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    fn ok(n: &str) -> bool {
        !n.is_empty() && n.chars().count() <= MAX_NAME
    }
    pub fn register(&mut self, name: &str) -> bool {
        if !Self::ok(name) || self.names.len() >= MAX_ENGINES {
            return false;
        }
        if self.names.iter().any(|e| e == name) {
            return false;
        }
        self.names.push(name.to_string());
        if self.active.is_empty() {
            self.active = name.to_string();
        }
        true
    }
    pub fn activate(&mut self, name: &str) -> bool {
        if self.names.iter().any(|e| e == name) {
            self.active = name.to_string();
            true
        } else {
            false
        }
    }
    #[must_use]
    pub fn active_of(&self) -> &str {
        &self.active
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reg_first_becomes_active() {
        let mut e = ContextEngines::new();
        assert!(e.register("editor"));
        assert_eq!(e.active_of(), "editor");
    }
    #[test]
    fn reg_bad_rejected() {
        let mut e = ContextEngines::new();
        assert!(!e.register(""));
        assert!(!e.register(&"x".repeat(MAX_NAME + 1)));
        assert_eq!(e.active_of(), "");
    }
    #[test]
    fn reg_dup_rejected() {
        let mut e = ContextEngines::new();
        assert!(e.register("kv"));
        assert!(!e.register("kv"));
    }
    #[test]
    fn reg_cap_rejected() {
        let mut e = ContextEngines::new();
        for i in 0..MAX_ENGINES {
            assert!(e.register(&format!("e{i}")));
        }
        assert!(!e.register("over"));
    }
    #[test]
    fn act_switches() {
        let mut e = ContextEngines::new();
        e.register("a");
        e.register("b");
        assert!(e.activate("b"));
        assert_eq!(e.active_of(), "b");
    }
    #[test]
    fn act_unknown_keeps() {
        let mut e = ContextEngines::new();
        e.register("a");
        assert!(!e.activate("z"));
        assert_eq!(e.active_of(), "a");
    }
}
