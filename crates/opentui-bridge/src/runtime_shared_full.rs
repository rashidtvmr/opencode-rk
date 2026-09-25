#![forbid(unsafe_code)]
//! Full shared runtime state (model + busy + turns).
//!
//! Extends `crate::run_runtime_shared` slot idea with turn tracking.

/// Max chars for model name.
pub const MODEL_CAP: usize = 128;

/// Shared runtime state.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeSharedFull {
    model: String,
    busy: bool,
    turns: u32,
}

impl RuntimeSharedFull {
    /// Empty state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Current model.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Whether a turn is in progress.
    pub fn is_busy(&self) -> bool {
        self.busy
    }

    /// Completed turn count.
    pub fn turns(&self) -> u32 {
        self.turns
    }

    /// Set model; false on empty/over-cap.
    pub fn set_model(&mut self, model: &str) -> bool {
        if model.is_empty() || model.len() > MODEL_CAP {
            return false;
        }
        self.model = model.to_string();
        true
    }

    /// Begin a turn; false when already busy.
    pub fn start_turn(&mut self) -> bool {
        if self.busy {
            return false;
        }
        self.busy = true;
        true
    }

    /// End a turn; bumps count saturating, clears busy.
    pub fn end_turn(&mut self) {
        self.turns = self.turns.saturating_add(1);
        self.busy = false;
    }

    /// Human-readable status.
    pub fn status(&self) -> String {
        format!(
            "model={} busy={} turns={}",
            self.model, self.busy, self.turns
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_model_empty_false() {
        let mut r = RuntimeSharedFull::new();
        assert!(!r.set_model(""));
        assert_eq!(r.model(), "");
    }

    #[test]
    fn set_model_over_cap_false() {
        let mut r = RuntimeSharedFull::new();
        let long = "x".repeat(MODEL_CAP + 1);
        assert!(!r.set_model(&long));
    }

    #[test]
    fn set_model_ok() {
        let mut r = RuntimeSharedFull::new();
        assert!(r.set_model("gpt"));
        assert_eq!(r.model(), "gpt");
    }

    #[test]
    fn double_start_false() {
        let mut r = RuntimeSharedFull::new();
        assert!(r.start_turn());
        assert!(!r.start_turn());
        assert!(r.is_busy());
    }

    #[test]
    fn end_bumps_clears() {
        let mut r = RuntimeSharedFull::new();
        r.start_turn();
        r.end_turn();
        assert_eq!(r.turns(), 1);
        assert!(!r.is_busy());
        assert!(r.start_turn());
    }

    #[test]
    fn end_saturates() {
        let mut r = RuntimeSharedFull {
            turns: u32::MAX,
            ..Default::default()
        };
        r.end_turn();
        assert_eq!(r.turns(), u32::MAX);
    }

    #[test]
    fn status_parts() {
        let mut r = RuntimeSharedFull::new();
        r.set_model("m");
        r.start_turn();
        let s = r.status();
        assert!(s.contains("m"), "{s}");
        assert!(s.contains("true"), "{s}");
        assert!(s.contains("turns="), "{s}");
    }
}
