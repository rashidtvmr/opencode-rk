#![forbid(unsafe_code)]
//! Minimal keymap.tsx pending sequence: bounded pending-key buffer.
//! TS truth: packages/tui/src/keymap.tsx:1-60 pending/leader addons;
//! crate::keybind_config_full bounded 64-char sides (fail-closed).
#[derive(Debug, Clone, Default)]
pub struct KeymapTs {
    pending: String,
}

impl KeymapTs {
    #[must_use]
    pub fn new() -> Self {
        Self {
            pending: String::new(),
        }
    }
    pub fn push_key(&mut self, key: &str) {
        self.pending.push_str(key);
        if self.pending.chars().count() > 64 {
            self.pending = self.pending.chars().take(64).collect();
        }
    }
    pub fn take(&mut self) -> String {
        std::mem::take(&mut self.pending)
    }
    #[must_use]
    pub fn pending_of(&self) -> &str {
        &self.pending
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_by_default() {
        let k = KeymapTs::new();
        assert_eq!(k.pending_of(), "");
    }
    #[test]
    fn push_accumulates() {
        let mut k = KeymapTs::new();
        k.push_key("g");
        k.push_key("g");
        assert_eq!(k.pending_of(), "gg");
    }
    #[test]
    fn caps_at_64_chars() {
        let mut k = KeymapTs::new();
        for _ in 0..70 {
            k.push_key("a");
        }
        assert_eq!(k.pending_of().chars().count(), 64);
    }
    #[test]
    fn take_drains() {
        let mut k = KeymapTs::new();
        k.push_key("a");
        assert_eq!(k.take(), "a");
        assert_eq!(k.pending_of(), "");
    }
}
