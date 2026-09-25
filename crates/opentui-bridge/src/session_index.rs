#![forbid(unsafe_code)]

//! Session index: foreground task count, bound action ids, kv hide flag.
//!
//! TS truth: `packages/tui/src/routes/session/index.tsx` - `foregroundTasks`
//! memo (line 214: running non-background `task` tool parts), `sessionBindingCommands`
//! list (line 116), `kv.signal` visibility toggles (lines 249-261).

/// Max bound action ids retained.
pub const MAX_ACTIONS: usize = 32;
/// Max chars per action id.
pub const MAX_ACTION_LEN: usize = 64;

/// Index state for the session route.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionIndex {
    pub foreground_tasks: u32,
    pub actions: Vec<String>,
    pub kv_hide: bool,
}

impl SessionIndex {
    /// Empty index: no tasks, no actions, kv shown.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// True when running foreground tasks fit within `max`.
    #[must_use]
    pub fn gate_tasks(&self, max: u32) -> bool {
        self.foreground_tasks <= max
    }

    /// Register an action id; false on duplicate or when capped at 32.
    /// Overlong ids are truncated to 64 chars before dedup/push.
    pub fn register_action(&mut self, action: String) -> bool {
        let action: String = action.chars().take(MAX_ACTION_LEN).collect();
        if self.actions.iter().any(|a| *a == action) {
            return false;
        }
        if self.actions.len() >= MAX_ACTIONS {
            return false;
        }
        self.actions.push(action);
        true
    }

    /// True when the action id is already registered.
    #[must_use]
    pub fn has_action(&self, action: &str) -> bool {
        self.actions.iter().any(|a| a == action)
    }

    /// Flip the kv hide flag.
    pub fn toggle_kv(&mut self) {
        self.kv_hide = !self.kv_hide;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_passes_within_max() {
        let idx = SessionIndex {
            foreground_tasks: 2,
            ..SessionIndex::new()
        };
        assert!(idx.gate_tasks(2));
    }

    #[test]
    fn gate_fails_over_max() {
        let idx = SessionIndex {
            foreground_tasks: 3,
            ..SessionIndex::new()
        };
        assert!(!idx.gate_tasks(2));
    }

    #[test]
    fn dup_action_returns_false() {
        let mut idx = SessionIndex::new();
        assert!(idx.register_action("session.sidebar.toggle".to_string()));
        assert!(!idx.register_action("session.sidebar.toggle".to_string()));
        assert_eq!(idx.actions.len(), 1);
    }

    #[test]
    fn cap_32_rejects_overflow() {
        let mut idx = SessionIndex::new();
        for i in 0..MAX_ACTIONS {
            assert!(idx.register_action(format!("action.{i}")));
        }
        assert!(!idx.register_action("one.too.many".to_string()));
        assert_eq!(idx.actions.len(), MAX_ACTIONS);
    }

    #[test]
    fn toggle_flips() {
        let mut idx = SessionIndex::new();
        assert!(!idx.kv_hide);
        idx.toggle_kv();
        assert!(idx.kv_hide);
        idx.toggle_kv();
        assert!(!idx.kv_hide);
    }

    #[test]
    fn has_missing_returns_false() {
        let idx = SessionIndex::new();
        assert!(!idx.has_action("session.toggle.actions"));
    }

    #[test]
    fn overlong_action_truncated_to_64() {
        let mut idx = SessionIndex::new();
        assert!(idx.register_action("x".repeat(100)));
        assert_eq!(idx.actions[0].chars().count(), MAX_ACTION_LEN);
    }
}
