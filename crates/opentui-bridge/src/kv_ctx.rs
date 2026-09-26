#![forbid(unsafe_code)]

//! Bounded in-memory KV context (lane-local TS `kv.tsx` mirror).
//!
//! Source: `packages/tui/src/context/kv.tsx:51-56` flat get/set store;
//! bounds per BRIDGE-PAR-149 (64/512/128, fail-closed `bool`).
//! Sibling: `crate::context_kv::KvStore` (richer Result API, 256/128/4096).

/// Max pairs held. `set` of a new key past this returns false.
pub const MAX_PAIRS: usize = 128;
/// Max key chars. Overlong `set` returns false.
pub const MAX_KEY_LEN: usize = 64;
/// Max value chars. Overlong `set` returns false.
pub const MAX_VALUE_LEN: usize = 512;

/// Insertion-ordered bounded string KV map.
#[derive(Debug, Clone, Default)]
pub struct KvCtx {
    pairs: Vec<(String, String)>,
}

impl KvCtx {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Upsert. False when key/value over cap or store full (new key only).
    pub fn set(&mut self, k: &str, v: &str) -> bool {
        if k.chars().count() > MAX_KEY_LEN || v.chars().count() > MAX_VALUE_LEN {
            return false;
        }
        if let Some(slot) = self.pairs.iter_mut().find(|(key, _)| key == k) {
            slot.1 = v.to_string();
            return true;
        }
        if self.pairs.len() >= MAX_PAIRS {
            return false;
        }
        self.pairs.push((k.to_string(), v.to_string()));
        true
    }

    /// `store[key] ?? undefined` (`kv.tsx:51-53`).
    #[must_use]
    pub fn get(&self, k: &str) -> Option<&str> {
        self.pairs
            .iter()
            .find(|(key, _)| key == k)
            .map(|(_, v)| v.as_str())
    }

    /// Remove key. False when absent.
    pub fn del(&mut self, k: &str) -> bool {
        match self.pairs.iter().position(|(key, _)| key == k) {
            Some(i) => {
                self.pairs.remove(i);
                true
            }
            None => false,
        }
    }

    /// Keys in insertion order.
    #[must_use]
    pub fn keys(&self) -> Vec<String> {
        self.pairs.iter().map(|(k, _)| k.clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_get_roundtrip() {
        let mut ctx = KvCtx::new();
        assert!(ctx.set("a", "1"));
        assert_eq!(ctx.get("a"), Some("1"));
        assert_eq!(ctx.get("missing"), None);
    }

    #[test]
    fn overwrite_keeps_single_entry() {
        let mut ctx = KvCtx::new();
        assert!(ctx.set("a", "1"));
        assert!(ctx.set("a", "2"));
        assert_eq!(ctx.get("a"), Some("2"));
        assert_eq!(ctx.keys(), vec!["a".to_string()]);
    }

    #[test]
    fn del_missing_false() {
        let mut ctx = KvCtx::new();
        assert!(!ctx.del("nope"));
        assert!(ctx.set("a", "1"));
        assert!(ctx.del("a"));
        assert_eq!(ctx.get("a"), None);
        assert!(!ctx.del("a"));
    }

    #[test]
    fn caps_reject_overlong() {
        let mut ctx = KvCtx::new();
        assert!(!ctx.set(&"k".repeat(MAX_KEY_LEN + 1), "v"));
        assert!(!ctx.set("k", &"v".repeat(MAX_VALUE_LEN + 1)));
        assert!(ctx.keys().is_empty());
        assert!(ctx.set(&"k".repeat(MAX_KEY_LEN), &"v".repeat(MAX_VALUE_LEN)));
    }

    #[test]
    fn cap_rejects_new_key_when_full() {
        let mut ctx = KvCtx::new();
        for i in 0..MAX_PAIRS {
            assert!(ctx.set(&format!("k{i}"), "v"));
        }
        assert!(!ctx.set("one-more", "v"));
        assert!(ctx.set("k0", "updated"));
        assert_eq!(ctx.get("k0"), Some("updated"));
    }

    #[test]
    fn keys_insertion_order() {
        let mut ctx = KvCtx::new();
        ctx.set("b", "1");
        ctx.set("a", "2");
        ctx.set("c", "3");
        assert_eq!(
            ctx.keys(),
            vec!["b".to_string(), "a".to_string(), "c".to_string()]
        );
        ctx.del("a");
        assert_eq!(ctx.keys(), vec!["b".to_string(), "c".to_string()]);
    }
}
