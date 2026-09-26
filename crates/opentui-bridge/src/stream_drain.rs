#![forbid(unsafe_code)]
//! Drain transport frames into SDK events (bridge glue).

use crate::sdk_stream::{SdkEvent, SdkStream};
use crate::stream_transport_full::StreamTransportFull;

/// Max frames per drain call.
pub const MAX_DRAIN_FRAMES: usize = 64;

/// Recv up to `max` bodies in FIFO order, capped at 64.
pub fn drain_frames(tx: &mut StreamTransportFull, max: usize) -> Vec<String> {
    let mut out = Vec::new();
    for _ in 0..max.min(MAX_DRAIN_FRAMES) {
        match tx.recv() {
            Some(f) => out.push(f.body),
            None => break,
        }
    }
    out
}

/// Push each body as `Text`; stop on first `false` (closed/full).
pub fn feed_sdk(sdk: &mut SdkStream, bodies: &[String]) {
    for b in bodies {
        if !sdk.push(SdkEvent::Text(b.clone())) {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_drain() {
        let mut t = StreamTransportFull::new();
        assert!(drain_frames(&mut t, 8).is_empty());
    }

    #[test]
    fn respects_max() {
        let mut t = StreamTransportFull::new();
        for b in ["a", "b", "c"] {
            t.send(b);
        }
        assert_eq!(
            drain_frames(&mut t, 2),
            vec!["a".to_owned(), "b".to_owned()]
        );
        assert_eq!(t.pending(), 1);
    }

    #[test]
    fn caps_64() {
        let mut t = StreamTransportFull::new();
        for _ in 0..70 {
            t.send("f");
        }
        let got = drain_frames(&mut t, 100);
        assert_eq!(got.len(), MAX_DRAIN_FRAMES);
        assert_eq!(t.pending(), 6);
    }

    #[test]
    fn feed_roundtrip() {
        let mut t = StreamTransportFull::new();
        t.send("x");
        t.send("y");
        let bodies = drain_frames(&mut t, 8);
        let mut s = SdkStream::new();
        feed_sdk(&mut s, &bodies);
        assert_eq!(s.drain_text(), "xy");
    }

    #[test]
    fn feed_stops_closed() {
        let mut s = SdkStream::new();
        s.close();
        feed_sdk(&mut s, &["a".to_string()]);
        assert!(s.drain_text().is_empty());
    }
}
