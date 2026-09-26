#![forbid(unsafe_code)]
//! Leader-prefixed keymap view (BRIDGE-PAR-239).
//!
//! TS truth: `packages/tui/src/keymap.tsx:20` (`LEADER_TOKEN`);
//! seeded from `crate::keymap_default::default_keymap`.

use crate::keymap_dispatch::KeymapDispatch;

/// Max leader chars.
pub const MAX_LEADER: usize = 16;

/// Full keymap: dispatch table plus leader prefix.
#[derive(Debug)]
pub struct KeymapFull {
    pub map: KeymapDispatch,
    pub leader: String,
}

impl Default for KeymapFull {
    fn default() -> Self {
        Self::new()
    }
}

impl KeymapFull {
    #[must_use]
    pub fn new() -> Self {
        Self {
            map: crate::keymap_default::default_keymap(),
            leader: String::new(),
        }
    }

    pub fn set_leader(&mut self, leader: &str) {
        self.leader = leader.chars().take(MAX_LEADER).collect();
    }

    #[must_use]
    pub fn dispatch_label(&self, key: &str) -> Option<String> {
        self.map.dispatch(key).map(|a| a.command.clone())
    }

    #[must_use]
    pub fn count(&self) -> usize {
        self.map.bindings.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeded_with_defaults() {
        let k = KeymapFull::new();
        assert_eq!(k.count(), 7);
        assert_eq!(
            k.dispatch_label("ctrl-p").as_deref(),
            Some("command.palette.show")
        );
    }

    #[test]
    fn leader_set_and_capped() {
        let mut k = KeymapFull::new();
        k.set_leader("space");
        assert_eq!(k.leader, "space");
        k.set_leader(&"l".repeat(40));
        assert_eq!(k.leader.len(), MAX_LEADER);
    }

    #[test]
    fn missing_key_none() {
        assert!(KeymapFull::new().dispatch_label("ctrl-z").is_none());
    }

    #[test]
    fn count_tracks_binds() {
        let mut k = KeymapFull::new();
        assert!(k.map.bind(
            "ctrl-z",
            crate::keymap_dispatch::KeyAction::new("Z", "cmd.z")
        ));
        assert_eq!(k.count(), 8);
    }

    #[test]
    fn leader_empty_by_default() {
        assert!(KeymapFull::new().leader.is_empty());
    }

    #[test]
    fn quit_lookup() {
        assert_eq!(
            KeymapFull::new().dispatch_label("ctrl-c").as_deref(),
            Some("app.quit")
        );
    }
}
