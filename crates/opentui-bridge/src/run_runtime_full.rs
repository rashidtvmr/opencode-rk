#![forbid(unsafe_code)]
//! Full runtime: shared state plus local turn mirror.
//! Mirrors `packages/opencode/src/cli/cmd/run/runtime.ts` turn loop.
use crate::runtime_shared_full::RuntimeSharedFull;

/// Max status length.
pub const STATUS_CAP: usize = 256;

/// Full runtime wrapping shared state.
#[derive(Debug, Default)]
pub struct RuntimeFull {
    rt: RuntimeSharedFull,
    turns: u32,
}

impl RuntimeFull {
    /// Empty runtime.
    pub fn new() -> Self {
        Self::default()
    }
    /// Shared state.
    pub fn rt(&self) -> &RuntimeSharedFull {
        &self.rt
    }
    /// Local completed-turn count.
    pub fn turns(&self) -> u32 {
        self.turns
    }
    /// Set model via shared state.
    pub fn set_model(&mut self, model: &str) -> bool {
        self.rt.set_model(model)
    }
    /// Begin a turn; false when busy.
    pub fn begin(&mut self) -> bool {
        self.rt.start_turn()
    }
    /// End a turn; local counter saturating +1.
    pub fn end(&mut self) {
        self.rt.end_turn();
        self.turns = self.turns.saturating_add(1);
    }
    /// Status with model, turns, busy; capped at 256.
    pub fn status(&self) -> String {
        let mut s = format!(
            "model={} turns={} busy={}",
            self.rt.model(),
            self.turns,
            self.rt.is_busy()
        );
        if s.len() > STATUS_CAP {
            s.truncate(STATUS_CAP);
            while !s.is_char_boundary(s.len()) {
                s.pop();
            }
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn new_is_idle() {
        let r = RuntimeFull::new();
        assert_eq!(r.turns(), 0);
        assert!(!r.rt().is_busy());
    }
    #[test]
    fn begin_busy_guard() {
        let mut r = RuntimeFull::new();
        assert!(r.begin());
        assert!(!r.begin());
    }
    #[test]
    fn end_bumps_and_clears() {
        let mut r = RuntimeFull::new();
        r.begin();
        r.end();
        assert_eq!(r.turns(), 1);
        assert_eq!(r.rt().turns(), 1);
        assert!(!r.rt().is_busy());
    }
    #[test]
    fn end_saturates() {
        let mut r = RuntimeFull {
            rt: RuntimeSharedFull::default(),
            turns: u32::MAX,
        };
        r.end();
        assert_eq!(r.turns(), u32::MAX);
    }
    #[test]
    fn status_has_parts() {
        let mut r = RuntimeFull::new();
        r.set_model("gpt");
        r.begin();
        let s = r.status();
        assert!(s.contains("gpt"), "{s}");
        assert!(s.contains("busy=true"), "{s}");
        assert!(s.contains("turns="), "{s}");
    }
    #[test]
    fn status_capped() {
        let r = RuntimeFull::new();
        assert!(r.status().len() <= STATUS_CAP);
    }
}
