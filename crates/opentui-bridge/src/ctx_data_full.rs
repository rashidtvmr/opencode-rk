#![forbid(unsafe_code)]

//! Bounded context data store.
//! Mirrors `packages/tui/src/context/data.tsx` flat record get/set.
//! Bounds: 64 pairs, 512 chars per key/value, fail-closed bool.

/// Max pairs held.
pub const MAX_PAIRS: usize = 64;
/// Max chars per key/value.
pub const MAX_LEN: usize = 512;

/// Insertion-ordered bounded string map.
#[derive(Debug, Clone, Default)]
pub struct CtxData {
    pairs: Vec<(String, String)>,
}

impl CtxData {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Upsert. False when over cap or full on new key.
    pub fn set(&mut self, k: &str, v: &str) -> bool {
        if k.chars().count() > MAX_LEN || v.chars().count() > MAX_LEN {
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

    #[must_use]
    pub fn get(&self, k: &str) -> Option<&str> {
        self.pairs
            .iter()
            .find(|(key, _)| key == k)
            .map(|(_, v)| v.as_str())
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.pairs.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let mut d = CtxData::new();
        assert!(d.set("a", "1"));
        assert_eq!(d.get("a"), Some("1"));
        assert_eq!(d.get("x"), None);
        assert_eq!(d.len(), 1);
    }

    #[test]
    fn overwrite() {
        let mut d = CtxData::new();
        assert!(d.set("a", "1"));
        assert!(d.set("a", "2"));
        assert_eq!(d.get("a"), Some("2"));
        assert_eq!(d.len(), 1);
    }

    #[test]
    fn caps() {
        let mut d = CtxData::new();
        assert!(!d.set(&"k".repeat(513), "v"));
        assert!(!d.set("k", &"v".repeat(513)));
        assert!(d.set(&"k".repeat(512), &"v".repeat(512)));
        assert_eq!(d.len(), 1);
    }

    #[test]
    fn full() {
        let mut d = CtxData::new();
        for i in 0..MAX_PAIRS {
            assert!(d.set(&format!("k{i}"), "v"));
        }
        assert_eq!(d.len(), MAX_PAIRS);
        assert!(!d.set("extra", "v"));
        assert!(d.set("k0", "u"));
        assert_eq!(d.get("k0"), Some("u"));
    }
}
