#![forbid(unsafe_code)]
//! Topic fanout bus (pure, std only, no IO/FFI).
//! Companion to `event_ctx`/`message_router`: exact-match subs, capped log.

/// Max topic subscriptions (fail-closed).
pub const MAX_FANOUT_SUBS: usize = 32;
/// Max retained log entries (fail-closed, oldest evicted).
pub const MAX_FANOUT_LOG: usize = 64;
/// Max chars per `topic:body` log entry.
pub const MAX_ENTRY: usize = 256;

/// Exact-match topic fanout with bounded log.
#[derive(Debug, Default, Clone)]
pub struct EventFanout {
    subs: Vec<String>,
    log: Vec<String>,
}

impl EventFanout {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            subs: Vec::new(),
            log: Vec::new(),
        }
    }

    /// Subscribe to `topic`. False when empty, duplicate, or full.
    pub fn subscribe(&mut self, topic: &str) -> bool {
        if topic.is_empty() || self.subs.len() >= MAX_FANOUT_SUBS {
            return false;
        }
        if self.subs.iter().any(|s| s == topic) {
            return false;
        }
        self.subs.push(topic.to_string());
        true
    }

    /// Emit to `topic`; returns matching sub count, always logs `topic:body`.
    pub fn emit(&mut self, topic: &str, body: &str) -> usize {
        let n = self.subs.iter().filter(|s| s.as_str() == topic).count();
        let entry: String = format!("{topic}:{body}").chars().take(MAX_ENTRY).collect();
        if self.log.len() >= MAX_FANOUT_LOG {
            self.log.remove(0);
        }
        self.log.push(entry);
        n
    }

    /// Last `n` log entries, oldest first.
    #[must_use]
    pub fn log_tail(&self, n: usize) -> Vec<String> {
        let start = self.log.len().saturating_sub(n);
        self.log[start..].to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sub_emit_hit() {
        let mut b = EventFanout::new();
        assert!(b.subscribe("a"));
        assert_eq!(b.emit("a", "x"), 1);
        assert_eq!(b.log_tail(1), vec!["a:x".to_string()]);
    }

    #[test]
    fn emit_miss_zero_but_logs() {
        let mut b = EventFanout::new();
        assert!(b.subscribe("a"));
        assert_eq!(b.emit("ghost", "y"), 0);
        assert_eq!(b.log_tail(1), vec!["ghost:y".to_string()]);
    }

    #[test]
    fn dup_empty_false() {
        let mut b = EventFanout::new();
        assert!(b.subscribe("a"));
        assert!(!b.subscribe("a"));
        assert!(!b.subscribe(""));
    }

    #[test]
    fn sub_cap_32() {
        let mut b = EventFanout::new();
        for i in 0..MAX_FANOUT_SUBS {
            assert!(b.subscribe(&format!("t{i}")));
        }
        assert!(!b.subscribe("extra"));
    }

    #[test]
    fn entry_truncates_256() {
        let mut b = EventFanout::new();
        b.subscribe("t");
        b.emit("t", &"y".repeat(MAX_ENTRY + 40));
        assert_eq!(b.log_tail(1)[0].chars().count(), MAX_ENTRY);
    }

    #[test]
    fn log_cap_64_and_tail() {
        let mut b = EventFanout::new();
        b.subscribe("t");
        for i in 0..(MAX_FANOUT_LOG + 10) {
            b.emit("t", &i.to_string());
        }
        assert_eq!(b.log_tail(1000).len(), MAX_FANOUT_LOG);
        assert_eq!(b.log_tail(0).len(), 0);
        let tail = b.log_tail(2);
        assert_eq!(tail, vec!["t:72".to_string(), "t:73".to_string()]);
    }
}
