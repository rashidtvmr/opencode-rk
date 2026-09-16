//! WEB-005 frozen tests T01..T05 (server SSE transport envelope, no web/ touch).
//!
//! The lane owns exactly two files and never edits shared `lib.rs`, so the
//! transport module is included via path (the integrator wires
//! `mod event_stream;` into `lib.rs` later).

#[path = "../src/event_stream.rs"]
mod event_stream;

use event_stream::{encode_frame, keepalive_frame, sse_headers, FrameError, Hub, StreamConfig};

// WEB-005-T01 (framing happy path).
#[test]
fn web005_t01_framing_happy_path() {
    let frame = encode_frame(b"{\"a\":1}").expect("small payload encodes");
    assert_eq!(frame, "data: {\"a\":1}\n\n");

    let mut hub = Hub::new(StreamConfig::default());
    let id = hub.subscribe();
    for payload in [
        b"{\"a\":1}".as_slice(),
        b"{\"b\":2}".as_slice(),
        b"{\"c\":3}".as_slice(),
    ] {
        let summary = hub.publish(payload);
        assert_eq!(summary.delivered, 1);
        assert_eq!(summary.dropped_oversize, 0);
        assert!(summary.slow_disconnected.is_empty());
    }
    let frames = hub.received(id);
    assert_eq!(frames.len(), 3);
    for frame in &frames {
        assert!(frame.starts_with("data: "), "partial data: line: {frame:?}");
        assert!(frame.ends_with("\n\n"), "unterminated frame: {frame:?}");
        assert_eq!(frame.matches("data: ").count(), 1);
    }
    assert_eq!(frames[0], "data: {\"a\":1}\n\n");
    assert_eq!(frames[1], "data: {\"b\":2}\n\n");
    assert_eq!(frames[2], "data: {\"c\":3}\n\n");

    let headers = sse_headers();
    assert!(headers.contains(&("Content-Type", "text/event-stream")));
    assert!(headers.contains(&("Cache-Control", "no-cache")));

    // Deterministic keepalive: fixed comment, no clock data.
    assert_eq!(keepalive_frame(), ":ping\n\n");
    assert_eq!(keepalive_frame(), keepalive_frame());
    assert!(!keepalive_frame().bytes().any(|b| b.is_ascii_digit()));
}

// WEB-005-T02 (oversize drop, subscriber kept alive).
#[test]
fn web005_t02_oversize_drop_keeps_subscriber() {
    let config = StreamConfig {
        max_frame_bytes: 16,
        max_buffered_frames: 64,
        keepalive_ms: 15_000,
    };
    assert_eq!(config.byte_cap(), 16 * 64);

    let big = vec![b'x'; 17];
    let err = config.encode_frame(&big).unwrap_err();
    assert_eq!(err, FrameError::TooLarge);

    let mut hub = Hub::new(config);
    let id = hub.subscribe();
    let summary = hub.publish(&big);
    assert_eq!(summary.dropped_oversize, 1);
    assert_eq!(summary.delivered, 0);
    assert_eq!(hub.dropped_frames(), 1);
    assert!(hub.received(id).is_empty());
    assert!(hub.is_connected(id));

    let summary = hub.publish(b"{\"ok\":1}");
    assert_eq!(summary.delivered, 1);
    let got = hub.received(id);
    assert_eq!(got.len(), 1);
    assert_eq!(got[0], "data: {\"ok\":1}\n\n");
}

