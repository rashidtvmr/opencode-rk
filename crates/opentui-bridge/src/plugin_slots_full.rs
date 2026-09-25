#![forbid(unsafe_code)]
//! Bounded custom slot-name list (mirrors `slots.tsx:25-65` registry for
//! non-host slots; string-name companion to `crate::plugin_slots`).

/// Max names held (host `SlotRegistry` holds 64; custom list holds 32).
pub const MAX_FULL_SLOTS: usize = 32;
/// Max bytes per slot name.
pub const MAX_FULL_NAME: usize = 64;

/// Ordered unique slot names, fail-closed on empty/oversize/full/duplicate.
#[derive(Debug, Default, Clone)]
pub struct SlotList {
    names: Vec<String>,
}

impl SlotList {
    #[must_use]
    pub const fn new() -> Self {
        Self { names: Vec::new() }
    }

    /// Add `name`; `false` when empty, too long, duplicate, or full.
    pub fn add(&mut self, name: &str) -> bool {
        if name.is_empty() || name.len() > MAX_FULL_NAME {
            return false;
        }
        if self.names.iter().any(|n| n == name) {
            return false;
        }
        if self.names.len() >= MAX_FULL_SLOTS {
            return false;
        }
        self.names.push(name.to_string());
        true
    }

    #[must_use]
    pub fn has(&self, name: &str) -> bool {
        self.names.iter().any(|n| n == name)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.names.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_has_len() {
        let mut s = SlotList::new();
        assert!(s.add("home_footer"));
        assert!(s.has("home_footer"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn rejects_bad_names() {
        let mut s = SlotList::new();
        assert!(!s.add(""));
        assert!(!s.add(&"x".repeat(65)));
        assert!(s.add("a"));
        assert!(!s.add("a"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn caps_at_32() {
        let mut s = SlotList::new();
        for i in 0..MAX_FULL_SLOTS {
            assert!(s.add(&format!("slot{i}")));
        }
        assert!(!s.add("one-more"));
        assert_eq!(s.len(), MAX_FULL_SLOTS);
    }
}
