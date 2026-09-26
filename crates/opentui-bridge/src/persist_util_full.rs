#![forbid(unsafe_code)]
//! Bounded in-memory persistence mirror (`util/persistence.ts:1-33`).
//!
//! TS truth is file-backed (`Bun.file`/`Bun.write`/tmp+rename); this type is
//! the testable in-memory twin: same small-string save/load/remove shape,
//! no fs, no deps. Cap 64 entries, 4 KiB per key/value; oversize or full
//! save fails closed with `false`, never partial.

/// Max entries held.
pub const MAX_ENTRIES: usize = 64;
/// Max bytes per key and per value.
pub const MAX_ITEM_BYTES: usize = 4 * 1024;

/// Bounded in-memory key-value store.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PersistMem {
    pub entries: Vec<(String, String)>,
}

impl PersistMem {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Insert or overwrite; `false` when key/value over cap or full on new key.
    pub fn save(&mut self, k: &str, v: &str) -> bool {
        if k.len() > MAX_ITEM_BYTES || v.len() > MAX_ITEM_BYTES {
            return false;
        }
        if let Some(pos) = self.entries.iter().position(|(ek, _)| ek == k) {
            self.entries.remove(pos);
            self.entries.push((k.to_string(), v.to_string()));
            return true;
        }
        if self.entries.len() >= MAX_ENTRIES {
            return false;
        }
        self.entries.push((k.to_string(), v.to_string()));
        true
    }

    /// Look up a value by key.
    pub fn load(&self, k: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(ek, _)| ek == k)
            .map(|(_, v)| v.as_str())
    }

    /// Delete a key; `true` iff present.
    pub fn remove(&mut self, k: &str) -> bool {
        if let Some(pos) = self.entries.iter().position(|(ek, _)| ek == k) {
            self.entries.remove(pos);
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let mut m = PersistMem::new();
        assert!(m.save("k", "v"));
        assert_eq!(m.load("k"), Some("v"));
    }

    #[test]
    fn overwrite_keeps_one_entry() {
        let mut m = PersistMem::new();
        assert!(m.save("k", "a"));
        assert!(m.save("k", "b"));
        assert_eq!(m.load("k"), Some("b"));
        assert_eq!(m.entries.len(), 1);
    }

    #[test]
    fn full_rejects_new_key() {
        let mut m = PersistMem::new();
        for i in 0..MAX_ENTRIES {
            assert!(m.save(&format!("k{i}"), "v"));
        }
        assert!(!m.save("overflow", "v"));
        assert_eq!(m.load("overflow"), None);
        assert!(m.save("k0", "new"));
    }

    #[test]
    fn oversize_key_or_value_rejected() {
        let mut m = PersistMem::new();
        let big = "x".repeat(MAX_ITEM_BYTES + 1);
        assert!(!m.save(&big, "v"));
        assert!(!m.save("k", &big));
        assert!(m.entries.is_empty());
    }

    #[test]
    fn remove_present_and_missing() {
        let mut m = PersistMem::new();
        assert!(m.save("k", "v"));
        assert!(m.remove("k"));
        assert_eq!(m.load("k"), None);
        assert!(!m.remove("k"));
    }

    #[test]
    fn missing_load_is_none() {
        let m = PersistMem::new();
        assert_eq!(m.load("nope"), None);
    }
}
