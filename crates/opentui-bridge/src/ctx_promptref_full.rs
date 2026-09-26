#![forbid(unsafe_code)]
//! Prompt ref holder mirroring `context/prompt.tsx` current/set.

/// Maximum prompt ref id length in chars.
pub const MAX_ID_LEN: usize = 128;

/// Minimal prompt ref holder (`current` + live flag).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PromptRef {
    id: String,
    live: bool,
}

impl PromptRef {
    /// Set id, truncated to 128 chars; empty clears liveness.
    pub fn set(&mut self, id: &str) {
        let capped: String = id.chars().take(MAX_ID_LEN).collect();
        self.live = !capped.is_empty();
        self.id = capped;
    }

    /// Clear id and liveness.
    pub fn clear(&mut self) {
        self.id.clear();
        self.live = false;
    }

    /// Whether a ref is currently held.
    #[must_use]
    pub fn is_live(&self) -> bool {
        self.live
    }

    /// Current id (empty when not live).
    #[must_use]
    pub fn id_of(&self) -> &str {
        &self.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_not_live() {
        assert!(!PromptRef::default().is_live());
    }

    #[test]
    fn set_roundtrips_id() {
        let mut r = PromptRef::default();
        r.set("abc");
        assert!(r.is_live());
        assert_eq!(r.id_of(), "abc");
    }

    #[test]
    fn set_truncates_to_128() {
        let mut r = PromptRef::default();
        r.set(&"x".repeat(200));
        assert_eq!(r.id_of().chars().count(), MAX_ID_LEN);
    }

    #[test]
    fn clear_unsets_live() {
        let mut r = PromptRef::default();
        r.set("abc");
        r.clear();
        assert!(!r.is_live());
        assert_eq!(r.id_of(), "");
    }
}
