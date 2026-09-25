#![forbid(unsafe_code)]
//! Send-counting wrapper over [`EventFanout`].

use crate::event_bus_full::EventFanout;

/// Fanout plus total-send counter.
#[derive(Debug, Default)]
pub struct WireFull {
    pub fan: EventFanout,
    sent: u64,
}

impl WireFull {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn send(&mut self, topic: &str, body: &str) -> usize {
        let n = self.fan.emit(topic, body);
        self.sent = self.sent.saturating_add(1);
        n
    }

    #[must_use]
    pub fn sent(&self) -> u64 {
        self.sent
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_zero() {
        assert_eq!(WireFull::new().sent(), 0);
    }

    #[test]
    fn hit_returns_one() {
        let mut w = WireFull::new();
        w.fan.subscribe("a");
        assert_eq!(w.send("a", "x"), 1);
        assert_eq!(w.sent(), 1);
    }

    #[test]
    fn miss_still_counts() {
        let mut w = WireFull::new();
        assert_eq!(w.send("ghost", "y"), 0);
        assert_eq!(w.sent(), 1);
        assert_eq!(w.fan.log_tail(1), vec!["ghost:y".to_string()]);
    }

    #[test]
    fn accumulates() {
        let mut w = WireFull::new();
        w.fan.subscribe("t");
        w.send("t", "1");
        w.send("t", "2");
        w.send("miss", "3");
        assert_eq!(w.sent(), 3);
    }

    #[test]
    fn log_passthrough() {
        let mut w = WireFull::new();
        w.fan.subscribe("t");
        w.send("t", "hi");
        assert_eq!(w.fan.log_tail(1), vec!["t:hi".to_string()]);
    }
}
