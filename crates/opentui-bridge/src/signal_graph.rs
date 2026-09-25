#![forbid(unsafe_code)]
//! Dependency-graph model of the solid-js reactivity used in TS truth
//! (`context/local.tsx:78-83`, `context/sync.tsx:23,64`):
//! `createSignal` -> `values`, `createMemo` -> `memos` (+ deps),
//! `createEffect` -> `run_effects` counter. `createStore` is modeled as
//! namespaced `values` keys (`"<store>/<field>"`). No JS runtime; std-only
//! bookkeeping so the bridge can reason about signal topology natively.

/// Max chars kept per key (and per memo dep entry).
pub const MAX_KEY_LEN: usize = 64;
/// Max `values` entries (`createSignal`/`createStore` cells).
pub const MAX_VALUES: usize = 128;
/// Max `memos` entries (`createMemo` cells).
pub const MAX_MEMOS: usize = 64;

/// Reactive dependency graph: signals, derived memos, effect-run counter.
#[derive(Debug, Clone, Default)]
pub struct SignalGraph {
    /// `(key, value)` signal cells.
    pub values: Vec<(String, i64)>,
    /// `(key, value, dep keys)` memo cells.
    pub memos: Vec<(String, i64, Vec<String>)>,
    /// Total `run_effects` invocations.
    pub effects_run: u64,
}

fn cap_key(key: &str) -> String {
    key.chars().take(MAX_KEY_LEN).collect()
}

impl SignalGraph {
    /// Empty graph.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert or update a signal cell. False when full (new key, at cap).
    pub fn set(&mut self, key: &str, val: i64) -> bool {
        let key = cap_key(key);
        if let Some(slot) = self.values.iter_mut().find(|(k, _)| *k == key) {
            slot.1 = val;
            return true;
        }
        if self.values.len() >= MAX_VALUES {
            return false;
        }
        self.values.push((key, val));
        true
    }

    /// Read a signal cell.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<i64> {
        let key = cap_key(key);
        self.values.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
    }

    /// Insert or update a memo cell with its dep keys. False when full.
    pub fn add_memo(&mut self, key: &str, val: i64, deps: &[&str]) -> bool {
        let key = cap_key(key);
        let owned: Vec<String> = deps.iter().map(|d| cap_key(d)).collect();
        if let Some(slot) = self.memos.iter_mut().find(|(k, _, _)| *k == key) {
            slot.1 = val;
            slot.2 = owned;
            return true;
        }
        if self.memos.len() >= MAX_MEMOS {
            return false;
        }
        self.memos.push((key, val, owned));
        true
    }

    /// Record one effect pass; returns the running total.
    pub fn run_effects(&mut self) -> u64 {
        self.effects_run = self.effects_run.saturating_add(1);
        self.effects_run
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_and_get_roundtrip() {
        let mut g = SignalGraph::new();
        assert!(g.set("count", 41));
        assert!(g.set("count", 42));
        assert_eq!(g.get("count"), Some(42));
    }

    #[test]
    fn missing_key_returns_none() {
        let g = SignalGraph::new();
        assert_eq!(g.get("nope"), None);
    }

    #[test]
    fn memo_deps_kept() {
        let mut g = SignalGraph::new();
        assert!(g.add_memo("double", 84, &["count"]));
        let (_, v, deps) = g.memos.iter().find(|(k, _, _)| k == "double").unwrap();
        assert_eq!(*v, 84);
        assert_eq!(deps, &vec!["count".to_string()]);
    }

    #[test]
    fn effects_counter_accumulates() {
        let mut g = SignalGraph::new();
        assert_eq!(g.run_effects(), 1);
        assert_eq!(g.run_effects(), 2);
        assert_eq!(g.effects_run, 2);
    }

    #[test]
    fn values_cap_rejects_new_keys() {
        let mut g = SignalGraph::new();
        for i in 0..MAX_VALUES {
            assert!(g.set(&format!("k{i}"), i as i64));
        }
        assert!(!g.set("overflow", 1));
        assert!(g.set("k0", 99));
        assert_eq!(g.get("k0"), Some(99));
    }

    #[test]
    fn memos_cap_and_key_truncation() {
        let mut g = SignalGraph::new();
        let long = "x".repeat(MAX_KEY_LEN + 10);
        assert!(g.set(&long, 7));
        assert_eq!(g.get(&long), Some(7));
        assert_eq!(g.values[0].0.len(), MAX_KEY_LEN);
        for i in 0..MAX_MEMOS {
            assert!(g.add_memo(&format!("m{i}"), i as i64, &[]));
        }
        assert!(!g.add_memo("extra", 1, &[]));
    }
}
