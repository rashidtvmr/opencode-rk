#![forbid(unsafe_code)]
//! TUI index mount latch (TS `packages/tui/src/index.tsx:1` re-exports `run`).
//! Single-mount guard: `start` latches once, `stop` releases.

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct TuiIndex {
    started: bool,
    mounts: u32,
}

impl TuiIndex {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            started: false,
            mounts: 0,
        }
    }
    pub fn start(&mut self) {
        if !self.started {
            self.started = true;
            self.mounts = self.mounts.saturating_add(1);
        }
    }
    pub fn stop(&mut self) {
        self.started = false;
    }
    #[must_use]
    pub const fn is_started(&self) -> bool {
        self.started
    }
    #[must_use]
    pub const fn mounts(&self) -> u32 {
        self.mounts
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn start_bumps_once() {
        let mut t = TuiIndex::new();
        t.start();
        t.start();
        assert!(t.is_started());
        assert_eq!(t.mounts(), 1);
    }
    #[test]
    fn stop_releases_restart_remounts() {
        let mut t = TuiIndex::new();
        t.start();
        t.stop();
        assert!(!t.is_started());
        t.start();
        assert_eq!(t.mounts(), 2);
    }
    #[test]
    fn stop_idempotent_fresh_zero() {
        let mut t = TuiIndex::new();
        t.stop();
        assert_eq!(t.mounts(), 0);
        assert!(!t.is_started());
    }
}
