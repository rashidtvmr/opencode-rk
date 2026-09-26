//! Bounded SDK SSE buffer (mirrors `sdk.tsx` event fan-out).
#![forbid(unsafe_code)]

pub const MAX_EVENTS: usize = 512;
pub const MAX_TEXT_BYTES: usize = 4096;
pub const MAX_ERROR_BYTES: usize = 512;
pub const MAX_DRAIN_CHARS: usize = 65536;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SdkEvent {
    Text(String),
    Done,
    Error(String),
}

#[derive(Debug, Default)]
pub struct SdkStream {
    events: Vec<SdkEvent>,
    closed: bool,
}

impl SdkStream {
    pub fn new() -> Self {
        Self::default()
    }
    /// Push event; `false` when closed or full.
    pub fn push(&mut self, event: SdkEvent) -> bool {
        if self.closed || self.events.len() >= MAX_EVENTS {
            return false;
        }
        let owned = match event {
            SdkEvent::Text(t) => SdkEvent::Text(trunc(&t, MAX_TEXT_BYTES)),
            SdkEvent::Error(e) => SdkEvent::Error(trunc(&e, MAX_ERROR_BYTES)),
            SdkEvent::Done => SdkEvent::Done,
        };
        self.events.push(owned);
        true
    }
    pub fn close(&mut self) {
        self.closed = true;
    }
    /// Concat `Text`, drop them, keep `Done`/`Error`; cap 64KiB chars.
    pub fn drain_text(&mut self) -> String {
        let mut out = String::new();
        self.events.retain(|e| match e {
            SdkEvent::Text(t) => {
                out.push_str(t);
                false
            }
            _ => true,
        });
        if out.chars().count() > MAX_DRAIN_CHARS {
            out = out.chars().take(MAX_DRAIN_CHARS).collect();
        }
        out
    }
}

fn trunc(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_ok() {
        let mut s = SdkStream::new();
        assert!(s.push(SdkEvent::Text("hi".into())));
        assert_eq!(s.events.len(), 1);
    }

    #[test]
    fn closed_blocks() {
        let mut s = SdkStream::new();
        s.close();
        assert!(!s.push(SdkEvent::Text("x".into())));
        assert!(!s.push(SdkEvent::Done));
        assert!(s.events.is_empty());
    }

    #[test]
    fn drain_concat_clear() {
        let mut s = SdkStream::new();
        s.push(SdkEvent::Text("a".into()));
        s.push(SdkEvent::Text("b".into()));
        assert_eq!(s.drain_text(), "ab");
        assert!(s.events.is_empty());
    }

    #[test]
    fn drain_keeps_error_done() {
        let mut s = SdkStream::new();
        s.push(SdkEvent::Error("boom".into()));
        s.push(SdkEvent::Text("t".into()));
        s.push(SdkEvent::Done);
        assert_eq!(s.drain_text(), "t");
        assert_eq!(s.events.len(), 2);
    }

    #[test]
    fn caps_events_and_payloads() {
        let mut s = SdkStream::new();
        for _ in 0..MAX_EVENTS {
            assert!(s.push(SdkEvent::Done));
        }
        assert!(!s.push(SdkEvent::Done));
        let mut t = SdkStream::new();
        t.push(SdkEvent::Text("a".repeat(5000)));
        assert!(matches!(&t.events[0], SdkEvent::Text(x) if x.len() == MAX_TEXT_BYTES));
        t.push(SdkEvent::Error("e".repeat(600)));
        assert!(matches!(&t.events[1], SdkEvent::Error(x) if x.len() == MAX_ERROR_BYTES));
    }

    #[test]
    fn drain_caps_64k() {
        let mut s = SdkStream::new();
        for _ in 0..16 {
            s.push(SdkEvent::Text("a".repeat(MAX_TEXT_BYTES)));
        }
        assert!(s.drain_text().chars().count() <= MAX_DRAIN_CHARS);
    }
}
