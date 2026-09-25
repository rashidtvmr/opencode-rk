#![forbid(unsafe_code)]
/// 1000ms poll transport: 512 cap, seq/ack, 1s-30s backoff. No SSE/batch.
#[derive(Debug, Clone, Default)]
pub struct LivePipe {
    pub seq: u64,
    pub ack: u64,
    pub backoff_ms: u32,
}
impl LivePipe {
    const CAP: usize = 512;
    const INIT: u32 = 1000;
    const MAX: u32 = 30000;
    #[must_use]
    pub fn new() -> Self {
        Self {
            backoff_ms: Self::INIT,
            ..Self::default()
        }
    }
    /// Push event; false when full (cap 512).
    pub fn push(&mut self) -> bool {
        if self.seq.wrapping_sub(self.ack) as usize >= Self::CAP {
            return false;
        }
        self.seq = self.seq.wrapping_add(1);
        true
    }
    pub fn ack_up(&mut self) {
        self.ack = self.ack.saturating_add(1);
    }
    /// Double backoff, saturate 1s..30s.
    pub fn next_backoff(&mut self) -> u32 {
        self.backoff_ms = self
            .backoff_ms
            .saturating_mul(2)
            .clamp(Self::INIT, Self::MAX);
        self.backoff_ms
    }
    pub fn reset_backoff(&mut self) {
        self.backoff_ms = Self::INIT;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn push_basic() {
        let mut p = LivePipe::new();
        assert!(p.push() && p.seq == 1 && p.ack == 0);
    }
    #[test]
    fn cap_512() {
        let mut p = LivePipe::new();
        for i in 0..512 {
            assert!(p.push(), "{i}");
        }
        assert!(!p.push() && p.seq == 512);
    }
    #[test]
    fn ack_frees_slot() {
        let mut p = LivePipe::new();
        p.push();
        p.push();
        p.ack_up();
        p.ack_up();
        assert!(p.ack == 2 && p.push());
    }
    #[test]
    fn backoff_doubles_saturates() {
        let mut p = LivePipe::new();
        assert_eq!((p.next_backoff(), p.next_backoff()), (2000, 4000));
        p.backoff_ms = u32::MAX;
        assert_eq!((p.next_backoff(), p.next_backoff()), (30000, 30000));
    }
    #[test]
    fn reset_restores() {
        let mut p = LivePipe::new();
        p.next_backoff();
        p.reset_backoff();
        assert_eq!(p.backoff_ms, 1000);
    }
}
