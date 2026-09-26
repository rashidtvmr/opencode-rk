#![forbid(unsafe_code)]
//! TSX runtime lifecycle.
//! Mirrors `packages/tui/src/runtime.tsx` boot/ready contract.

/// Lifecycle state for TSX runtime boot.
#[derive(Debug, Default)]
pub struct RuntimeTsx {
    ready: bool,
    boots: u32,
}

impl RuntimeTsx {
    /// Fresh unbooted runtime.
    pub fn new() -> Self {
        Self::default()
    }
    /// Boot runtime; marks ready, saturating boot count.
    pub fn boot(&mut self) {
        self.ready = true;
        self.boots = self.boots.saturating_add(1);
    }
    /// Shut runtime down; clears ready flag.
    pub fn shutdown(&mut self) {
        self.ready = false;
    }
    /// True after boot until shutdown.
    pub fn is_ready(&self) -> bool {
        self.ready
    }
    /// Completed boot count.
    pub fn boots(&self) -> u32 {
        self.boots
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn new_not_ready() {
        let r = RuntimeTsx::new();
        assert!(!r.is_ready());
        assert_eq!(r.boots(), 0);
    }
    #[test]
    fn boot_marks_ready_and_bumps() {
        let mut r = RuntimeTsx::new();
        r.boot();
        r.boot();
        assert!(r.is_ready());
        assert_eq!(r.boots(), 2);
    }
    #[test]
    fn shutdown_clears_ready_keeps_boots() {
        let mut r = RuntimeTsx::new();
        r.boot();
        r.shutdown();
        assert!(!r.is_ready());
        assert_eq!(r.boots(), 1);
    }
}
