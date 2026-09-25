#![forbid(unsafe_code)]
//! Ordered config key index (`packages/tui/src/config/index.tsx:1-50`).
/// Max tracked keys / bytes per key (fail-closed; TS unbounded).
pub const MAX_KEYS: usize = 64;
pub const MAX_KEY_LEN: usize = 128;

/// Ordered unique key set with hard caps.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConfigIndex {
    keys: Vec<String>,
}

impl ConfigIndex {
    #[must_use]
    pub fn new() -> Self {
        Self { keys: Vec::new() }
    }

    /// Insert key; false when empty, too long, duplicate, or full.
    pub fn add(&mut self, key: &str) -> bool {
        if key.is_empty() || key.len() > MAX_KEY_LEN {
            return false;
        }
        if self.keys.iter().any(|k| k == key) {
            return false;
        }
        if self.keys.len() >= MAX_KEYS {
            return false;
        }
        self.keys.push(key.to_string());
        true
    }

    #[must_use]
    pub fn has(&self, key: &str) -> bool {
        self.keys.iter().any(|k| k == key)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_and_has_roundtrip() {
        let mut i = ConfigIndex::new();
        assert!(i.is_empty());
        assert!(i.add("theme"));
        assert!(i.has("theme"));
        assert_eq!(i.len(), 1);
    }

    #[test]
    fn dup_empty_long_rejected() {
        let mut i = ConfigIndex::default();
        assert!(!i.add(""));
        assert!(!i.add(&"k".repeat(MAX_KEY_LEN + 1)));
        assert!(i.add("scroll"));
        assert!(!i.add("scroll"));
        assert_eq!(i.len(), 1);
    }

    #[test]
    fn cap_enforced() {
        let mut i = ConfigIndex::new();
        for n in 0..MAX_KEYS {
            assert!(i.add(&format!("k{n}")));
        }
        assert_eq!(i.len(), MAX_KEYS);
        assert!(!i.add("overflow"));
    }

    #[test]
    fn missing_key_absent() {
        let i = ConfigIndex::new();
        assert!(!i.has("leader"));
        assert!(i.is_empty());
    }
}
