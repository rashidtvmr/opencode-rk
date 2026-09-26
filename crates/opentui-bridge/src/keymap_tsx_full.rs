#![forbid(unsafe_code)]
//! Minimal keymap.tsx leader state: leader token + pending count.
//! TS truth: packages/tui/src/keymap.tsx:20 LEADER_TOKEN, :53-60 mode
//! stack base, timed-leader addon; default mirrors keybind.ts:41.
const MAX_LEADER: usize = 16;
const DEFAULT_LEADER: &str = "ctrl+x";
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeymapTsx {
    leader: String,
    count: u32,
}

impl KeymapTsx {
    #[must_use]
    pub fn new() -> Self {
        Self {
            leader: DEFAULT_LEADER.to_string(),
            count: 0,
        }
    }
    pub fn set_leader(&mut self, leader: &str) {
        self.leader = leader.chars().take(MAX_LEADER).collect();
    }
    pub fn bump(&mut self) {
        self.count = self.count.saturating_add(1);
    }
    #[must_use]
    pub fn leader_of(&self) -> &str {
        &self.leader
    }
    #[must_use]
    pub fn count(&self) -> u32 {
        self.count
    }
}
impl Default for KeymapTsx {
    fn default() -> Self {
        Self::new()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_leader_and_count() {
        let k = KeymapTsx::new();
        assert_eq!(k.leader_of(), "ctrl+x");
        assert_eq!(k.count(), 0);
    }
    #[test]
    fn set_leader_caps_at_16_chars() {
        let mut k = KeymapTsx::new();
        k.set_leader("ctrl+x");
        assert_eq!(k.leader_of(), "ctrl+x");
        k.set_leader("12345678901234567890");
        assert_eq!(k.leader_of(), "1234567890123456");
    }
    #[test]
    fn bump_counts_and_saturates() {
        let mut k = KeymapTsx::new();
        k.bump();
        k.bump();
        assert_eq!(k.count(), 2);
        k.count = u32::MAX;
        k.bump();
        assert_eq!(k.count(), u32::MAX);
    }
}
