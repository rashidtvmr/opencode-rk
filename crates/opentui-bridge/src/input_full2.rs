#![forbid(unsafe_code)]
//! Capped byte buffer over `input_adapter`.
use crate::input_adapter::{drain_step, feed_bytes};
use crate::native_input::MAX_TEXT;

/// Stdin byte buffer capped at 4KiB (`MAX_TEXT`).
#[derive(Debug, Default)]
pub struct InputFull {
    buf: Vec<u8>,
}

impl InputFull {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append bytes; true when fully accepted, false when capped.
    // ponytail: bool only, no partial-take count; add when caller needs it.
    pub fn feed(&mut self, bytes: &[u8]) -> bool {
        let before = self.buf.len();
        let kept = feed_bytes(&mut self.buf, bytes);
        kept == before.saturating_add(bytes.len())
    }

    /// Decode complete chunks into labels, draining consumed bytes.
    pub fn drain(&mut self) -> Vec<String> {
        drain_step(&mut self.buf)
    }

    /// Undecoded bytes held.
    pub fn pending(&self) -> usize {
        self.buf.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_drain_empty() {
        let mut f = InputFull::new();
        assert_eq!(f.pending(), 0);
        assert!(f.drain().is_empty());
    }

    #[test]
    fn submit_roundtrip() {
        let mut f = InputFull::new();
        assert!(f.feed(b"\r"));
        assert_eq!(f.pending(), 1);
        assert_eq!(f.drain(), vec!["submit".to_string()]);
        assert_eq!(f.pending(), 0);
    }

    #[test]
    fn cap_4kib() {
        let mut f = InputFull::new();
        assert!(!f.feed(&vec![b'a'; MAX_TEXT + 8]));
        assert_eq!(f.pending(), MAX_TEXT);
        assert_eq!(f.pending(), 4096);
    }

    #[test]
    fn ctrl_c_quits() {
        let mut f = InputFull::new();
        assert!(f.feed(&[0x03]));
        assert_eq!(f.drain(), vec!["quit".to_string()]);
    }

    #[test]
    fn small_feed_true_tracks_pending() {
        let mut f = InputFull::new();
        assert!(f.feed(b"ab"));
        assert_eq!(f.pending(), 2);
    }
}
