#![forbid(unsafe_code)]

//! Versioned [`KvCtx`] wrapper (TS solid-store mirror).

use crate::kv_ctx::KvCtx;

/// `KvCtx` plus a bump-on-write version counter.
#[derive(Debug, Clone, Default)]
pub struct SolidStore {
    kv: KvCtx,
    version: u64,
}

impl SolidStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, k: &str, v: &str) -> bool {
        if self.kv.set(k, v) {
            self.version = self.version.wrapping_add(1);
            true
        } else {
            false
        }
    }

    #[must_use]
    pub fn get(&self, k: &str) -> Option<&str> {
        self.kv.get(k)
    }

    #[must_use]
    pub fn version(&self) -> u64 {
        self.version
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_at_zero() {
        let s = SolidStore::new();
        assert_eq!(s.version(), 0);
        assert_eq!(s.get("a"), None);
    }

    #[test]
    fn set_bumps_version() {
        let mut s = SolidStore::new();
        assert!(s.set("a", "1"));
        assert_eq!(s.version(), 1);
        assert_eq!(s.get("a"), Some("1"));
    }

    #[test]
    fn overwrite_bumps_again() {
        let mut s = SolidStore::new();
        assert!(s.set("a", "1"));
        assert!(s.set("a", "2"));
        assert_eq!(s.version(), 2);
        assert_eq!(s.get("a"), Some("2"));
    }

    #[test]
    fn rejected_set_does_not_bump() {
        let mut s = SolidStore::new();
        assert!(s.set("a", "1"));
        assert!(!s.set(&"k".repeat(65), "v"));
        assert!(!s.set("k", &"v".repeat(513)));
        assert_eq!(s.version(), 1);
    }

    #[test]
    fn missing_key_returns_none() {
        let s = SolidStore::new();
        assert_eq!(s.get("nope"), None);
    }
}
