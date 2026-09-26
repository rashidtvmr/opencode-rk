#![forbid(unsafe_code)]
//! SDK connection latch (TS: `packages/tui/src/context/sdk.tsx` createSDK pattern).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CtxSdk {
    pub connected: bool,
    pub calls: u32,
}
impl CtxSdk {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            connected: false,
            calls: 0,
        }
    }
    pub fn connect(&mut self) {
        self.connected = true;
    }
    pub fn call(&mut self) -> bool {
        if !self.connected {
            return false;
        }
        // ponytail: no disconnect/retry ceiling; add when TS abort/recreate mapped.
        self.calls = self.calls.saturating_add(1);
        true
    }
    #[must_use]
    pub const fn is_connected(&self) -> bool {
        self.connected
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn down_call_fails_without_count() {
        let mut s = CtxSdk::new();
        assert!(!s.call());
        assert_eq!(s.calls, 0);
        assert!(!s.is_connected());
    }
    #[test]
    fn connect_enables_counted_calls() {
        let mut s = CtxSdk::default();
        s.connect();
        assert!(s.is_connected());
        assert!(s.call());
        assert!(s.call());
        assert_eq!(s.calls, 2);
    }
    #[test]
    fn count_saturates_at_max() {
        let mut s = CtxSdk {
            connected: true,
            calls: u32::MAX,
        };
        assert!(s.call());
        assert_eq!(s.calls, u32::MAX);
    }
}
