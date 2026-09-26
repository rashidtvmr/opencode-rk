#![forbid(unsafe_code)]
//! Run lifecycle: idle -> running -> done (BRIDGE-PAR-219, cf. TurnWire).

/// Max chars for [`Lifecycle::phase_of`].
pub const PHASE_CAP: usize = 32;

/// Run lifecycle state.
#[derive(Debug, Default)]
pub struct Lifecycle {
    phase: String,
    turns: u32,
}

impl Lifecycle {
    #[must_use]
    pub fn new() -> Self {
        Self {
            phase: "idle".to_string(),
            turns: 0,
        }
    }
    /// idle -> running; false otherwise.
    pub fn start(&mut self) -> bool {
        if self.phase == "idle" {
            self.phase = "running".to_string();
            return true;
        }
        false
    }
    /// running -> done, turns saturating +1.
    pub fn stop(&mut self) {
        if self.phase == "running" {
            self.phase = "done".to_string();
            self.turns = self.turns.saturating_add(1);
        }
    }
    /// done -> idle.
    pub fn reset(&mut self) {
        if self.phase == "done" {
            self.phase = "idle".to_string();
        }
    }
    #[must_use]
    pub fn phase_of(&self) -> &str {
        &self.phase
    }
    #[must_use]
    pub fn turns(&self) -> u32 {
        self.turns
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn starts_idle() {
        let l = Lifecycle::new();
        assert_eq!(l.phase_of(), "idle");
        assert_eq!(l.turns(), 0);
    }
    #[test]
    fn start_moves_to_running() {
        let mut l = Lifecycle::new();
        assert!(l.start());
        assert_eq!(l.phase_of(), "running");
        assert!(!l.start());
    }
    #[test]
    fn stop_counts_and_dones() {
        let mut l = Lifecycle::new();
        l.stop();
        assert_eq!((l.phase_of(), l.turns()), ("idle", 0));
        l.start();
        l.stop();
        assert_eq!((l.phase_of(), l.turns()), ("done", 1));
    }
    #[test]
    fn reset_returns_to_idle() {
        let mut l = Lifecycle::new();
        l.reset();
        assert_eq!(l.phase_of(), "idle");
        l.start();
        l.reset();
        assert_eq!(l.phase_of(), "running");
        l.stop();
        l.reset();
        assert_eq!(l.phase_of(), "idle");
        assert!(l.start());
    }
    #[test]
    fn full_cycle_recurs() {
        let mut l = Lifecycle::new();
        l.start();
        l.stop();
        l.reset();
        l.start();
        l.stop();
        assert_eq!((l.phase_of(), l.turns()), ("done", 2));
    }
    #[test]
    fn phase_cap_holds() {
        for p in ["idle", "running", "done"] {
            assert!(p.len() <= PHASE_CAP);
        }
        assert!(Lifecycle::new().phase_of().len() <= PHASE_CAP);
    }
}
