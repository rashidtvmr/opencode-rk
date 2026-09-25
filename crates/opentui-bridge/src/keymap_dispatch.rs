#![forbid(unsafe_code)]
//! Key-to-command dispatch table (BRIDGE-PAR-134).
//!
//! Mirrors `packages/tui/src/keymap.tsx`: keymap binds key strokes to named
//! commands; unmatched keys fall through (return `None`). This module is the
//! minimal state-machine core: register, overwrite, lookup, remove.

/// Max label chars.
pub const MAX_LABEL: usize = 64;
/// Max command chars.
pub const MAX_COMMAND: usize = 64;
/// Max key chars.
pub const MAX_KEY: usize = 32;
/// Max bindings.
pub const MAX_BINDINGS: usize = 64;

fn trunc(s: &str, cap: usize) -> String {
    s.chars().take(cap).collect()
}

/// Named command bound to a key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyAction {
    pub label: String,
    pub command: String,
}

impl KeyAction {
    #[must_use]
    pub fn new(label: &str, command: &str) -> Self {
        Self {
            label: trunc(label, MAX_LABEL),
            command: trunc(command, MAX_COMMAND),
        }
    }
}

/// Key dispatch table. Duplicate key overwrites prior action.
#[derive(Debug, Default)]
pub struct KeymapDispatch {
    pub bindings: Vec<(String, KeyAction)>,
}

impl KeymapDispatch {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Bind `key` to `action`. Overwrites on dup. False when full or key empty.
    pub fn bind(&mut self, key: &str, action: KeyAction) -> bool {
        let k = trunc(key, MAX_KEY);
        if k.is_empty() {
            return false;
        }
        if let Some(slot) = self.bindings.iter_mut().find(|(ek, _)| *ek == k) {
            slot.1 = action;
            return true;
        }
        if self.bindings.len() >= MAX_BINDINGS {
            return false;
        }
        self.bindings.push((k, action));
        true
    }

    /// Look up action for `key`. `None` = fallthrough (unbound).
    #[must_use]
    pub fn dispatch(&self, key: &str) -> Option<&KeyAction> {
        self.bindings.iter().find(|(k, _)| k == key).map(|(_, a)| a)
    }

    /// Remove binding. False when absent.
    pub fn unbind(&mut self, key: &str) -> bool {
        match self.bindings.iter().position(|(k, _)| k == key) {
            Some(i) => {
                self.bindings.remove(i);
                true
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bind_and_dispatch() {
        let mut d = KeymapDispatch::new();
        assert!(d.bind("ctrl+p", KeyAction::new("Palette", "command.palette.show")));
        let a = d.dispatch("ctrl+p").unwrap();
        assert_eq!(a.command, "command.palette.show");
        assert_eq!(a.label, "Palette");
    }

    #[test]
    fn overwrite_dup_key() {
        let mut d = KeymapDispatch::new();
        d.bind("ctrl+k", KeyAction::new("A", "cmd.a"));
        assert!(d.bind("ctrl+k", KeyAction::new("B", "cmd.b")));
        assert_eq!(d.bindings.len(), 1);
        assert_eq!(d.dispatch("ctrl+k").unwrap().command, "cmd.b");
    }

    #[test]
    fn missing_returns_none() {
        let d = KeymapDispatch::new();
        assert!(d.dispatch("ctrl+z").is_none());
    }

    #[test]
    fn unbind_removes() {
        let mut d = KeymapDispatch::new();
        d.bind("esc", KeyAction::new("Close", "dialog.close"));
        assert!(d.unbind("esc"));
        assert!(!d.unbind("esc"));
        assert!(d.dispatch("esc").is_none());
    }

    #[test]
    fn cap_rejects_when_full() {
        let mut d = KeymapDispatch::new();
        for i in 0..MAX_BINDINGS {
            assert!(d.bind(&format!("k{i}"), KeyAction::new("L", "c")));
        }
        assert!(!d.bind("overflow", KeyAction::new("L", "c")));
        assert_eq!(d.bindings.len(), MAX_BINDINGS);
    }

    #[test]
    fn caps_truncate_fields() {
        let a = KeyAction::new(&"l".repeat(100), &"c".repeat(100));
        assert_eq!(a.label.len(), MAX_LABEL);
        assert_eq!(a.command.len(), MAX_COMMAND);
        let mut d = KeymapDispatch::new();
        assert!(!d.bind("", KeyAction::new("L", "c")));
    }
}
