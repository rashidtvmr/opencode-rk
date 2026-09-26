#![forbid(unsafe_code)]
//! Sequenced run-stream transport framing (TS `run/stream.transport.ts` send/wait shape).
//!
//! Minimal boundary: callers `send` a body chunk, get a `seq` id, and later
//! `ack` it to drop it from `pending`. Fail-closed: an overlong body or a
//! full pending queue rejects the whole send, never partially queuing.

/// Byte cap for one frame body (8 KiB).
pub const FRAME_CAP_BYTES: usize = 8 * 1024;

/// Max queued unacked frames.
pub const TRANSPORT_CAP_FRAMES: usize = 128;

/// One sequenced transport frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub seq: u32,
    pub body: String,
}

/// Pending unacked frames with a wrapping sequence counter.
#[derive(Debug, Clone, Default)]
pub struct Transport {
    next_seq: u32,
    pending: Vec<Frame>,
}

impl Transport {
    /// Empty transport, sequence starts at 0.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Queue a body frame; returns its seq. Errs when overlong or queue full.
    pub fn send(&mut self, body: &str) -> Result<u32, String> {
        if body.len() > FRAME_CAP_BYTES {
            return Err(format!(
                "overlong frame: {} > {}",
                body.len(),
                FRAME_CAP_BYTES
            ));
        }
        if self.pending.len() >= TRANSPORT_CAP_FRAMES {
            return Err(format!(
                "transport full: {} frames pending",
                TRANSPORT_CAP_FRAMES
            ));
        }
        let seq = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1);
        self.pending.push(Frame {
            seq,
            body: body.to_owned(),
        });
        Ok(seq)
    }

    /// Drop the frame with `seq`; true when one was removed.
    pub fn ack(&mut self, seq: u32) -> bool {
        match self.pending.iter().position(|f| f.seq == seq) {
            Some(i) => {
                self.pending.remove(i);
                true
            }
            None => false,
        }
    }

    /// Number of unacked frames.
    #[must_use]
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn send_assigns_sequence() {
        let mut t = Transport::new();
        assert_eq!(t.send("a").unwrap(), 0);
        assert_eq!(t.send("b").unwrap(), 1);
        assert_eq!(t.pending_len(), 2);
    }

    #[test]
    fn send_overlong_errs() {
        let mut t = Transport::new();
        let big = "x".repeat(FRAME_CAP_BYTES + 1);
        assert!(t.send(&big).is_err());
        assert_eq!(t.pending_len(), 0);
    }

    #[test]
    fn ack_removes_frame() {
        let mut t = Transport::new();
        let seq = t.send("hi").unwrap();
        assert!(t.ack(seq));
        assert_eq!(t.pending_len(), 0);
    }

    #[test]
    fn ack_unknown_returns_false() {
        let mut t = Transport::new();
        t.send("hi").unwrap();
        assert!(!t.ack(999));
        assert_eq!(t.pending_len(), 1);
    }

    #[test]
    fn pending_cap_128() {
        let mut t = Transport::new();
        for _ in 0..TRANSPORT_CAP_FRAMES {
            t.send("f").unwrap();
        }
        assert_eq!(t.pending_len(), TRANSPORT_CAP_FRAMES);
        assert!(t.send("one-too-many").is_err());
    }

    #[test]
    fn seq_wraps_on_overflow() {
        let mut t = Transport::new();
        t.next_seq = u32::MAX;
        assert_eq!(t.send("a").unwrap(), u32::MAX);
        assert_eq!(t.send("b").unwrap(), 0);
    }
}
