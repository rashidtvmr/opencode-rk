//! Run timing trace spans (TS `run/trace.ts` timing ref).
//!
//! [`TraceLog`] is an in-memory span list: capped names, capped count,
//! saturating totals. Mirrors the dev-only JSONL trace intent without I/O.

#![forbid(unsafe_code)]

/// Maximum span name length in bytes.
pub const SPAN_NAME_CAP: usize = 128;

/// Maximum spans retained per log.
pub const SPAN_CAP: usize = 256;

fn truncate(mut s: String, cap: usize) -> String {
    if s.len() > cap {
        let mut end = cap;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        s.truncate(end);
    }
    s
}

/// One named timing span.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceSpan {
    name: String,
    ms: u64,
}

impl TraceSpan {
    /// Build a span, truncating name to 128 bytes.
    #[must_use]
    pub fn new(name: impl Into<String>, ms: u64) -> Self {
        Self {
            name: truncate(name.into(), SPAN_NAME_CAP),
            ms,
        }
    }

    /// Span name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Span duration in ms.
    #[must_use]
    pub fn ms(&self) -> u64 {
        self.ms
    }
}

/// Ordered span log, at most [`SPAN_CAP`] entries.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TraceLog {
    spans: Vec<TraceSpan>,
}

impl TraceLog {
    /// Empty log.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a span; returns false (drops) when at cap.
    pub fn record(&mut self, name: impl Into<String>, ms: u64) -> bool {
        if self.spans.len() >= SPAN_CAP {
            return false;
        }
        self.spans.push(TraceSpan::new(name, ms));
        true
    }

    /// Spans in insertion order.
    #[must_use]
    pub fn spans(&self) -> &[TraceSpan] {
        &self.spans
    }

    /// Saturating sum of span durations.
    #[must_use]
    pub fn total_ms(&self) -> u64 {
        self.spans.iter().fold(0u64, |a, s| a.saturating_add(s.ms))
    }

    /// Span with the largest duration, if any.
    #[must_use]
    pub fn slowest(&self) -> Option<&TraceSpan> {
        self.spans.iter().max_by_key(|s| s.ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_ok() {
        let mut log = TraceLog::new();
        assert!(log.record("render", 12));
        assert_eq!(log.spans().len(), 1);
        assert_eq!(log.spans()[0].name(), "render");
        assert_eq!(log.spans()[0].ms(), 12);
    }

    #[test]
    fn name_truncated_to_cap() {
        let s = TraceSpan::new("x".repeat(200), 1);
        assert_eq!(s.name().len(), SPAN_NAME_CAP);
    }

    #[test]
    fn cap_drops() {
        let mut log = TraceLog::new();
        for i in 0..SPAN_CAP {
            assert!(log.record(format!("s{i}"), i as u64));
        }
        assert!(!log.record("overflow", 1));
        assert_eq!(log.spans().len(), SPAN_CAP);
    }

    #[test]
    fn total_saturates() {
        let mut log = TraceLog::new();
        log.record("a", u64::MAX);
        log.record("b", 1);
        assert_eq!(log.total_ms(), u64::MAX);
    }

    #[test]
    fn slowest_picks_max() {
        let mut log = TraceLog::new();
        log.record("fast", 3);
        log.record("slow", 99);
        log.record("mid", 10);
        assert_eq!(log.slowest().unwrap().name(), "slow");
    }

    #[test]
    fn empty_none() {
        let log = TraceLog::new();
        assert!(log.slowest().is_none());
        assert_eq!(log.total_ms(), 0);
    }
}
