#![forbid(unsafe_code)]
//! Owned full keybind config map.
//!
//! TS truth: `packages/tui/src/config/keybind.ts:45-240` Definitions table
//! (action -> default stroke); `keybind.ts:449-458` parse merges overrides.
//! Bounded port: max 64 entries, each side max 64 chars. Fail-closed.
//!
//! ponytail: flat Vec, no ordering/dedup beyond action replace; upgrade to
//! IndexMap when ordering matters.

/// Bounded action -> stroke map.
#[derive(Debug, Clone, Default)]
pub struct KeybindCfg {
    binds: Vec<(String, String)>,
}

impl KeybindCfg {
    /// Empty config.
    pub fn new() -> Self {
        Self { binds: Vec::new() }
    }

    /// Insert or replace `action -> stroke`. False when empty, overlong, or full.
    pub fn bind(&mut self, action: &str, stroke: &str) -> bool {
        if action.is_empty() || stroke.is_empty() {
            return false;
        }
        if action.len() > 64 || stroke.len() > 64 {
            return false;
        }
        if let Some(slot) = self.binds.iter_mut().find(|(a, _)| a == action) {
            slot.1 = stroke.to_string();
            return true;
        }
        if self.binds.len() >= 64 {
            return false;
        }
        self.binds.push((action.to_string(), stroke.to_string()));
        true
    }

    /// Stroke for `action`, if bound.
    pub fn lookup(&self, action: &str) -> Option<&str> {
        self.binds
            .iter()
            .find(|(a, _)| a == action)
            .map(|(_, s)| s.as_str())
    }

    /// Bound entry count.
    pub fn len(&self) -> usize {
        self.binds.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let mut c = KeybindCfg::new();
        assert!(c.bind("app_exit", "ctrl+c"));
        assert_eq!(c.lookup("app_exit"), Some("ctrl+c"));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn replace_keeps_len() {
        let mut c = KeybindCfg::new();
        assert!(c.bind("app_exit", "ctrl+c"));
        assert!(c.bind("app_exit", "ctrl+d"));
        assert_eq!(c.lookup("app_exit"), Some("ctrl+d"));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn rejects_empty_and_long() {
        let mut c = KeybindCfg::new();
        assert!(!c.bind("", "ctrl+c"));
        assert!(!c.bind("a", ""));
        assert!(!c.bind(&"a".repeat(65), "ctrl+c"));
        assert!(!c.bind("a", &"b".repeat(65)));
        assert_eq!(c.len(), 0);
    }

    #[test]
    fn caps_at_64() {
        let mut c = KeybindCfg::new();
        for i in 0..64 {
            assert!(c.bind(&format!("a{i}"), "ctrl+c"));
        }
        assert!(!c.bind("overflow", "ctrl+c"));
        assert_eq!(c.len(), 64);
    }

    #[test]
    fn miss_returns_none() {
        let c = KeybindCfg::new();
        assert_eq!(c.lookup("nope"), None);
    }
}
