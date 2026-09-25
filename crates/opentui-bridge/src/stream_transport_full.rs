#![forbid(unsafe_code)]
//! Full FIFO stream transport (TS `run/stream.transport.ts` buffered/drain shape).
pub const BODY_CAP_BYTES: usize = 4 * 1024;
pub const FRAMES_CAP: usize = 512;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub seq: u64,
    pub body: String,
}
#[derive(Debug, Clone, Default)]
pub struct StreamTransportFull {
    frames: Vec<Frame>,
    next: u64,
}
impl StreamTransportFull {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    fn trunc(s: &str) -> String {
        if s.len() <= BODY_CAP_BYTES {
            return s.to_owned();
        }
        let mut end = BODY_CAP_BYTES;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        s[..end].to_owned()
    }
    pub fn send(&mut self, body: &str) -> u64 {
        let seq = self.next;
        self.next = self.next.wrapping_add(1);
        if self.frames.len() >= FRAMES_CAP {
            self.frames.remove(0);
        }
        self.frames.push(Frame {
            seq,
            body: Self::trunc(body),
        });
        seq
    }
    pub fn recv(&mut self) -> Option<Frame> {
        if self.frames.is_empty() {
            return None;
        }
        Some(self.frames.remove(0))
    }
    #[must_use]
    pub fn pending(&self) -> usize {
        self.frames.len()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn seq_grows() {
        let mut t = StreamTransportFull::new();
        assert_eq!(t.send("a"), 0);
        assert_eq!(t.send("b"), 1);
    }
    #[test]
    fn fifo_order() {
        let mut t = StreamTransportFull::new();
        t.send("a");
        t.send("b");
        assert_eq!(t.recv().unwrap().body, "a");
        assert_eq!(t.recv().unwrap().body, "b");
        assert!(t.recv().is_none());
    }
    #[test]
    fn cap_evicts_oldest() {
        let mut t = StreamTransportFull::new();
        for i in 0..FRAMES_CAP as u64 {
            assert_eq!(t.send("f"), i);
        }
        assert_eq!(t.pending(), FRAMES_CAP);
        assert_eq!(t.send("new"), FRAMES_CAP as u64);
        assert_eq!(t.pending(), FRAMES_CAP);
        assert_eq!(t.recv().unwrap().seq, 1);
    }
    #[test]
    fn truncates_body() {
        let mut t = StreamTransportFull::new();
        let big = "x".repeat(BODY_CAP_BYTES + 10);
        t.send(&big);
        assert_eq!(t.recv().unwrap().body.len(), BODY_CAP_BYTES);
    }
    #[test]
    fn trunc_char_boundary() {
        let mut t = StreamTransportFull::new();
        let big = "e".repeat(BODY_CAP_BYTES - 1) + "日本語tail";
        t.send(&big);
        let f = t.recv().unwrap();
        assert!(f.body.len() <= BODY_CAP_BYTES);
        assert!(f.body.is_char_boundary(f.body.len()));
    }
    #[test]
    fn pending_counts() {
        let mut t = StreamTransportFull::new();
        assert_eq!(t.pending(), 0);
        t.send("a");
        assert_eq!(t.pending(), 1);
        t.recv();
        assert_eq!(t.pending(), 0);
    }
}
