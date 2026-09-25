#![forbid(unsafe_code)]
//! Dirty-latch sync tick (TS: `packages/tui/src/context/sync.tsx` batch/stale pattern).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CtxSync {
    tick: u64,
    dirty: bool,
}
impl CtxSync {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            tick: 0,
            dirty: false,
        }
    }
    pub fn mark(&mut self) {
        self.tick = self.tick.wrapping_add(1);
        self.dirty = true;
    }
    pub fn flush(&mut self) -> bool {
        let was = self.dirty;
        self.dirty = false;
        was
    }
    #[must_use]
    pub const fn tick_of(&self) -> u64 {
        self.tick
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fresh_is_clean_at_zero() {
        let mut s = CtxSync::new();
        assert_eq!(s.tick_of(), 0);
        assert!(!s.flush());
    }
    #[test]
    fn mark_bumps_tick_and_flush_reports_dirty_once() {
        let mut s = CtxSync::new();
        s.mark();
        s.mark();
        assert_eq!(s.tick_of(), 2);
        assert!(s.flush());
        assert!(!s.flush());
        assert_eq!(s.tick_of(), 2);
    }
    #[test]
    fn flush_without_mark_stays_clean() {
        let mut s = CtxSync::default();
        s.mark();
        assert!(s.flush());
        assert!(!s.flush());
        assert!(!s.flush());
    }
}