// WEB-005-T03 (slow consumer disconnect, others unaffected).
#[test]
fn web005_t03_slow_consumer_disconnects_only_slow() {
    let slow_cfg = StreamConfig {
        max_frame_bytes: 1_024,
        max_buffered_frames: 2,
        keepalive_ms: 15_000,
    };
    let roomy_cfg = StreamConfig {
        max_frame_bytes: 1_024,
        max_buffered_frames: 16,
        keepalive_ms: 15_000,
    };
    let mut hub = Hub::new(StreamConfig::default());
    let slow = hub.subscribe_with(slow_cfg);
    let live = hub.subscribe_with(roomy_cfg);

    let mut expected = Vec::new();
    for i in 0..5 {
        let payload = format!("{{\"i\":{i}}}");
        let summary = hub.publish(payload.as_bytes());
        assert_eq!(summary.dropped_oversize, 0);
        expected.push(format!("data: {payload}\n\n"));
    }

    assert!(hub.disconnected_as_slow(slow));
    assert!(!hub.is_connected(slow));
    // Frames accepted before the overflow stay buffered; nothing after.
    assert_eq!(hub.received(slow), &expected[..2]);

    assert!(hub.is_connected(live));
    assert!(!hub.disconnected_as_slow(live));
    assert_eq!(hub.received(live), expected);
}

// WEB-005-T04 (disconnect reclaims state, no leak).
#[test]
fn web005_t04_disconnect_reclaims_state() {
    let mut hub = Hub::new(StreamConfig::default());
    let baseline_bytes = hub.buffered_bytes();
    let baseline_tasks = hub.live_tasks();

    let a = hub.subscribe();
    let b = hub.subscribe();
    let c = hub.subscribe();
    assert_eq!(hub.live_subscribers(), 3);
    assert_eq!(hub.live_tasks(), baseline_tasks + 3);

    hub.publish(b"{\"n\":1}");
    hub.publish(b"{\"n\":2}");
    assert!(hub.buffered_bytes() > baseline_bytes);

    // Drop one mid-stream: its pending frames are reclaimed, others keep theirs.
    assert!(hub.unsubscribe(b));
    assert_eq!(hub.live_subscribers(), 2);
    let per_sub: usize = hub.received(a).iter().map(|f| f.len()).sum();
    assert_eq!(hub.buffered_bytes(), baseline_bytes + 2 * per_sub);

    // Publishing after close never flushes to the closed subscriber.
    let summary = hub.publish(b"{\"n\":3}");
    assert_eq!(summary.delivered, 2);
    assert!(!hub.flushed_after_close());
    assert_eq!(hub.received(a).len(), 3);
    assert_eq!(hub.received(c).len(), 3);
    assert_eq!(
        hub.publish_to(b, b"{\"n\":4}"),
        Err(event_stream::DeliverError::Closed)
    );

    // Dropping the rest returns bytes and tasks to baseline: no leak.
    assert!(hub.unsubscribe(a));
    assert!(hub.unsubscribe(c));
    assert!(!hub.unsubscribe(b));
    assert_eq!(hub.live_subscribers(), 0);
    assert_eq!(hub.buffered_bytes(), baseline_bytes);
    assert_eq!(hub.live_tasks(), baseline_tasks);
}

// WEB-005-T05 (invalid UTF-8: no partial frame, fixture-only side effects).
#[test]
fn web005_t05_invalid_utf8_no_partial_frame() {
    let dir = tempfile::tempdir().expect("disposable fixture");
    let bad: &[u8] = b"fo\x80o";

    let err = StreamConfig::default().encode_frame(bad).unwrap_err();
    assert_eq!(err, FrameError::InvalidUtf8);

    let mut hub = Hub::new(StreamConfig::default());
    let id = hub.subscribe();
    let summary = hub.publish(bad);
    assert_eq!(summary.delivered, 0);
    assert!(summary.emitted_bytes.is_empty());
    assert_eq!(hub.invalid_frames(), 1);
    assert!(hub.received(id).is_empty());
    assert!(hub.is_connected(id));

    // Error text carries zero payload bytes (secret safety: no payload echo).
    let rendered = format!("{err} {err:?}");
    assert!(!rendered.as_bytes().windows(bad.len()).any(|w| w == bad));

    // Only the disposable fixture dir is touched.
    let marker = dir.path().join("frames.log");
    std::fs::write(&marker, b"web-005 fixture").expect("fixture write");
    assert!(marker.exists());
}
